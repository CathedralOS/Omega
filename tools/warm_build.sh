#!/bin/bash
# Start managed cloud sessions with the compiler already built.
#
# A cloud session is a fresh 4-CPU container, and building the compiler from
# nothing there takes about 11 minutes. CI on main builds the same compiler in
# an identical container and publishes the build directory to the GitHub
# Container Registry (ghcr.io/cathedralos/omega-warm-build, one tag per slot);
# a session unpacks it before its first build and compiles only what changed
# since. The archive is about 300 MB, so the restore takes seconds.
#
#   bash tools/warm_build.sh pack <slot> <out-dir>   # CI: archive ./target
#   bash tools/warm_build.sh publish <slot> <out-dir> # CI: push it to the registry
#   bash tools/warm_build.sh restore <slot> [replace-stale] # unpack into the slot
#   bash tools/warm_build.sh session-start           # SessionStart hook, cloud only
#
# mbx cannot do this job here. This workspace builds with Cargo's checksum
# freshness (.cargo/config.toml), which passes `-Zchecksum-hash-algorithm` to
# rustc, and mbx 1.29.2 does not cache a compilation with a flag it does not
# model: every unit is reported as bypassed and its store stays empty. The
# same setting is what makes a plain directory restore sound: Cargo compares
# source contents, not file times, so a fresh clone reuses a restored build and
# recompiles exactly the crates whose sources differ. Build scripts are the
# exception: Cargo still dates their inputs by modification time, which
# settle_source_times handles.
#
# There are two slots because Cargo gives the two places a session builds in
# different identities. A pool worktree (.agents/worktree/worker-N) sits inside
# the primary checkout, so Cargo reads both .cargo/config.toml files and passes
# the rustflags twice; a build made in one place is never reused in the other.
#   primary  the checkout itself, where `cargo xtask worktree acquire` first runs
#   worker   .agents/worktree/worker-1, the first pool slot
# CI builds each slot in a container set up like a session: as root, at the
# same paths, with the tools tools/agent_setup.sh installs.
#
# The restore pays off only at the published commit. Most of the 11 minutes is
# this workspace's own 89 crates, and a change to a foundation crate such as
# language-core recompiles all of them: a build restored three commits away
# from its sources took 673 s, as long as a cold one. So session-start creates
# worker-1 at the published commit, `cargo xtask worktree acquire` hands it out
# there, and the work keeps that commit as its base; the merge queue
# integrates it with main.
set -euo pipefail

REPOSITORY=CathedralOS/Omega
# The build is stored as an OCI artifact, not a release: releases are the
# product, and installers resolve the newest one. Each slot is a tag whose
# artifact holds the archive and its manifest as two titled layers.
REGISTRY=https://ghcr.io
PACKAGE=cathedralos/omega-warm-build
# The release the build lived on before the registry. Read only when the
# registry has nothing, until the first registry publication.
RELEASE=https://github.com/$REPOSITORY/releases/download/warm-build-linux-x86_64
# A local directory served as file:// stands in for the registry in tests.
SOURCE=${OMEGA_WARM_BUILD_SOURCE:-}
SCHEMA=omega-warm-build-v2
# Room the first build needs beyond the unpacked archive.
HEADROOM_KIB=$((2 * 1024 * 1024))

fail() { echo "warm build: $*" >&2; exit 1; }

primary_root() {
  local common
  common=$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --path-format=absolute --git-common-dir)
  dirname "$common"
}

slot_dir() {
  case "$1" in
    primary) primary_root ;;
    worker) echo "$(primary_root)/.agents/worktree/worker-1" ;;
    *) fail "unknown slot '$1' (primary or worker)" ;;
  esac
}

# The exact compiler: artifacts from any other one are rebuilt, never reused.
toolchain() { (cd "$1" && rustc -V); }

manifest_value() { sed -n "s/^$2=//p" "$1" | head -n 1; }

