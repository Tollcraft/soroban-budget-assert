# Security Policy

## Supported Versions
Currently, only the `main` branch is supported for security updates.

## Reporting a Vulnerability
We take the security of `soroban-budget-assert` seriously. 
If you discover a security vulnerability, please do NOT report it by creating a public GitHub issue or posting in public chat channels.

Instead, please report security vulnerabilities privately using **GitHub Private Vulnerability Reporting**:
- Submit an advisory report via the [Security Advisories page](https://github.com/Tollcraft/soroban-budget-assert/security/advisories/new) or navigate to the repository's **Security** tab and select **"Report a vulnerability"**.

### What to Expect
- **Acknowledgment:** We will acknowledge receipt of your vulnerability report within 48 hours.
- **Assessment & Updates:** We will provide an initial assessment and regular progress updates as we investigate and develop a fix.
- **Public Disclosure:** We will coordinate the disclosure timeline with you once a patch has been verified and released.

### Public Community Channels
For general questions and community discussions, you can reach out via Telegram at `https://t.me/+Gflo5jZStw1jMjE0` or Discord at `https://discord.gg/5aprtMSyR`. Please note that these are public channels and must **not** be used to disclose sensitive security vulnerabilities.

## Audit Status
**DISCLAIMER**: This project is an MVP developer tool and is **UNAUDITED**. 
Do not use this software in production environments to govern significant financial value without conducting your own independent security review and audit.

## Dependency Auditing

Every dependency change is checked against the [RustSec Advisory Database](https://rustsec.org) by [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny), run in CI via [`.github/workflows/security.yml`](.github/workflows/security.yml):

- **On every push and pull request** that touches `Cargo.lock`, any `Cargo.toml`, or the audit configuration itself.
- **On a daily schedule**, since most vulnerabilities are disclosed after the code that depends on them has already merged — a schedule-only trigger is the only way to catch that case.
- **On manual dispatch**, for an on-demand check outside those triggers.

Policy lives in the committed [`deny.toml`](deny.toml): it fails the build on any RustSec vulnerability, unsoundness, or notice advisory; on yanked crates; and on dependency licenses outside the MIT/Apache-2.0-compatible allow-list. A finding is either fixed by updating the dependency, or explicitly accepted by adding its advisory ID to `deny.toml`'s `[advisories].ignore` list with a comment explaining why it does not apply — findings are never silenced just to get CI green.

Run the same check locally with:

```bash
cargo install cargo-deny --locked
cargo deny check
```

