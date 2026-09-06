#!/bin/sh
# Eval harness for the `advance` skill.
#
# `advance` commits and pushes to `main`, so it cannot be evaluated in this
# checkout and cannot be evaluated in a git worktree either: worktrees share
# `.git/config` (and therefore the real remote) and share `refs/heads/main`, so
# two agents told to commit to `main` would interleave into one history. Each
# side instead gets its own bare origin plus a clone of it, on disk, reachable
# from nothing.
#
# Usage:
#   sh harness.sh setup <skill-a-path> <skill-b-path|none>
#   sh harness.sh prompt <a|b> <eval-prompt>
#   sh harness.sh collect <a|b> <report-dir>      # report + git evidence
#   sh harness.sh regate  <a|b> <report-dir>      # independent gate re-run
#   sh harness.sh reset   <a|b>
#   sh harness.sh clean
#
# Work root defaults under %LOCALAPPDATA%\Temp; override with OMEGA_EVAL_ROOT.
# Keep it SHORT: Windows caps paths at 260 characters and this repo's deepest
# source paths are ~120, so a long root fails the clone with "Filename too long".
set -u

REPO=${OMEGA_EVAL_REPO:-$(git rev-parse --show-toplevel)}
_local=$(cygpath -u "${LOCALAPPDATA:-}" 2>/dev/null || echo "/c/Users/${USERNAME:-User}/AppData/Local")
WORK=${OMEGA_EVAL_ROOT:-$_local/Temp/omega-eval}
MARKER="Stage eval skill"

usage() { sed -n '2,25p' "$0"; exit 2; }

require_mbx() {
  version=$(mbx --version 2>/dev/null) || {
    echo "error: mbx 1.7.0 or newer is required; direct Cargo fallback is forbidden" >&2
    exit 1
  }
  number=${version#mbx }
  major=${number%%.*}
  remainder=${number#*.}
  minor=${remainder%%.*}
  case "$major" in
    ''|*[!0-9]*)
      echo "error: could not parse mbx version: $version" >&2
      exit 1
      ;;
  esac
  case "$minor" in
    ''|*[!0-9]*)
      echo "error: could not parse mbx version: $version" >&2
      exit 1
      ;;
  esac
  if [ "$major" -lt 1 ] || { [ "$major" -eq 1 ] && [ "$minor" -lt 7 ]; }; then
    echo "error: mbx 1.7.0 or newer is required; found $version" >&2
    exit 1
  fi
  echo "using $version"
}

# ---------------------------------------------------------------- setup

cmd_setup() {
  require_mbx
  skill_a=$1; skill_b=$2
  rm -rf "$WORK"; mkdir -p "$WORK"
  for side in a b; do
    git clone --bare --quiet "$REPO" "$WORK/o$side.git"
    git -C "$WORK/o$side.git" remote remove origin 2>/dev/null
    git -c core.longpaths=true clone --quiet "$WORK/o$side.git" "$WORK/$side"
    git -C "$WORK/$side" config core.longpaths true
    git -C "$WORK/$side" config user.name  "Eval Runner"
    git -C "$WORK/$side" config user.email "eval@localhost"
    mkdir -p "$WORK/scratch-$side"
  done

  _stage a "$skill_a"
  _stage b "$skill_b"

  echo "--- remotes (must contain no forge URL) ---"
  git -C "$WORK/a" remote -v; git -C "$WORK/b" remote -v
  echo "--- prewarming mbx in both clones (cold; expect several minutes) ---"
  for side in a b; do (_prewarm "$side" > "$WORK/prewarm-$side.log" 2>&1 &) ; done
  echo "prewarm running; watch $WORK/prewarm-{a,b}.log for 'prewarm done'"
  echo "then run: sh harness.sh baseline"
}

# Put the named SKILL.md into a clone and commit it, so every run starts from a
# clean tree at a known base. `none` removes the skill entirely (no-skill arm).
_stage() {
  side=$1; skill=$2; dest="$WORK/$side/.claude/skills/advance"
  if [ "$skill" = "none" ]; then rm -rf "$WORK/$side/.claude/skills"
  else mkdir -p "$dest"; cp "$skill" "$dest/SKILL.md"; fi
  git -C "$WORK/$side" add -A
  # --allow-empty matters: if the staged SKILL.md is byte-identical to what is
  # already committed, there is no diff, and without a marker commit `collect`
  # has no base to anchor its ranges on.
  git -C "$WORK/$side" commit --quiet --allow-empty -m "$MARKER"
  git -C "$WORK/$side" push --quiet -f origin main
  # The clone's committed .gitignore may ignore .claude/, in which case the file
  # is present on disk but untracked. Agents read it by path, so that is fine --
  # but `reset` must not delete it, hence `git clean -fd` without -x below.
  printf '%s skill: %s\n' "$side" "$skill"
}

