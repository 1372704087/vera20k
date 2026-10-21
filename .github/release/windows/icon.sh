#!/usr/bin/env bash
# Render .github/release/icon.svg into the Windows images beside this script:
# vera20k.ico, the icon of the program (build.rs) and of its installer
# (vera20k.iss), and wizard-small.png, the image on the installer's pages. Both
# are committed; run this on macOS after changing icon.svg. sips renders the
# SVG, as app.sh does for the Mac app.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Windows icons reach nearly to the edge of their square, so drop most of the
# margin macOS's icon grid keeps around the tile: 20 px of 864 remain.
sed 's/viewBox="0 0 1024 1024"/viewBox="80 80 864 864"/' "$here/../icon.svg" > "$work/icon.svg"
grep -q 'viewBox="80 80 864 864"' "$work/icon.svg"

# The sizes Windows draws an app icon at, from small list icons to 256 px tiles.
images=()
for size in 16 20 24 32 40 48 64 96 256; do
  sips -s format png -z "$size" "$size" "$work/icon.svg" --out "$work/$size.png" > /dev/null
  images+=("$work/$size.png")
done
# The installer shrinks this to its image area, 58 px at 100% scaling and
# 159 px at 250% (Inno Setup's WizardSmallImageFile).
sips -s format png -z 159 159 "$work/icon.svg" --out "$here/wizard-small.png" > /dev/null

# An icon file is a header, a directory entry per image, then the images. Each
# image here is a whole PNG file, which Windows reads at any size.
python3 - "$here/vera20k.ico" "${images[@]}" <<'EOF'
import struct
import sys

output, *paths = sys.argv[1:]
images = []
for path in paths:
    with open(path, "rb") as file:
        data = file.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        sys.exit(f"{path} is not a PNG file")
    width, height = struct.unpack(">II", data[16:24])
    images.append((width, height, data))

icon = bytearray(struct.pack("<HHH", 0, 1, len(images)))
offset = len(icon) + 16 * len(images)
for width, height, data in images:
    # A width or height of 256 is stored as 0.
    icon += struct.pack("<BBBBHHII", width % 256, height % 256, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
for _, _, data in images:
    icon += data
with open(output, "wb") as file:
    file.write(icon)
EOF
