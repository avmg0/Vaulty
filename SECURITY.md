# Security policy

Vaulty holds people's financial records, so security reports are the most important issues we get.
Thank you for taking the time.

## Reporting a vulnerability

**Please don't open a public issue.** Report privately through GitHub:
[Security → Report a vulnerability](https://github.com/avmg0/Vaulty/security/advisories/new).

Include what you found, how to reproduce it, and what an attacker could do with it. A proof of
concept helps, but please use your own test data (demo mode: `VAULTY_DEMO=1`), never anyone's real data.

## What to expect

- **Within 7 days:** we confirm we've received it.
- **Within 14 days:** we tell you whether we can reproduce it and how serious we think it is.
- **Fixes:** critical and high-severity issues are fixed as soon as possible, aiming for 30 days;
  others in the next planned release.
- **Credit:** once fixed and released, we publish a GitHub security advisory (with a CVE where it
  applies) and credit you, unless you'd rather stay anonymous.

Vaulty is maintained by volunteers, so please allow for that, and give us a reasonable time to fix
an issue before disclosing it publicly. We won't take legal action against good-faith research that
follows this policy.

## Supported versions

Only the latest release receives security fixes. Vaulty has no public release yet; until then,
reports against the `dev` branch are welcome.

## Scope

In scope: the Vaulty app (Rust core, desktop shell, interface), its file formats (backups,
imports), its build and release process.

Out of scope: problems that require an already-compromised device while a profile is unlocked,
and anything listed under "Where protection ends" in [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).
Those are known limits, though ideas for narrowing them are welcome as normal issues.
