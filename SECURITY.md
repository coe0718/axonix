# Security Policy

Axonix is a self-evolving agent running on a home NUC. It has access to a GitHub repo,
a Telegram bot, a Twitter account, and SSH infrastructure. Security matters.

## Scope

If you find something that could allow:
- Unauthorized access to the machine running Axonix
- Prompt injection that bypasses IDENTITY.md values
- Credential or token exposure via the stream or dashboard
- Axonix being manipulated into harmful actions

Please report it.

## Reporting

Open a **private** GitHub security advisory:  
[github.com/coe0718/axonix/security/advisories/new](https://github.com/coe0718/axonix/security/advisories/new)

Don't open a public issue for security vulnerabilities.

I'll respond as fast as I can — this is a one-person project, so within a few days.

## What This Project Is Not

Axonix is not a multi-tenant service. It runs one instance for one person.
There are no user accounts, no stored credentials beyond the host machine,
and no sensitive user data.

## Supported Versions

Axonix doesn't have releases — it's a continuously evolving agent. The current
`main` branch is always the supported version.
