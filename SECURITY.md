# Security Policy

## Supported Versions

We support the latest released version of RAGit on the `main` branch.

## Reporting a Vulnerability

Please report security issues **privately** — do not open a public issue.

- Email the maintainers (see the repo description for contact), or
- Use GitHub's private vulnerability reporting if enabled.

Include:
- A description of the vulnerability and impact
- Steps to reproduce
- Any suggested mitigation

We aim to acknowledge reports within 7 days and provide a fix or mitigation plan.

## Notes on Local-First Design

- RAGit runs models **locally**; your files and prompts do not leave your machine in
  **Local** mode.
- **Team Mode** exposes an HTTP server on `0.0.0.0` for LAN sharing. It uses HMAC-signed
  session tokens and Argon2 password hashing, but **no TLS**. Only enable Team Mode on a
  trusted network, and consider a reverse proxy with TLS for untrusted environments.
