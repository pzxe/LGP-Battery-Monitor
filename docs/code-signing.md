# Code signing status

**Current status: releases are NOT code-signed.** Windows SmartScreen and some antivirus
products may warn about the downloaded files ("Windows protected your PC" / "Unknown publisher").
Verify downloads with the published SHA256 checksums instead.

## Options evaluated

| Option | Cost | Fit |
|---|---|---|
| [SignPath Foundation](https://signpath.org/) free certificate for open-source projects | Free | Best fit. Requires an application and approval; the project must be public, OSI-licensed, actively maintained, and builds must come from CI. |
| Self-signed certificate | Free | Rejected: not trusted by Windows and would give users a false sense of safety. |
| Commercial OV/EV certificate | Paid | Not used. |

## What is prepared

- Public repository with an OSI license (MIT) and CI-built releases (`.github/workflows/release.yml`).
- A disabled `sign` job in the release workflow that submits the unsigned artifact to SignPath.
  It runs only when the repository variable `SIGNPATH_ENABLED` is `true`. **It has never been run**,
  and the exact inputs may need adjusting to the policy SignPath assigns.

## What remains (requires the maintainer)

1. Apply at <https://signpath.org/apply> (needs a human to submit the form and accept the terms).
2. After approval: create the project (`lgp-battery-monitor`) and signing policy, add the repository
   secret `SIGNPATH_API_TOKEN` and variables `SIGNPATH_ORGANIZATION_ID` / `SIGNPATH_ENABLED`.
3. Re-run a release, confirm the signature on the executable, and only then update the README.

Even with a signature, SmartScreen reputation builds over time; warnings may not disappear immediately.