_prewarm() {
  cd "$WORK/$1" || exit 1
  echo "=== $1 prewarm start $(date) ==="
  mbx clippy --workspace --all-targets 2>&1 | tail -2
  mbx check  --workspace --all-targets 2>&1 | tail -2
  mbx nextest run   --workspace --lib --no-run 2>&1 | tail -2
  mbx nextest run -p omega-architecture-test --all-targets --no-run 2>&1 | tail -2
  echo "=== $1 prewarm done $(date) ==="
}

# ------------------------------------------------------- baseline reference

# Record which gates are red BEFORE any agent runs. Without this you cannot tell
# an agent's breakage from the host's, and "gate honesty" is ungradeable.
# Runs alone on purpose: concurrent `mbx nextest run` suites make load-sensitive tests
# (CPU/timer limits) fail spuriously.
cmd_baseline() {
  out="$WORK/baseline"; mkdir -p "$out"
  _regate b "$out"
  echo "--- failures from the completed library gate ---"
  sed -n -E 's/^ *(FAIL|TIMEOUT|ABORT|LEAK) +\[[^]]*\] +(\( *[0-9]+\/[0-9]+\) +)?/\1 /p' "$out/_nofailfast.txt" | sort -u \
    | tee "$out/_known_failures.txt"
  echo "saved to $out/"
}

# ---------------------------------------------------------------- prompts

