#!/usr/bin/env sh
set -eu

ARTIFACT_DIR=${1:?artifact directory required}
OUTPUT_FILE=${2:-SHA256SUMS.txt}
OUTPUT_PATH="$ARTIFACT_DIR/$OUTPUT_FILE"

find "$ARTIFACT_DIR" -maxdepth 1 -type f ! -name "$OUTPUT_FILE" -print0 |
  xargs -0 sha256sum > "$OUTPUT_PATH"

echo "Wrote checksums to $OUTPUT_PATH"
