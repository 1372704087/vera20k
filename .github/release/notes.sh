#!/usr/bin/env bash
# Write the release notes for the Release workflow (.github/workflows/release.yml)
# to file $1: a few lines on which installer to run, since a release holds only
# the three installers. Both of its paths use them: a dev build, and a release
# promoted from a dev build (PROMOTED_FROM set to the dev build's tag).
#
# GitHub turns every line break in release notes into a visible one, so each
# paragraph and list item is a single line.
#
# Environment: GITHUB_REPOSITORY, and for a dev build COMMIT (the full sha the
# installers were built from).
set -euo pipefail
out="$1"
{
  if [ -z "${PROMOTED_FROM:-}" ]; then
    echo "**Development build** of commit [\`${COMMIT:0:7}\`](https://github.com/$GITHUB_REPOSITORY/commit/$COMMIT): newer than the latest release, but tested less."
    echo
  fi
  cat <<'EOF'
You need your own copy of Yuri's Revenge.

- **Windows:** run the `-setup.exe`. If SmartScreen warns, click More info, then Run anyway.
- **macOS (Apple Silicon):** open the `.dmg` and drag VERA20K into Applications. The first time, macOS blocks it: click Done, then Open Anyway in System Settings > Privacy & Security.
- **Linux:** make the `.AppImage` executable and start it.
EOF
} > "$out"
cat "$out"
