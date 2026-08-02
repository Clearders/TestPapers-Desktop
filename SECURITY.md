# Security Policy

## Supported versions

This repository contains a governance baseline and has no released Desktop application yet. Supported versions will be listed here when the first signed build is published.

## Reporting a vulnerability

Do not open a public issue for suspected vulnerabilities or exposed credentials. Use GitHub private vulnerability reporting for this repository. Include affected versions or commits, reproduction steps, impact, and any suggested mitigation.

Maintainers will acknowledge a report, assess severity, coordinate remediation, and publish an advisory when appropriate. Avoid accessing data that is not yours and allow a reasonable remediation window before disclosure.

## Credential and trust boundaries

- Never commit signing certificates, updater private keys, access/refresh tokens, production `.env` files, database snapshots, or user documents.
- Desktop secrets must use operating-system secure storage when implemented.
- The Rust Local Engine is the trust boundary for SQLite, filesystem access, credentials, and Cloud synchronization.
- The Desktop UI must not expose generic SQL, shell, or unrestricted filesystem IPC.
- Cloud persistence and worker internals are not Desktop dependencies.

If a secret is committed, revoke and rotate it immediately; deleting it from the latest commit is not sufficient.
