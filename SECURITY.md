# Security Policy

## Reporting a Vulnerability

Antra takes security seriously. If you believe you have found a security
vulnerability in Antra, please **do not** file a public GitHub issue.

Send a private report instead, so the issue can be fixed before it is
disclosed:

- **Email:** (maintainer email — see the GitHub profile for `ifelse-codes`)
- **GitHub:** use the [Security advisory](https://github.com/ifelse-codes/antra/security/advisories/new)
  form to create a private vulnerability report.

We aim to acknowledge reports within **3 business days** and to provide an
initial assessment within **7 business days**. You will be kept informed of
the status of your report as it is triaged and fixed.

## Security model

Antra is a local development tool. Its two trust-sensitive surfaces are:

1. **Local Certificate Authority (CA)** — Antra generates a CA on your machine
   and, with your consent, installs it into the system trust store. Anyone
   with access to the CA private key can issue certificates that your machine
   trusts. The key is stored locally and should never leave your device.
2. **Hosts file management** — Antra writes managed entries to your OS hosts
   file, scoped to a `# BEGIN ANTRA MANAGED HOSTS` block)Skip. These changes are
   reversible via `antra clean` / `antra trust --remove`.

Neither surface involves a network service controlled by the project. Antra
has no telemetry, no accounts, and no cloud dependency.

## Scope

The following are **out of scope** and are not eligible for disclosure under
this policy:

- Theft or loss of the local CA private key due to compromise of the host
  machine itself (inherent to any local trust store).
- Social engineering of the user.
- Vulnerabilities in third-party dependencies already fixed upstream; please
  report those to the upstream project.

## Coordinating public disclosure

We appreciate coordinated disclosure. We will work with you to agree on a
timeline before public release once a fix is available. We will credit
researchers in the release notes when a confirmed vulnerability is reported
responsibly.
