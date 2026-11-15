#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$project_dir"

# A normal launch needs only the executable, not Cargo or its build caches.
# Use --build after editing source to explicitly rebuild before launching.
rebuild=false
if [[ "${1:-}" == --build ]]; then
  rebuild=true
  shift
fi
binary="$project_dir/target/release/terramars"
if [[ "$rebuild" == false && -x "$binary" ]]; then
  exec "$binary" "$@"
fi

# Prefer the optional workspace-local toolchain when one was bootstrapped here.
if [[ -x "$project_dir/.tools/cargo/bin/cargo" && -d "$project_dir/.tools/rustup" ]]; then
  export CARGO_HOME="$project_dir/.tools/cargo"
  export RUSTUP_HOME="$project_dir/.tools/rustup"
  cargo_bin="$CARGO_HOME/bin/cargo"
elif command -v cargo >/dev/null 2>&1; then
  cargo_bin="$(command -v cargo)"
else
  echo 'Install Rust with https://rustup.rs/, then run this script again.' >&2
  exit 1
fi

# This optional local linker directory is only needed on development machines
# that have a runtime library but lack its development-package symlink.
if [[ "$(uname -s)" == Linux && -d "$project_dir/.tools/native" ]]; then
  export LIBRARY_PATH="$project_dir/.tools/native${LIBRARY_PATH:+:$LIBRARY_PATH}"
fi

"$cargo_bin" build --locked --release --bin terramars
exec "$binary" "$@"
