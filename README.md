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
    - [Install on the server](#install-on-the-server)
    - [Run the desktop client](#run-the-desktop-client)
- [Complete setup guide](#complete-setup-guide)
    - [Requirements](#requirements)
    - [1. Build the agent](#1-build-the-agent)
    - [2. Create the service accounts](#2-create-the-service-accounts)
    - [3. Install the agent and its configuration](#3-install-the-agent-and-its-configuration)
    - [4. Install the service templates and polkit](#4-install-the-service-templates-and-polkit)
    - [5. Start the agent](#5-start-the-agent)
    - [6. Allow the SSH tunnel](#6-allow-the-ssh-tunnel)
    - [7. Web sites with nginx (optional)](#7-web-sites-with-nginx-optional)
    - [8. Deploy on push with GitHub webhooks (optional)](#8-deploy-on-push-with-github-webhooks-optional)
    - [9. Register and deploy a service](#9-register-and-deploy-a-service)
    - [Updating](#updating)
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

### Install on the server

The short version, as root on the VPS, from the repository root (each step is explained in the
[complete setup guide](#complete-setup-guide)):

```sh
apt install git openssh-client polkitd
for user in blacksmith blacksmith-apps blacksmith-fetch; do
    useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "$user"
done
install -D -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -d -o root -g blacksmith -m 0750 /etc/blacksmith
install -o root -g blacksmith -m 0640 deploy/agent.env /etc/blacksmith/agent.env
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
install -m 0644 deploy/systemd/*.service deploy/blacksmith.service /etc/systemd/system/
systemctl daemon-reload
sh deploy/polkit/verify.sh && sh deploy/systemd/verify-sandbox.sh
systemctl enable --now blacksmith.service
```

Optional: `sh deploy/nginx/install.sh` for web sites and
`sh deploy/nginx/install-webhooks.sh <domain>` for deploy-on-push. Never replace the polkit rule
with a broad `sudoers` entry.

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

Every command in steps 1–8 runs **as root on the VPS, from the root of a checkout of this
repository** (the scripts and unit files are read from `deploy/`).

### Requirements

On the VPS (Debian 12+ or Ubuntu 22.04+; package names may differ elsewhere):

- systemd, polkit (`polkitd`), Git and the OpenSSH client (`git`, `openssh-client`): the agent
  manages units through polkit, and deployments clone with `git` (and `ssh` for deploy keys).
- An SSH server that allows TCP forwarding (see step 6).
- For web sites: `nginx` and, for HTTPS, `certbot`. A domain whose DNS points to the VPS and
  ports 80 and 443 open.
- To build on the VPS itself: Rust 1.97+ and a C compiler (`build-essential`).

On the desktop:

- Rust 1.97 or newer
- Node.js 24.14.0
- pnpm 11
- Tauri 2 system prerequisites for your platform (on Linux, the GTK 3 and WebKitGTK 4.1
  development packages listed in the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/))

```sh
apt install git openssh-client polkitd
```

### 1. Build the agent

```sh
cargo build --release -p agent
```

Build on the VPS, or on a machine with the same architecture and an equal or older glibc, and
copy `target/release/blacksmith-agent` together with the `deploy/` directory.

### 2. Create the service accounts

Three accounts, so that no deployed code runs with the agent's permissions:

| Account | Runs | Can |
|---|---|---|
| `blacksmith` | the agent | manage its own units through polkit |
| `blacksmith-apps` | services and their setup commands | write only in its service directory |
| `blacksmith-fetch` | the repository clone | read the deployment credentials, nothing else |

```sh
for user in blacksmith blacksmith-apps blacksmith-fetch; do
    useradd --system --no-create-home --home-dir /nonexistent \
        --shell /usr/sbin/nologin "$user"
done
```

### 3. Install the agent and its configuration

```sh
install -D -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -d -o root -g blacksmith -m 0750 /etc/blacksmith
install -o root -g blacksmith -m 0640 deploy/agent.env /etc/blacksmith/agent.env
```

The binary belongs to root so the agent can never replace its own executable.
`/etc/blacksmith/agent.env` must exist even if every line is commented out: the defaults are
the production ones. It accepts the `BLACKSMITH_*` variables documented in
[`.env.template`](./.env.template); an unknown one stops the agent at startup. Secrets never go
in this file.

### 4. Install the service templates and polkit

```sh
install -m 0644 deploy/systemd/*.service /etc/systemd/system/
install -m 0644 deploy/blacksmith.service /etc/systemd/system/
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
systemctl daemon-reload
```

Check both before starting the agent:

```sh
sh deploy/polkit/verify.sh            # ends with «regla correcta»
sh deploy/systemd/verify-sandbox.sh   # ends with «Sandbox correcto.»
```

`verify.sh` creates temporary units and checks what the agent may and may not do.
`verify-sandbox.sh` starts a throwaway service with the same sandbox as a real one and checks
that it cannot see other services, the agent's data, D-Bus or other users' processes. Both
clean up after themselves.

### 5. Start the agent

```sh
systemctl enable --now blacksmith.service
systemctl status blacksmith.service
journalctl -u blacksmith.service -n 50
```

The agent listens only on `127.0.0.1:4040` and logs at startup whether polkit authorizes it.
Its database is created at `/var/lib/blacksmith/agent.db`. Do not open port 4040 in the
firewall: nothing outside the server needs it.

### 6. Allow the SSH tunnel

The desktop client reaches the agent through an SSH tunnel to `127.0.0.1:4040`, using an SSH
key of any account on the VPS (it does not need root). The SSH server must allow TCP
forwarding for that account. It does by default; if `/etc/ssh/sshd_config` (or a file in
`sshd_config.d/`) sets `AllowTcpForwarding no`, set it to `local` for that account, for example:

```text
Match User deploy
    AllowTcpForwarding local
```

and `systemctl reload ssh`. On the first connection the client shows the server's key
fingerprint; compare it with `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` before accepting.

### 7. Web sites with nginx (optional)

Needed only for services that serve a web site (static files or a reverse proxy):

```sh
apt install nginx certbot
sh deploy/nginx/install.sh
```

The script installs the shared configuration, the security snippets, the editable templates
in `/etc/blacksmith/nginx/templates/`, and the two units that run as root on the agent's behalf
(validate and reload nginx, request certificates), each guarded by a script that re-validates
what the agent wrote. It needs no domain and can be run again after every update.

If it warns that another `default_server` exists (usually Debian's `default` site), remove it so
that unknown hosts do not receive that site, and run the script again:

```sh
rm /etc/nginx/sites-enabled/default
sh deploy/nginx/install.sh
```

Domains are configured per service, in the client's **Web** tab. Before saving a site, point the
domain's DNS (A/AAAA records) to the VPS and open ports 80 and 443. With HTTPS enabled, the
certificate is requested in the background and renewed by certbot's own timer.

### 8. Deploy on push with GitHub webhooks (optional)

Requires step 7 and a domain of its own for the webhooks (for example `hooks.example.com`) whose
DNS already points to the VPS:

```sh
sh deploy/nginx/install-webhooks.sh hooks.example.com you@example.com
```

The script generates the webhook secret (root-only, handed to the agent as a systemd credential
through a drop-in, so you never edit the unit), obtains the certificate for that domain, publishes
only `POST /api/webhooks/github` and prints the values to enter in GitHub. Then, in each
repository: **Settings → Webhooks → Add webhook**:

- **Payload URL**: `https://hooks.example.com/api/webhooks/github`
- **Content type**: `application/json`
- **Secret**: the value printed by the script (also in `/etc/blacksmith/credentials/github-webhook-secret`)
- **Events**: *Just the push event*

Each push to a branch deploys every service configured with that repository and branch whose
**Deploy on push** switch (in the **Deployment** tab) is on; it is on by default. Requests with
an invalid signature are rejected; the administration API is never published.

If you use your own GitHub App instead of repository webhooks, set the same URL and secret in
its webhook settings. The default BBEL Blacksmith App cannot be used for this, because you do not
control its webhook. Blacksmith never needs the private key of any GitHub App.

When a service uses a deploy key, a GitHub App of your own with **Administration: Read and write**
registers the generated read-only key automatically. With the default BBEL Blacksmith App the
client falls back to the manual copy-and-paste flow, because that permission is deliberately not
granted to it.

### 9. Register and deploy a service

From the desktop client:

1. Add the VPS and connect through SSH.
2. Register a service with a lowercase slug.
3. Select the GitHub repository and branch.
4. Add setup commands such as `python -m venv .venv`, `npm ci`, or `cargo build --release`.
5. Set the service start command, if it has a process.
6. Optionally choose a public directory for a static site, and configure its domain in the
   **Web** tab.
7. Deploy and inspect the service status and logs.

Each deployment gets its own release directory. The previous release remains available for
rollback when a new build or service start fails.

### Updating

Rebuild the agent and repeat the installs, which are idempotent:

```sh
install -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -m 0644 deploy/systemd/*.service deploy/blacksmith.service /etc/systemd/system/
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
systemctl daemon-reload && systemctl restart blacksmith.service
sh deploy/nginx/install.sh                                  # if you use web sites
sh deploy/nginx/install-webhooks.sh hooks.example.com       # if you use webhooks
```

The database is migrated automatically. Your edited nginx templates and the webhook secret are
kept.

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