pack() {
  local slot=$1 out=$2 dir commit asset
  dir=$(slot_dir "$slot")
  [ "$(pwd -P)" = "$dir" ] || fail "pack $slot runs in $dir, the path sessions build at"
  [ -x target/debug/omega ] || fail "target/debug/omega is missing; build before packing"
  commit=$(git rev-parse HEAD)
  asset=target-$slot-${commit:0:12}.tar.zst
  mkdir -p "$out"
  # pax headers keep nanosecond file times; Cargo compares outputs by them.
  tar --format=posix -cf - target | zstd -T0 -6 -q -f -o "$out/$asset"
  {
    echo "schema=$SCHEMA"
    echo "slot=$slot"
    echo "commit=$commit"
    echo "root=$dir"
    echo "rustc=$(toolchain "$dir")"
    echo "asset=$asset"
    echo "sha256=$(sha256sum "$out/$asset" | cut -d' ' -f1)"
    echo "unpacked_kib=$(du -sk target | cut -f1)"
  } > "$out/warm-build-$slot.txt"
  cat "$out/warm-build-$slot.txt"
}

download() {
  # Downloads through the session proxy are sometimes cut off partway; -C -
  # resumes at the next unread byte instead of starting over. A redirect to the
  # registry's blob store does not carry the Authorization header along.
  local dest=$1; shift
  curl -fsSL --retry 5 --retry-all-errors --retry-delay 2 --connect-timeout 20 -C - -o "$dest" "$@"
}

# The digest of the layer titled $2 in the image manifest $1. `publish` writes
# one layer per line, and the registry serves a manifest byte for byte.
layer_digest() {
  grep -F "\"org.opencontainers.image.title\":\"$2\"" "$1" |
    sed -n 's/.*"digest":"\(sha256:[0-9a-f]\{64\}\)".*/\1/p' | head -n 1
}

# fetch <slot> <file> <dest>: one file of a slot's published build.
fetch() {
  local slot=$1 name=$2 dest=$3 token digest manifest=$3.oci-manifest
  if [ -n "$SOURCE" ]; then download "$dest" "$SOURCE/$name"; return; fi
  token=$(curl -fsS --retry 3 --connect-timeout 20 "$REGISTRY/token?scope=repository:$PACKAGE:pull" |
    sed -n 's/.*"token":"\([^"]*\)".*/\1/p') || token=
  if [ -n "$token" ] &&
    # Silent: until the first registry publication the tag does not exist.
    curl -fs --retry 3 --connect-timeout 20 -o "$manifest" -H "Authorization: Bearer $token" \
      -H 'Accept: application/vnd.oci.image.manifest.v1+json' "$REGISTRY/v2/$PACKAGE/manifests/$slot" &&
    digest=$(layer_digest "$manifest" "$name") && [ -n "$digest" ]; then
    download "$dest" -H "Authorization: Bearer $token" "$REGISTRY/v2/$PACKAGE/blobs/$digest"
    return
  fi
  download "$dest" "$RELEASE/$name"
}

