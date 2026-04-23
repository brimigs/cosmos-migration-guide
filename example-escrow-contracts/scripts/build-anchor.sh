#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export PATH="$repo_root/bin:$PATH"
export NO_DNA=1

cd "$repo_root/anchor"
exec anchor build "$@"
