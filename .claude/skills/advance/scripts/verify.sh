#!/bin/sh
# Mechanical admission checks; no spawning, checkout, integration, or publishing.
# All manifests and results belong in the assigner's session directory.
set -eu

fail() { echo "advance: $*" >&2; exit 1; }

lanes() {
  # Whitespace-separated owner/path rows. A path covers itself and descendants.
  # Reject unsupported names instead of interpreting Git's quoted path output.
  awk '
    NF != 2 || $1 !~ /^[A-Za-z0-9_-]+$/ ||
      $2 !~ /^[A-Za-z0-9_.\/-]+$/ || $2 ~ /(^\/|\/$|\/\/)/ ||
      $2 ~ /(^|\/)\.\.?($|\/)/ { bad = 1; next }
    { path[NR] = $2; count++ }
    END {
      for (left in path) for (right in path) if (left < right &&
        (path[left] == path[right] ||
         index(path[left], path[right] "/") == 1 ||
         index(path[right], path[left] "/") == 1)) bad = 1
      exit (bad || !count)
    }
  ' "$1" || fail 'lanes are empty, malformed, or overlap'
}

clean_head() {
  [ "$(git -C "$1" rev-parse HEAD)" = "$2" ] || fail 'HEAD changed'
  [ -z "$(git -C "$1" status --porcelain --untracked-files=all)" ] ||
    fail 'tracked or untracked work is present'
}

paths() {
  repository=$1; base=$2; revision=$3
  git -C "$repository" merge-base --is-ancestor "$base" "$revision" ||
    fail 'base is not an ancestor'
  [ "$base" != "$revision" ] || fail 'empty revision range'
  [ -z "$(git -C "$repository" rev-list --merges "$base..$revision")" ] ||
    fail 'merge commits require separate review'
  # Walk every commit: an edit followed by a revert still touched the file.
  history=$(git -C "$repository" log --format= --name-only --no-renames "$base..$revision") || exit 1
  printf '%s\n' "$history" | LC_ALL=C sort -u | sed '/^$/d'
}

commit() {
  manifest=$1; owner=$2; repository=$3; base=$4; revision=$5; accepted=$6
  lanes "$manifest"
  clean_head "$repository" "$revision"
  changed=$(paths "$repository" "$base" "$revision") || exit 1
  [ -n "$changed" ] || fail 'no changed paths'
  printf '%s\n' "$changed" | awk -v selected="$owner" '
    NR == FNR { if ($1 == selected) allowed[$2] = 1; next }
    {
      matched = 0
      if ($0 !~ /^[A-Za-z0-9_.\/-]+$/) bad = 1
      for (prefix in allowed)
        if ($0 == prefix || index($0, prefix "/") == 1) matched = 1
      if (!matched) bad = 1
    }
    END { exit bad }
  ' "$manifest" - || fail 'commit touched a path outside its lane'
  # accepted is a newline-delimited union emitted by earlier successful checks.
  [ -f "$accepted" ] || fail 'accepted-path ledger is missing'
  printf '%s\n' "$changed" | awk '
    FILENAME != "-" { seen[$0] = 1; next }
    $0 in seen { overlap = 1 }
    END { exit overlap }
  ' "$accepted" - || fail 'commit overlaps an accepted commit'
  printf '%s\n' "$changed"
}

gates() {
  repository=$1; output=$2
  case "$output" in /*|[A-Za-z]:/*) ;; *) fail 'gate output must be absolute' ;; esac
  [ ! -e "$output" ] || fail 'gate output already exists; use a fresh directory'
  mkdir -p "$output"
  output=$(cd "$output" && pwd -P)
  repository=$(cd "$repository" && pwd -P)
  case "$output/" in "$repository/"*) fail 'gate output must be outside checkout' ;; esac
  revision=$(git -C "$repository" rev-parse HEAD)
  clean_head "$repository" "$revision"
  mbx --version > "$output/tool.txt" || fail 'mbx is unavailable'
  awk '$1 == "mbx" { split($2, version, ".");
    if (version[1] ~ /^[0-9]+$/ && version[2] ~ /^[0-9]+$/ &&
      (version[1] > 1 || (version[1] == 1 && version[2] >= 7))) ok = 1 }
    END { exit !ok }' "$output/tool.txt" || fail 'mbx 1.7.0 or newer is required'
  cd "$repository"
  failed=0
  for gate in fmt clippy architecture check lib; do
    # Capture the command itself, never the exit of tee, tail, or grep.
    status=0
    case "$gate" in
      fmt) cargo fmt --all -- --check > "$output/$gate.log" 2>&1 || status=$? ;;
      clippy) mbx clippy --workspace --all-targets -- -D warnings > "$output/$gate.log" 2>&1 || status=$? ;;
      architecture) mbx test -p omega-architecture-test --all-targets > "$output/$gate.log" 2>&1 || status=$? ;;
      check) mbx check --workspace --all-targets > "$output/$gate.log" 2>&1 || status=$? ;;
      lib) mbx test --workspace --lib --no-fail-fast > "$output/$gate.log" 2>&1 || status=$? ;;
    esac
    printf '%s %s %s\n' "$revision" "$gate" "$status" >> "$output/results.txt"
    printf '%s exit=%s\n' "$gate" "$status"
    [ "$status" = 0 ] || failed=1
    clean_head "$repository" "$revision"
  done
  [ "$failed" = 0 ] || fail 'one or more gates failed'
  printf '%s\n' "$revision" > "$output/GREEN"
}

green() {
  repository=$1; revision=$2; output=$3
  clean_head "$repository" "$revision"
  [ "$(cat "$output/GREEN")" = "$revision" ] || fail 'no green result for this HEAD'
  awk -v revision="$revision" '
    NF != 3 || $1 != revision || $3 != "0" { bad = 1 }
    $2 !~ /^(fmt|clippy|architecture|check|lib)$/ { bad = 1 }
    { if (seen[$2]++) bad = 1; count++ }
    END { exit (bad || count != 5) }
  ' "$output/results.txt" || fail 'gates are missing, duplicated, stale, or red'
}

[ $# -gt 0 ] || fail 'expected lanes, commit, gates, or green'
command=$1; shift
case "$command" in
  lanes) [ $# = 1 ] || fail 'lanes MANIFEST'; lanes "$@" ;;
  commit) [ $# = 6 ] || fail 'commit MANIFEST OWNER REPO BASE SHA ACCEPTED_PATHS'; commit "$@" ;;
  gates) [ $# = 2 ] || fail 'gates REPO FRESH_ABSOLUTE_OUTPUT'; gates "$@" ;;
  green) [ $# = 3 ] || fail 'green REPO SHA OUTPUT'; green "$@" ;;
  *) fail "unknown command: $command" ;;
esac
