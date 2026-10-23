# Security Policy

## Supported versions

Security fixes are currently provided for the following release line:

| Version | Supported |
| --- | --- |
| `v1.0.x` | Yes |
| `v1.0.0` | Yes |
| Older releases | No |
| Development snapshots and unreleased builds | No |

The supported version is `v1.0.0` and later patch releases in the `v1.0.x` line. Users should
upgrade to the latest available patch release before reporting a problem whenever possible.

This policy may change as new release lines are published. The repository's release notes and
tags are the source of truth for supported versions.

## Reporting a vulnerability

Please do not report security vulnerabilities in a public GitHub issue, pull request, discussion,
or chat message.

Use GitHub's private vulnerability reporting flow for
[HormigaDev/blacksmith](https://github.com/HormigaDev/blacksmith/security/advisories/new). This
keeps the report visible only to the maintainers and invited security collaborators while it is
being investigated.

If private vulnerability reporting is unavailable, contact the project maintainers privately
through the repository owner's GitHub profile and include the subject `Blacksmith security
report`. Do not disclose sensitive details until the maintainers confirm a private channel.

Reports should include enough information to reproduce and assess the issue:

- The affected Blacksmith version, commit, or package
- The affected component, such as the agent, Tauri client, deployment units, nginx integration,
  webhook handling, or shared types
- The operating system and relevant versions of Rust, systemd, nginx, or other dependencies
- A concise description of the security impact and the suspected attack path
- Reproduction steps, a minimal proof of concept, or sanitized logs
- Any known mitigations or whether the issue is already being exploited

Please remove passwords, private keys, webhook secrets, access tokens, personal data, hostnames,
IP addresses, and other credentials from the report. If evidence must contain sensitive data,
say so in the initial message and wait for instructions.

## What to expect

The maintainers will try to:

1. Acknowledge receipt within 7 calendar days.
2. Triage the report and request clarification when necessary.
3. Confirm whether the issue affects a supported version.
4. Coordinate a fix, mitigation, or disclosure plan with the reporter.
5. Publish a security advisory or release note when disclosure is appropriate.

These are response targets, not guaranteed deadlines. The project is maintained on a best-effort
basis, and complex issues or issues involving upstream dependencies may require more time.

Please allow the maintainers reasonable time to investigate and release a fix before making
the report public. We will credit reporters when they want public credit and it is safe to do
so.

## Scope and operational security

Blacksmith manages systemd services, deployment commands, environment files, SSH connections,
polkit rules, nginx configuration, and host resources. A secure implementation cannot compensate
for an unsafe deployment.

Operators are responsible for reviewing and correctly applying the deployment files, service
accounts, filesystem permissions, firewall rules, SSH configuration, host-key verification,
GitHub credentials, webhook secrets, TLS configuration, and commands executed during a
deployment. Do not expose the agent's administration port to the public Internet, grant broader
privileges than necessary, or run deployment commands that you have not reviewed.

The three project READMEs state the same limitation: Blacksmith is intended for people who
understand the systems they are administering. No tool remains secure when it is configured
incorrectly or when its administration surface and credentials are deliberately exposed.
Read the security model and deployment instructions before using Blacksmith on a production
host.

## Out of scope

The following generally do not qualify as vulnerabilities in Blacksmith by themselves:

- A deliberately insecure application deployed through Blacksmith
- A deployment command or systemd unit configured unsafely by an operator
- Missing hardening that is outside the documented supported configuration
- Vulnerabilities in a user's operating system, VPS provider, GitHub account, repository, or
  application dependencies
- Publicly disclosed vulnerabilities in an unsupported Blacksmith version
- Reports that require access to credentials or a host already compromised by the reporter

We may still use these reports to improve documentation or secure defaults.

## Disclosure and legal notice

This policy describes the project's intended security response; it does not create a service
level agreement, warranty, or guarantee of a fix. Blacksmith is distributed under the
[MIT License](./LICENSE), including its `AS IS` warranty disclaimer and limitation of
liability. Those terms describe the license under which the software is provided; they do not
replace the operator's responsibility to assess the software, secure the host, comply with
applicable law, and obtain professional advice where appropriate.

Thank you for reporting security issues privately and giving maintainers a reasonable opportunity
to protect users before public disclosure.
