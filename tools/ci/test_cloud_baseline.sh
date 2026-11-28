#!/usr/bin/env bash
# Exercise the launcher against a real Git remote without building a Rust tool.
set -euo pipefail
launcher=$(cd "$(dirname "$0")/.." && pwd)/cloud_baseline.sh
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
cd "$scratch"
git init -q -b main upstream
cd upstream
git config user.name Test
git config user.email test@example.org
mkdir -p tools/xtask
echo 1.21.0 > tools/mbx-version
git add tools && git commit -qm snapshot
cd ..
git clone -q --single-branch --branch main upstream 'old snapshot'
cd upstream
echo 1.29.2-omega.3 > tools/mbx-version
cat > tools/xtask/bootstrap-mbx.sh <<'INSTALL'
set -eu
[[ $1 == --install-dir ]]
mkdir -p "$2"
printf '#!/bin/sh\necho "mbx %s"\n' "$(cat tools/mbx-version)" > "$2/mbx"
chmod +x "$2/mbx"
INSTALL
git add tools && git commit -qm 'new pin and installer'
expected=$(git rev-parse HEAD)
cd '../old snapshot'
old=$(git rev-parse HEAD)
echo 'local work' >> tools/mbx-version
mkdir "$scratch/old-bin"
printf '#!/bin/sh\necho "mbx 1.21.0"\n' > "$scratch/old-bin/mbx"
chmod +x "$scratch/old-bin/mbx"
export PATH="$scratch/old-bin:$PATH"
# Cloud snapshots may have fetched only a task branch.
git config remote.origin.fetch '+refs/heads/task:refs/remotes/origin/task'
run() { bash "$launcher" "$@"; }
baseline=$(run pwd)
[[ $(run git rev-parse HEAD) == "$expected" && $(run mbx --version) == 'mbx 1.29.2-omega.3' ]]
[[ $(run pwd) == "$baseline" && $(git rev-parse HEAD) == "$old" && $(mbx --version) == 'mbx 1.21.0' ]]
[[ $(tail -1 tools/mbx-version) == 'local work' ]]
if run sh -c 'exit 17'; then exit 1; else [[ $? == 17 ]]; fi
touch "$baseline/measurement.txt"
if run true; then echo 'used a dirty baseline' >&2; exit 1; fi
git -C ../upstream commit --allow-empty -qm 'new main'
next=$(run pwd)
[[ "$next" != "$baseline" && -f "$baseline/measurement.txt" ]]
[[ $(git -C "$next" rev-parse HEAD) == "$(git -C ../upstream rev-parse HEAD)" ]]
git remote set-url origin "$scratch/missing"
if run true; then echo 'used a cached ref after a failed fetch' >&2; exit 1; fi
# Route the public fallback to the fixture to cover snapshots without a remote.
git remote remove origin
git config url."$scratch/upstream".insteadOf https://github.com/CathedralOS/Omega.git
[[ $(run pwd) == "$next" ]]
echo 'cloud baseline tests passed'
