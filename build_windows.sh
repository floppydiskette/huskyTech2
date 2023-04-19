#!/usr/bin/env bash

# the I_PREFER variable should specify either "docker" or "podman", but if not set, it will default to "podman"
if [ -z "$I_PREFER" ]; then
  I_PREFER="podman"
fi

if [ -z "$CONTAINER_NAME" ]; then
  CONTAINER_NAME="ht2_building_container_win64"
fi

WORKING_DIR=$(realpath "$(dirname "$0")")
echo "creating container $CONTAINER_NAME"
"$I_PREFER" build -t "$CONTAINER_NAME" .

echo "running container $CONTAINER_NAME with cargo build --release --target x86_64-pc-windows-msvc"
"$I_PREFER" run --rm -it -v "$WORKING_DIR":/ht2 -w=/ht2 "$CONTAINER_NAME" cargo build --release --target x86_64-pc-windows-msvc --no-default0-features --features "graphical"