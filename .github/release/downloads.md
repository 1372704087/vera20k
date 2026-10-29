# Website downloads

The release workflow builds and verifies three installers. After a stable release
is published, `downloads.yml` runs `publish-r2.sh` to mirror those exact files to
Cloudflare R2. It never rebuilds or uploads game data.

The publisher verifies the release tag, GitHub size/SHA-256 and the build
attestation. It uploads immutable `releases/v<version>/<filename>` objects,
reads them back and checks their bytes, then stores `manifest.json` for that
version and `manifest.json.sig`, its Ed25519 signature. The manifest's `notes`
are the release page's change list in plain text, the bullets under its first
"Changes" heading that are more than a link, which the game's update offer
lists. Its `updater` is the number in `updater-version` at the commit the
installers were built from (none before that file), and its `update_key` the
public key that signs it. A retry keeps these fields as the version was first
published, and none for a version mirrored before manifests had them.
Finally it conditionally replaces `latest.json`, only if this is GitHub's
latest stable release and the pointer would not move backward. `latest.json`
holds the same bytes as the version's `manifest.json`, so the website's three
`/download/<platform>` routes read it unchanged.

The game's updater (`src/app/updater`) reads `latest.json`, and installs a
newer version only when `releases/v<version>/manifest.json.sig` verifies those
exact bytes with the key in `update-key.pub` and the download matches the
manifest's size and SHA-256. A newer version whose `updater` is higher than
the game's own `updater-version` it offers as a download by hand from
vera20k.org. One that another key signs, the key its `update_key` names, as
after a key change, it offers as a download from the GitHub releases, showing
only its version: anyone who can write to R2 could sign such a manifest, and
the website's download links follow `latest.json`. Raise `updater-version` in
the change after which older games can no longer install a release themselves,
such as a new installer format. Whatever else later manifests change, they
keep `version`, `tag`, `updater`, `update_key` and `notes` in this form, which
every older game reads.

## Setup and retry

GitHub Actions secrets are `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID` and
`R2_SECRET_ACCESS_KEY`. Use an R2 S3 token with object read/write permission
restricted to the `vera20k-downloads` bucket; bucket administration is unnecessary.
The public custom domain is `downloads.vera20k.org`; r2.dev stays disabled.
Credentials must not appear in code, command arguments or logs.

The secret `UPDATE_SIGNING_KEY` holds the PEM Ed25519 private key whose public
half is `update-key.pub` (base64 of the raw 32 bytes). The publisher refuses to
upload anything when the two do not match. Installed games install only updates
that key signed: after it is replaced, they offer each newer release as a
download from the GitHub releases, until their players install one that
carries the new `update-key.pub`.
To make a key at `$KEY`, a path outside the repository readable only by you,
print its public half and set the secret:

```sh
(umask 077; openssl genpkey -algorithm ed25519 -out "$KEY")
openssl pkey -in "$KEY" -pubout -outform DER | tail -c 32 | openssl base64 -A
gh secret set UPDATE_SIGNING_KEY --repo YuriPlanet/vera20k < "$KEY"
```

The Ubuntu 24.04 runner supplies `gh`, AWS CLI, `jq` and OpenSSL 3. To retry,
dispatch `downloads.yml` with the published stable tag, for example `v0.2.2`.
A local run uses `bash .github/release/publish-r2.sh v0.2.2` with `R2_ACCOUNT_ID`,
`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `UPDATE_SIGNING_KEY`, OpenSSL 3 or
later, and GitHub authentication already in the environment. Never enable shell
tracing.

The initial v0.2.2 build predates this workflow, so dispatch its first mirror
manually on `main`. Later releases whose tagged commit includes the workflow
start it on publication.

Retries reuse existing objects only when their bytes match. A failed upload
leaves the prior latest pointer intact. Workflow concurrency and a local
atomic-directory lock serialize normal runs; S3 conditional writes preserve
objects and competing pointer updates across machines. After an interrupted
local run, inspect a remaining temporary lock before removing it. GitHub and
R2 cannot provide one cross-service transaction; the publisher rechecks the
latest release immediately before updating the pointer.

Tests: `python -m unittest tools.test_release_downloads`; the repository's
`python -m tools.run_tests` includes these local CLI fixtures. Each dev build's
`update-test` job publishes it with this script and a throwaway key
(`tools/update_test.py`), then updates an installed older build to it on every
platform.

References: [R2 tokens](https://developers.cloudflare.com/r2/api/tokens/),
[AWS CLI access](https://developers.cloudflare.com/r2/examples/aws/aws-cli/),
[conditional writes](https://developers.cloudflare.com/r2/api/s3/api/#implemented-object-level-operations).
