set -euo pipefail

IMAGE_NAME="homeslashmusic-builder"

# Build the container image automatically if it doesn't exist yet (or if --rebuild is passed)
if ! podman image exists "$IMAGE_NAME" || [[ "${1:-}" == "--rebuild" ]]; then
  echo "Building $IMAGE_NAME..."
  podman build -t "$IMAGE_NAME" .
  # Shift off --rebuild if passed so remaining args pass cleanly to cargo
  if [[ "${1:-}" == "--rebuild" ]]; then
    shift
  fi
fi

# Default to `cargo build --release` unless custom cargo args are passed
if [ $# -eq 0 ]; then
  CARGO_ARGS=("build" "--release")
else
  CARGO_ARGS=("$@")
fi

echo "Compiling inside $IMAGE_NAME..."
podman run --rm \
  -v "$PWD:/app" \
  -v cargo-cache:/usr/local/cargo/registry \
  -w /app \
  "$IMAGE_NAME" \
  cargo "${CARGO_ARGS[@]}" --target-dir target-podman

echo "Done! Output available in ./target-podman/"
