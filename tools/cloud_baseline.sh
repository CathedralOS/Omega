#!/usr/bin/env bash
# Run a baseline on fetched main, even when the cloud image has old source and mbx.
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
cd "$repo"
remote=origin
if ! git remote get-url origin >/dev/null 2>&1; then
  remote=https://github.com/CathedralOS/Omega.git
fi
# A narrow clone may not track origin/main. Use a private ref so another
# fetch (for example a claim update) cannot replace our commit in FETCH_HEAD.
ref=refs/omega/cloud-baseline/main
git fetch --no-tags --no-write-fetch-head "$remote" "+refs/heads/main:$ref"
commit=$(git rev-parse --verify "$ref^{commit}")

# Each revision has its own sibling checkout so a later fetch cannot move
# the files under a running measurement or inherit the snapshot's Cargo config.
checkout="$(dirname "$repo")/$(basename "$repo")-baseline-$commit"
if [[ ! -e "$checkout" && ! -L "$checkout" ]]; then
  git worktree add --quiet --detach "$checkout" "$commit"
fi
common=$(git rev-parse --path-format=absolute --git-common-dir)
if [[ -L "$checkout" || ! -f "$checkout/.git" ||
      $(git -C "$checkout" rev-parse --show-toplevel) != "$checkout" ||
      $(git -C "$checkout" rev-parse --path-format=absolute --git-common-dir) != "$common" ||
      $(git -C "$checkout" rev-parse HEAD) != "$commit" ]]; then
  echo "cloud baseline: expected a worktree of $repo at $commit in $checkout" >&2
  exit 1
fi
cd "$checkout"
changes=$(git status --porcelain --untracked-files=normal --ignore-submodules=none)
if [[ -n "$changes" ]]; then
  echo "cloud baseline: $checkout contains changes; inspect them before rerunning" >&2
  exit 1
fi

# Install outside PATH shared by other checkouts. The installer verifies its
# recipe-keyed cache before copying the executable, including on repeat runs.
bin="$(git rev-parse --absolute-git-dir)/omega-bin"
bash tools/xtask/bootstrap-mbx.sh --install-dir "$bin" >&2
export PATH="$bin:$PATH"
expected=$(tr -d '[:space:]' < tools/mbx-version)
actual=$(mbx --version)
[[ "$actual" == "mbx $expected" ]] || { echo "cloud baseline: expected mbx $expected, found $actual" >&2; exit 1; }
printf 'cloud baseline: %s | %s | %s | %s\n' "$commit" "$actual" "$(uname -sm)" "$checkout" >&2
if (( $# )); then
  exec "$@"
fi
printf 'cd %q && export PATH=%q:"$PATH"\n' "$checkout" "$bin"