# Both arms MUST come from this one template. The only permitted differences are
# the clone path, the scratch path, the skill paragraph, and the report path.
# An asymmetry anywhere else lands on whatever assertion you are trying to
# measure -- e.g. saying pushing is "expected" in one arm and only "safe" in the
# other silently decides `pushed_to_origin` for you.
cmd_prompt() {
  side=$1; task=$2
  if [ -d "$WORK/$side/.claude/skills/advance" ]; then
    skillpara="Your operating instructions for this session are in \`$WORK/$side/.claude/skills/advance/SKILL.md\`. Read that file first and follow it as written. It builds on \`$WORK/$side/AGENTS.md\`, so read that too."
  else
    skillpara="You have no special operating instructions beyond the repository's own \`$WORK/$side/AGENTS.md\`. Read that first. Do not load, read, or invoke any skill, and do not read anything under a \`.claude/\` directory."
  fi
  cat <<PROMPT
You are running one real work session in an isolated, throwaway clone of the Omega compiler repository.

Working directory: \`$WORK/$side\` — cd there first and do ALL work there.

Use \`$WORK/scratch-$side\` for temporary files and scripts. Do NOT use the session scratchpad directory — other agents are using it concurrently and your files will collide.

This clone's git \`origin\` is a local throwaway bare repository on disk, NOT a hosted forge. Committing and pushing are safe here.

$skillpara

The user's request for this session is:

"$task"

Scope limit: stop after your first coherent commit, or after roughly 30 minutes of work, whichever comes first. Do not start a second task after the first one lands.

When you stop, write your report to \`$WORK/$side/EVAL_REPORT.md\` — inside the clone, so no permission boundary blocks it. Use exactly these sections:

## Task picked
What you chose and why. If it came from a board, quote the board line verbatim and name the file it came from.

## Commands run
Every shell command you ran, in order, one per line.

## Gate results
Each gate command you ran, its actual outcome, and WHEN you ran it relative to your edits. Report failures as failures.

## Commits
The verbatim output of \`git log --oneline <base>..HEAD\` where <base> is the commit your clone started at.

## Instruction files used
Which instruction files or skills you read or invoked.

## Notes
Anything you deliberately chose not to do, and why.

Do not commit EVAL_REPORT.md. Write nothing outside the clone.
PROMPT
}

# ---------------------------------------------------------------- collect

cmd_collect() {
  side=$1; out=$2; R="$WORK/$side"
  mkdir -p "$out"
  [ -f "$R/EVAL_REPORT.md" ] && mv "$R/EVAL_REPORT.md" "$out/report.md"
  base=$(git -C "$R" log --format='%H %s' --all | grep -m1 -F "$MARKER" | cut -d' ' -f1)
  git -C "$R" branch --show-current           > "$out/_branch.txt"    2>&1
  git -C "$R" log --oneline "$base"..HEAD     > "$out/_commits.txt"   2>&1
  git -C "$R" status --short                  > "$out/_status.txt"    2>&1
  git -C "$R" diff "$base"..HEAD --stat       > "$out/_diffstat.txt"  2>&1
  git -C "$R" diff "$base"..HEAD              > "$out/_full_diff.txt" 2>&1
  git -C "$R" diff "$base"..HEAD -- 'TASKS*.md' OWNER_QUESTIONS.md > "$out/_board_diff.txt" 2>&1
  git -C "$R" branch -a                       > "$out/_branches.txt"  2>&1
  head=$(git -C "$R" rev-parse HEAD); om=$(git -C "$R" rev-parse origin/main)
  { echo "base=$base"; echo "HEAD=$head"; echo "origin/main=$om"
    if [ "$om" = "$head" ] && [ "$head" != "$base" ]; then echo "pushed=yes"; else echo "pushed=no"; fi
  } > "$out/_refs.txt"
  cat "$out/_refs.txt"
}

# ---------------------------------------------------------------- regate

# Independent re-run of the AGENTS.md gate list on whatever HEAD the agent left.
# This is the only way to check a report's gate claims. Run it with the machine
# otherwise idle and one side at a time.
cmd_regate() { _regate "$1" "$2"; }
_regate() {
  require_mbx
  side=$1; out=$2; mkdir -p "$out"
  cd "$WORK/$side" || exit 1
  {
    echo "### regate on $(git rev-parse --short HEAD) at $(date)"
    for g in "cargo fmt --all -- --check" \
             "mbx clippy --workspace --all-targets -- -D warnings" \
             "mbx nextest run -p omega-architecture-test --all-targets --no-fail-fast" \
             "mbx check --workspace --all-targets" \
             "mbx nextest run --color never --workspace --lib --no-fail-fast"; do
      echo "--- GATE: $g"
      o=$(eval "$g" 2>&1); echo "EXIT=$?"; echo "$o" | tail -15
      if [ "$g" = "mbx nextest run --color never --workspace --lib --no-fail-fast" ]; then
        printf '%s\n' "$o" > "$out/_nofailfast.txt"
      fi
    done
    echo "REGATE_COMPLETE"     # sentinel: `pgrep` does not exist in git-bash,
  } > "$out/_regate.txt" 2>&1  # so never wait on a process, wait on this line.
  grep '^--- GATE\|^EXIT=' "$out/_regate.txt" | sed 's/--- GATE: //' | paste -d' ' - -
}

# ---------------------------------------------------------------- reset/clean

cmd_reset() {
  side=$1; R="$WORK/$side"
  base=$(git -C "$R" log --format='%H %s' --all | grep -m1 -F "$MARKER" | cut -d' ' -f1)
  git -C "$R" checkout --quiet --force main 2>/dev/null || git -C "$R" checkout --quiet -B main "$base"
  git -C "$R" reset --quiet --hard "$base"
  git -C "$R" clean -qfd            # -d not -x: keeps the warm target/ dir
  git -C "$WORK/o$side.git" update-ref refs/heads/main "$base"
  git -C "$R" fetch --quiet origin
  for b in $(git -C "$R" for-each-ref --format='%(refname:short)' refs/heads | grep -v '^main$'); do
    git -C "$R" branch -qD "$b"
  done
  rm -rf "${WORK:?}/scratch-$side"; mkdir -p "$WORK/scratch-$side"
  echo "$side reset to $base ($(git -C "$R" status --short | wc -l) dirty)"
}

cmd_clean() { rm -rf "${WORK:?}"; echo "removed $WORK"; }

# ----------------------------------------------------------------- dispatch

[ $# -ge 1 ] || usage
c=$1; shift
case $c in
  setup)    [ $# -eq 2 ] || usage; cmd_setup "$1" "$2" ;;
  baseline) cmd_baseline ;;
  prompt)   [ $# -eq 2 ] || usage; cmd_prompt "$1" "$2" ;;
  collect)  [ $# -eq 2 ] || usage; cmd_collect "$1" "$2" ;;
  regate)   [ $# -eq 2 ] || usage; cmd_regate "$1" "$2" ;;
  reset)    [ $# -eq 1 ] || usage; cmd_reset "$1" ;;
  clean)    cmd_clean ;;
  *)        usage ;;
esac