# publish <slot> <out-dir>: push what `pack` wrote as the slot's tag. Blobs go
# up before the manifest that names them, so a reader never sees a tag whose
# files are missing. Needs GH_TOKEN with packages: write.
publish() {
  local slot=$1 out=$2 token file name sum size type location layers='' manifest
  token=$(curl -fsS -u "x-access-token:$GH_TOKEN" \
    "$REGISTRY/token?service=ghcr.io&scope=repository:$PACKAGE:push,pull" |
    sed -n 's/.*"token":"\([^"]*\)".*/\1/p')
  [ -n "$token" ] || fail "publish: the registry refused the token"
  # OCI artifacts carry an empty config blob.
  printf '{}' > "$out/empty.json"
  for file in "$out/empty.json" "$out/$(manifest_value "$out/warm-build-$slot.txt" asset)" \
    "$out/warm-build-$slot.txt"; do
    sum=$(sha256sum "$file" | cut -d' ' -f1)
    if ! curl -fsS -I -o /dev/null -H "Authorization: Bearer $token" "$REGISTRY/v2/$PACKAGE/blobs/sha256:$sum"; then
      location=$(curl -fsS -X POST -D - -o /dev/null -H "Authorization: Bearer $token" \
        "$REGISTRY/v2/$PACKAGE/blobs/uploads/" | sed -n 's/^[Ll]ocation: *//p' | tr -d '\r')
      [ -n "$location" ] || fail "publish: the registry gave no upload location"
      case $location in /*) location=$REGISTRY$location ;; esac
      case $location in *\?*) location="$location&digest=sha256:$sum" ;; *) location="$location?digest=sha256:$sum" ;; esac
      curl -fsS -T "$file" -o /dev/null -H "Authorization: Bearer $token" \
        -H 'Content-Type: application/octet-stream' "$location" ||
        fail "publish: upload of $(basename "$file") failed"
    fi
    name=$(basename "$file")
    size=$(stat -c %s "$file")
    case $name in
      empty.json) continue ;;
      *.zst) type=application/zstd ;;
      *) type=text/plain ;;
    esac
    layers="$layers${layers:+,
}{\"mediaType\":\"$type\",\"digest\":\"sha256:$sum\",\"size\":$size,\"annotations\":{\"org.opencontainers.image.title\":\"$name\"}}"
  done
  manifest=$out/oci-manifest-$slot.json
  cat > "$manifest" <<JSON
{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","artifactType":"application/vnd.cathedralos.omega.warm-build.v2",
"config":{"mediaType":"application/vnd.oci.empty.v1+json","digest":"sha256:$(sha256sum "$out/empty.json" | cut -d' ' -f1)","size":2},
"layers":[
$layers
],
"annotations":{"org.opencontainers.image.source":"https://github.com/$REPOSITORY","org.opencontainers.image.description":"Prebuilt omega target directory for Claude Code cloud sessions; see tools/warm_build.sh."}}
JSON
  curl -fsS -X PUT -o /dev/null -H "Authorization: Bearer $token" \
    -H 'Content-Type: application/vnd.oci.image.manifest.v1+json' \
    --data-binary "@$manifest" "$REGISTRY/v2/$PACKAGE/manifests/$slot" ||
    fail "publish: the registry refused the $slot manifest"
  echo "warm build: published $slot as ghcr.io/$PACKAGE:$slot"
}

# stale_restore <slot> <dir> <published-commit>: whether $dir/target is a
# restored build of another commit that nothing has built on, so a newer
# published build may replace it. A cloud environment can start a session from
# a saved container that still holds an old restore under .agents/, which Git
# ignores. Kept, it is reused for nothing: a build tens of commits away from
# its sources recompiles every crate. The worker slot also needs its checkout
# untouched at that commit, since replacing the build moves the checkout.
stale_restore() {
  local slot=$1 dir=$2 published=$3 stamp=$2/target/.warm-build.txt old
  [ -f "$stamp" ] || return 1
  old=$(manifest_value "$stamp" commit)
  [ -n "$old" ] && [ "$old" != "$published" ] || return 1
  # The stamp is written last; any build since then leaves a newer file.
  [ -z "$(find "$dir/target" -newer "$stamp" -print -quit 2>/dev/null)" ] || return 1
  if [ "$slot" = worker ]; then
    [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null)" = "$old" ] || return 1
    [ -z "$(git -C "$dir" status --porcelain 2>/dev/null)" ] || return 1
  fi
}

# settle_source_times <dir> <published-commit>: date the checkout's tracked
# files back to the published commit, so the restored build is not rerun.
# Checksum freshness covers what rustc reads, but Cargo still judges a build
# script's rerun-if-changed inputs by modification time. The compiler crate's
# build script watches every file under omega-rust/ and source/library/, and a
# fresh clone writes all of them after CI built: without this, the first build
# reruns that script and recompiles compiler, package-manager, orchestration
# and omega (21-35 s). Only a clean checkout at the published commit holds the
# exact bytes that build read, so any other checkout keeps its real times.
settle_source_times() {
  local dir=$1 commit=$2 when
  [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null)" = "$commit" ] || return 0
  [ -z "$(git -C "$dir" status --porcelain --untracked-files=all 2>/dev/null)" ] || return 0
  # The commit exists before CI starts building it, so its time is older than
  # every timestamp the build recorded.
  when=$(git -C "$dir" log -1 --format=%ct "$commit") || return 0
  (cd "$dir" && git ls-files -z | xargs -0 -r touch -h -c -d "@$when" &&
    git ls-files -z | xargs -0 -r dirname -z | sort -zu | xargs -0 -r touch -h -c -d "@$when") ||
    echo "warm build: could not date $dir back to ${commit:0:12}; the first build reruns the compiler build script"
}

# restore <slot> [replace-stale]
restore() (
  local slot=$1 replace=${2:-} root dir stage manifest commit asset want have need free started
  local built_at old=''
  started=$(date +%s)
  root=$(primary_root)
  dir=$(slot_dir "$slot")
  stage=$root/.agents/worktree/.warm-$slot.$$
  mkdir -p "$stage"
  trap 'rm -rf "$stage"' EXIT
  manifest=$stage/manifest.txt
  fetch "$slot" "warm-build-$slot.txt" "$manifest" || fail "$slot: no published build"
  [ "$(manifest_value "$manifest" schema)" = "$SCHEMA" ] || fail "$slot: unknown manifest schema"
  commit=$(manifest_value "$manifest" commit)
  # Cargo records the absolute paths of files a crate reads from outside its
  # package (language-core includes source/library/*.omg), so a build made at
  # another path recompiles every workspace crate. Restoring one would cost
  # the download and still leave the full 9-minute build.
  built_at=$(manifest_value "$manifest" root)
  [ -z "$built_at" ] || [ "$built_at" = "$dir" ] ||
    fail "$slot: built at $built_at, this session builds at $dir; Cargo would recompile every crate"
  # Never unpack over a build: another restore or a session's own build owns
  # it. The one exception is an untouched restore of an older commit.
  if [ -d "$dir/target" ] && [ -n "$(ls -A "$dir/target/")" ]; then
    if [ "$replace" != replace-stale ] || ! stale_restore "$slot" "$dir" "$commit"; then
      echo "warm build: $slot kept, $dir/target already holds a build"
      return 0
    fi
    old=$(manifest_value "$dir/target/.warm-build.txt" commit)
    if [ "$slot" = worker ]; then
      git -C "$root" cat-file -e "$commit^{commit}" 2>/dev/null ||
        git -C "$root" fetch -q origin "$commit" 2>/dev/null ||
        { echo "warm build: $slot kept; commit ${commit:0:12} is not in this clone"; return 0; }
    fi
  fi
  if [ ! -e "$dir" ]; then
    # The pool's first slot does not exist yet. Create it at the commit the
    # build came from, where the build is reused whole; `cargo xtask worktree
    # acquire` keeps it there. A clone too shallow to hold that commit gets
    # its own HEAD, and the first build recompiles what differs.
    local base=HEAD
    if git -C "$root" cat-file -e "$commit^{commit}" 2>/dev/null; then base=$commit; fi
    git -C "$root" worktree add -q --detach "$dir" "$base"
  fi
  asset=$(manifest_value "$manifest" asset)
  want=$(manifest_value "$manifest" rustc)
  have=$(toolchain "$dir")
  [ "$want" = "$have" ] || fail "$slot: built with '$want', this checkout uses '$have'"
  need=$(($(manifest_value "$manifest" unpacked_kib) + HEADROOM_KIB))
  free=$(df -Pk "$dir" | awk 'NR==2 {print $4}')
  [ "$free" -ge "$need" ] || fail "$slot: needs $need KiB free, the disk has $free KiB"
  fetch "$slot" "$asset" "$stage/$asset" || fail "$slot: download of $asset failed"
  echo "$(manifest_value "$manifest" sha256)  $stage/$asset" | sha256sum --check --status ||
    fail "$slot: $asset does not match its published digest"
  zstd -dc "$stage/$asset" | tar -xf - -C "$stage"
  cp "$manifest" "$stage/target/.warm-build.txt"
  if [ -n "$old" ]; then
    # The stale build goes into the stage, which the exit trap deletes.
    mv -T "$dir/target" "$stage/stale-target" ||
      fail "$slot: could not move the stale build of ${old:0:12} aside"
    if [ "$slot" = worker ]; then
      git -C "$dir" checkout -q --detach "$commit" ||
        fail "$slot: could not move $dir to ${commit:0:12}"
    fi
    echo "warm build: $slot replaced the untouched build of ${old:0:12}"
  fi
  # One rename publishes the whole build. It fails if a build appeared in the
  # meantime, and that build then stays untouched.
  # mbx leaves `target` as a link into its cache. After its collector removes
  # that directory the link dangles, and a rename cannot replace a link.
  if [ -L "$dir/target" ] && [ ! -e "$dir/target" ]; then rm -f "$dir/target"; fi
  rmdir "$dir/target" 2>/dev/null || true
  mv -T "$stage/target" "$dir/target" 2>/dev/null ||
    fail "$slot: $dir/target appeared during the restore; left as it is"
  settle_source_times "$dir" "$commit"
  echo "warm build: $slot restored commit ${commit:0:12} into $dir/target in $(($(date +%s) - started)) s"
)

session_start() {
  # Other hosts keep their builds between sessions and have nothing to restore.
  # CODEX_ENVIRONMENT_ID identifies a managed environment; ordinary local Codex
  # sessions must not restore a cloud build or change their cache settings.
  [ "${CLAUDE_CODE_REMOTE:-}" = "true" ] || [ -n "${CODEX_ENVIRONMENT_ID:-}" ] || return 0
  # Claude Code passes the hook's input as JSON on stdin. Only a fresh start
  # may replace a stale restore; a resumed session's build is its own.
  local input='' replace=replace-stale
  [ -t 0 ] || input=$(timeout 2 cat 2>/dev/null || true)
  case $input in *'"source":"resume"'* | *'"source": "resume"'*) replace='' ;; esac
  # mbx deletes build directories, mid-build included, when free space falls
  # under 10% of the disk's size. The container's disk reports 252 GB while the
  # session may use about 30 GB of it, so that floor is 25 GiB and one compiler
  # build crosses it. 5 GiB is mbx's own minimum for a small disk.
  if command -v mbx >/dev/null && [ -z "$(mbx settings get gc.min_free_size 2>/dev/null)" ]; then
    mbx settings set gc.min_free_size 5GiB >/dev/null 2>&1 || true
  fi
  # Both at once: each is a 270 MB download, and the session waits for this hook.
  local slot
  for slot in primary worker; do
    { restore "$slot" "$replace" 2>&1 ||
      echo "warm build: $slot not restored; the first build compiles everything"; } &
  done
  wait
  local worker stamp commit
  worker=$(slot_dir worker)
  stamp=$worker/target/.warm-build.txt
  [ -f "$stamp" ] || return 0
  commit=$(manifest_value "$stamp" commit)
  if [ "$(git -C "$worker" rev-parse HEAD 2>/dev/null)" = "$commit" ]; then
    echo "warm build: .agents/worktree/worker-1 is at the published commit ${commit:0:12};" \
      "\`cargo xtask worktree acquire\` hands it out there, built. Keep that commit as the" \
      "base of the work: moving to a newer main recompiles what changed, often all of it."
  else
    echo "warm build: .agents/worktree/worker-1 is not at the published commit ${commit:0:12}," \
      "so its first build recompiles what differs"
  fi
}

case "${1:-}:$#" in
  pack:3) pack "$2" "$3" ;;
  publish:3) publish "$2" "$3" ;;
  restore:2) restore "$2" ;;
  restore:3) [ "$3" = replace-stale ] || fail "restore: the third argument can only be replace-stale"; restore "$2" "$3" ;;
  # A hook must never block or fail a session; the cold build still works.
  session-start:1) session_start || true ;;
  *) fail "usage: warm_build.sh pack|publish <slot> <out-dir> | restore <slot> [replace-stale] | session-start" ;;
esac
