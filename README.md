# Blacksmith

[![Client checks](https://github.com/HormigaDev/blacksmith/actions/workflows/client.yml/badge.svg)](https://github.com/HormigaDev/blacksmith/actions/workflows/client.yml)
[![Rust](https://img.shields.io/badge/Rust-1.97%2B-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

[English](./README.md) | [Español](./docs/README_ESPAÑOL.md) | [Português](./docs/README_PORTUGUÊS.md)

## Table of contents

- [Why Blacksmith exists](#why-blacksmith-exists)
- [What it can manage](#what-it-can-manage)
- [What it deliberately does not do](#what-it-deliberately-does-not-do)
- [Architecture](#architecture)
    - [Security model](#security-model)
- [Screenshots](#screenshots)
- [Quick start for experienced users](#quick-start-for-experienced-users)
    - [Build the agent](#build-the-agent)
    - [Install the server units](#install-the-server-units)
    - [Run the desktop client](#run-the-desktop-client)
- [Complete setup guide](#complete-setup-guide)
    - [Requirements](#requirements)
    - [1. Create the service accounts](#1-create-the-service-accounts)
    - [2. Install and verify polkit](#2-install-and-verify-polkit)
    - [3. Configure the agent](#3-configure-the-agent)
    - [4. Install nginx integration when needed](#4-install-nginx-integration-when-needed)
    - [5. Register and deploy a service](#5-register-and-deploy-a-service)
- [Configure GitHub webhooks](#configure-github-webhooks)
- [Development](#development)
- [Repository map](#repository-map)
- [Security](#security)
- [License](#license)
- [Support the project](#support-the-project)

Blacksmith is a small desktop control panel and server agent for deploying straightforward
applications to bare-metal VPS machines.

It is designed for people who want the convenience of a deployment panel without turning a
4 GB VPS into a second infrastructure project. The agent is written in Rust, runs as a
systemd service, and stays private on the server. The desktop client reaches it through an
SSH tunnel, so the administration API does not need to be exposed to the Internet.

Blacksmith is intentionally narrow. It is not a general-purpose PaaS, a container platform,
or a multi-host orchestrator. It manages a small number of applications on a server you
control, using the operating system tools that are already there.

## Why Blacksmith exists

Self-hosted alternatives inspired by Heroku or Vercel can be useful, but they often bring
their own databases, queues, proxies, container runtimes, or orchestration layers. On a VPS
with 4 GB of RAM and one vCPU, that overhead is not an abstraction; it is the budget.

Blacksmith was built to solve a simpler problem:

> Deploy and operate a few projects reliably without manually repeating the same SSH commands
> for every release, and without installing a heavy platform to do it.

Rust was chosen for the agent because the server-side part needs to be small, predictable,
and careful around process execution, credentials, systemd, and filesystem boundaries.
The result is a tool that keeps deployment close to the host while providing a usable
desktop workflow.

## What it can manage

Blacksmith is a good fit for applications such as:

- Python services managed by systemd
- Node.js applications and APIs
- Rust binaries and services
- Static sites and built frontends
- APIs, WebSocket services, or an application with a frontend, API, and WebSocket endpoint

A deployment can clone a branch, run preparation commands, activate a new release, start the
service, and retain the previous release for rollback. Static output can be published through
the bundled nginx integration. Environment files, service logs, resource limits, and basic
host information are managed from the same client.

## What it deliberately does not do

Blacksmith does not use or manage:

- Docker, Podman, Kubernetes, or other container runtimes
- Serverless functions
- Multi-host orchestration
- Complex distributed applications
- A public administration endpoint
- A replacement for systemd, nginx, Git, or OpenSSH

The bare-metal constraint is part of the design, not a missing feature. If the application
needs a service mesh, a distributed scheduler, automatic horizontal scaling, or isolated
container images, Blacksmith is the wrong tool.

## Architecture

The repository is a Cargo workspace with three main parts:

| Component           | Role                                                  |
| ------------------- | ----------------------------------------------------- |
| `agent/`            | Axum and Tokio service installed on the VPS           |
| `client/`           | SvelteKit user interface                              |
| `client/src-tauri/` | Tauri 2 desktop runtime and SSH transport             |
| `shared/`           | Serializable API types shared by the agent and client |

The agent listens on `127.0.0.1:4040` by default. The desktop application opens an SSH
local-forward to that address and sends requests through the tunnel. OpenSSH authenticates
the connection; Blacksmith does not add a second login system with JWTs or browser sessions.

The public HTTP surface is limited to the GitHub webhook endpoint when webhooks are enabled.
Administration remains behind SSH.

### Security model

- The agent does not execute application commands itself; systemd service templates do.
- Application services run under a separate unprivileged account on server installations.
- systemd and polkit restrict which units the agent may control.
- Deployment credentials are passed through systemd credentials rather than command-line
  arguments or repository URLs.
- Private keys stay in the desktop runtime and are stored in the system credential store
  using envelope encryption.
- Service processes are confined to their own state directory by systemd sandboxing.
- Resource limits apply to the service cgroup, not just its main process.

Review the deployment templates and polkit rules before using Blacksmith on a production host.
They are part of the security boundary.

## Screenshots

The repository includes the generated visual snapshots used by the client regression suite.
These are representative views of the desktop workflow:

![Server list](client/e2e/__screenshots__/visual/server-list--lista-dark.png)

![Service dashboard](client/e2e/__screenshots__/visual/service-dashboard--en-marcha-dark.png)

![Deployment form](client/e2e/__screenshots__/visual/deployment-form--definido-dark.png)

![Live logs](client/e2e/__screenshots__/visual/log-viewer--en-vivo-dark.png)

The visual suite also covers light mode, connection failures, host-key confirmation,
deployment history, environment editing, resource limits, nginx sites, and static services.

## Quick start for experienced users

### Build the agent

Blacksmith requires Rust 1.97 or newer. The repository pins the toolchain in
[`rust-toolchain.toml`](./rust-toolchain.toml).

```sh
git clone https://github.com/HormigaDev/blacksmith.git
cd blacksmith
cargo build --release -p agent
```

The resulting binary is `target/release/blacksmith-agent`.

### Install the server units

Install the agent and the systemd templates as root on the VPS. The exact paths can be
adapted to your host, but the service account and unit names should remain consistent with
the files in `deploy/`.

```sh
install -D -o root -g root -m 0755 \
  target/release/blacksmith-agent \
  /opt/blacksmith/bin/blacksmith-agent

install -m 0644 deploy/systemd/*.service /etc/systemd/system/
install -m 0644 deploy/blacksmith.service /etc/systemd/system/
install -m 0644 deploy/polkit/10-blacksmith.rules \
  /etc/polkit-1/rules.d/10-blacksmith.rules

systemctl daemon-reload
systemctl enable --now blacksmith.service
```

The service account, directories, environment file, nginx integration, and credentials must
be prepared according to the deployment files and the full guide below. Do not replace the
polkit rules with a broad `sudoers` entry.

### Run the desktop client

```sh
cd client
pnpm install
pnpm tauri dev
```

Add the server in the client, accept its host key on first connection, and configure the
service repository and branch. For local development without a VPS, the agent can run against
the current user's systemd instance:

```sh
BLACKSMITH_SYSTEMD_SCOPE=user cargo run -p agent
```

The local mode does not provide the server sandbox. Use it only with repositories and commands
you trust.

## Complete setup guide

### Requirements

On the VPS:

- A Linux system with systemd
- Git
- Rust-built Blacksmith agent
- polkit
- nginx and ACL support when serving static sites or reverse proxies
- certbot when HTTPS certificates are required

On the desktop:

- Rust 1.97 or newer
- Node.js 24.14.0
- pnpm 11
- Tauri 2 system prerequisites for your platform

For Linux desktop development, install the GTK 3 and WebKitGTK 4.1 development packages
listed in the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/).

### 1. Create the service accounts

Server installations use separate accounts for the agent, application services, and fetch
operations. Create them once as root:

```sh
for user in blacksmith blacksmith-apps blacksmith-fetch; do
    useradd --system --no-create-home --home-dir /nonexistent \
        --shell /usr/sbin/nologin "$user"
done
```

The committed units in `deploy/systemd/` document the ownership and filesystem paths they
expect. Read them before starting the agent.

### 2. Install and verify polkit

```sh
install -o root -g root -m 0644 \
  deploy/polkit/10-blacksmith.rules \
  /etc/polkit-1/rules.d/10-blacksmith.rules

sh deploy/polkit/verify.sh
```

The verification script creates temporary test units and checks both allowed and denied
operations. A successful result should end with `regla correcta`.

### 3. Configure the agent

Copy the environment template and set the values appropriate for the host:

```sh
install -d -o root -g blacksmith -m 0750 /etc/blacksmith
install -o root -g blacksmith -m 0640 agent.env /etc/blacksmith/agent.env
```

Configuration uses `BLACKSMITH_*` variables. The agent validates unknown variables and
out-of-range values at startup. In production, secrets such as the webhook secret should be
provided through systemd credentials rather than ordinary environment variables.

### 4. Install nginx integration when needed

For sites and reverse proxies:

```sh
apt install nginx certbot acl
sh deploy/nginx/install.sh
```

The agent generates nginx configuration from validated site data. It supports static roots,
HTTP upstreams, WebSockets, and optional HTTPS through certbot. The administration API is not
published by this setup.

## Configure GitHub webhooks

The webhook signature uses a dedicated HMAC secret. It is **not** the GitHub user token, the
OAuth client secret, or the private key of a GitHub App. Blacksmith does not need and must never
receive the private key of the BBEL Blacksmith App.

The default BBEL Blacksmith App cannot give end users its webhook secret. To use automatic
deployments safely, create and manage your own GitHub App:

1. Open GitHub **Settings → Developer settings → GitHub Apps → New GitHub App**.
2. Set a descriptive name and homepage URL for your project.
3. Enable **Webhook** and set the callback URL to
   `https://hooks.example.com/api/webhooks/github`.
4. Generate a random webhook secret and keep it available while configuring the app and server:

5. Configure the app's repository permissions for the repositories it will manage. For
   automatic deploy-key registration, grant **Administration: Read and write** and install the
   app only on the repositories that need it. This permission is powerful because it can manage
   repository settings; do not grant it to the default BBEL Blacksmith App.
6. Install the app on the selected account or repositories and use its **App ID/client ID and
   slug** in the desktop client's GitHub settings. The app's private key is not required by
   Blacksmith and should not be uploaded to the client or server.

Store the webhook secret only on the VPS. Create the protected credential file as root:

```sh
install -d -o root -g root -m 0700 /etc/blacksmith/credentials
umask 077
openssl rand -hex 32 > /etc/blacksmith/credentials/github-webhook-secret
chown root:root /etc/blacksmith/credentials/github-webhook-secret
chmod 0600 /etc/blacksmith/credentials/github-webhook-secret
cat /etc/blacksmith/credentials/github-webhook-secret
```

Copy the displayed value into the GitHub App's webhook configuration, then uncomment these lines in
`deploy/blacksmith.service`:

```ini
LoadCredential=github-webhook-secret:/etc/blacksmith/credentials/github-webhook-secret
Environment=BLACKSMITH_GITHUB_WEBHOOK_SECRET_FILE=/run/credentials/blacksmith.service/github-webhook-secret
```

Apply the unit and expose only the webhook endpoint through the nginx configuration:

```sh
install -m 0644 deploy/blacksmith.service /etc/systemd/system/blacksmith.service
systemctl daemon-reload
systemctl restart blacksmith.service
nginx -t && systemctl reload nginx
```

The same secret can also be configured on a repository webhook under **Repository Settings →
Webhooks**, if you do not use a GitHub App. In that case, the webhook still authenticates
GitHub's signed request; it does not authenticate a desktop user. Keep the endpoint restricted
to `POST /api/webhooks/github` and never expose the agent's administration port.

When a service uses a deploy key, a custom GitHub App with **Administration: Read and write**
automatically registers the generated read-only key in the selected repository. With the default
BBEL Blacksmith App, the client deliberately falls back to the documented manual copy-and-paste
flow because that permission is not granted.

### 5. Register and deploy a service

From the desktop client:

1. Add the VPS and connect through SSH.
2. Register a service with a lowercase slug.
3. Select the GitHub repository and branch.
4. Add setup commands such as `python -m venv .venv`, `npm ci`, or `cargo build --release`.
5. Set the service start command, if it has a process.
6. Optionally choose a public directory for a static site.
7. Deploy and inspect the service status and logs.

Each deployment gets its own release directory. The previous release remains available for
rollback when a new build or service start fails.

## Development

From the repository root:

```sh
cargo check --workspace
cargo test -p agent -p shared
cargo clippy -p agent -p shared --all-targets -- -D warnings
```

For the client:

```sh
cd client
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm test:e2e:app
```

The client workflow is manually dispatched from GitHub Actions because its browser and visual
test suites are intentionally heavier than a per-commit check.

## Repository map

```text
agent/                 VPS agent and systemd integration
client/                SvelteKit UI, tests, and visual snapshots
client/src-tauri/      Tauri desktop runtime and SSH implementation
shared/                Shared Rust API models
deploy/                systemd, nginx, polkit, and installation files
```

## Security

Blacksmith requires careful system administration. Please read the
[security policy](./SECURITY.md) before deploying it to a production host. It explains the
supported `v1.0.x` releases, private vulnerability reporting process, and the operator's
responsibility to configure SSH, systemd, polkit, credentials, firewall rules, and deployment
commands safely.

## License

Blacksmith is released under the [MIT License](./LICENSE).

## Support the project

If Blacksmith saves you time or makes a small VPS more practical, you can support its
continued maintenance:

[![Donate with PayPal](https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif)](https://www.paypal.com/donate/?hosted_button_id=UCL7EE2G44KPQ)
