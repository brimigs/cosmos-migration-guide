#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export PATH="$repo_root/bin:$PATH"

cd "$repo_root/pinocchio"
exec cargo build-sbf --features bpf-entrypoint "$@"
