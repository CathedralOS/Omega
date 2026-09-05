#!/bin/sh
# Tests the admission contract with tiny Git fixtures and fake build commands.
# This never builds Omega and does not establish that its compiler gates pass.
set -eu
checker=$(cd "$(dirname "$0")" && pwd)/verify.sh
scratch=$(mktemp -d "${TMPDIR:-/tmp}/omega-advance-verify.XXXXXX")
repository=$scratch/a
mkdir -p "$repository" "$scratch/bin"
git -C "$repository" init --quiet
git -C "$repository" config user.name 'Advance check fixture'
git -C "$repository" config user.email 'advance-test@localhost'
git -C "$repository" config core.autocrlf false
mkdir "$repository/a" "$repository/b"
printf 'base\n' > "$repository/a/file"
printf 'base\n' > "$repository/b/file"
git -C "$repository" add .
git -C "$repository" commit --quiet -m base
base=$(git -C "$repository" rev-parse HEAD)
printf 'worker a\nother b\n' > "$scratch/lanes"
: > "$scratch/accepted"
count=0
pass() { "$@" > "$scratch/latest.log" 2>&1 || { cat "$scratch/latest.log"; exit 1; }; count=$((count + 1)); }
reject() { if "$@" > "$scratch/latest.log" 2>&1; then echo "unexpected success: $*"; exit 1; fi; count=$((count + 1)); }
pass sh "$checker" lanes "$scratch/lanes"
printf 'worker a\nother a/child\n' > "$scratch/bad-lanes"
reject sh "$checker" lanes "$scratch/bad-lanes"
printf 'worker a\nother a\n' > "$scratch/bad-lanes"
reject sh "$checker" lanes "$scratch/bad-lanes"
printf 'worker ../a\n' > "$scratch/bad-lanes"
reject sh "$checker" lanes "$scratch/bad-lanes"
printf 'worker a\nother ab\n' > "$scratch/bad-lanes"
pass sh "$checker" lanes "$scratch/bad-lanes"
printf 'changed\n' > "$repository/a/file"
git -C "$repository" commit --quiet -am change
revision=$(git -C "$repository" rev-parse HEAD)
pass sh "$checker" commit "$scratch/lanes" worker "$repository" "$base" "$revision" "$scratch/accepted"
reject sh "$checker" commit "$scratch/lanes" other "$repository" "$base" "$revision" "$scratch/accepted"
printf 'a/file\n' > "$scratch/accepted"
reject sh "$checker" commit "$scratch/lanes" worker "$repository" "$base" "$revision" "$scratch/accepted"
: > "$scratch/accepted"
printf 'dirty\n' > "$repository/untracked"
reject sh "$checker" commit "$scratch/lanes" worker "$repository" "$base" "$revision" "$scratch/accepted"
rm "$repository/untracked"
reject sh "$checker" commit "$scratch/lanes" worker "$repository" "$base" "$base" "$scratch/accepted"
# The final diff hides this outside-lane change; the commit walk must retain it.
printf 'outside\n' > "$repository/b/file"
git -C "$repository" commit --quiet -am outside
git -C "$repository" revert --no-edit HEAD > /dev/null
revision=$(git -C "$repository" rev-parse HEAD)
reject sh "$checker" commit "$scratch/lanes" worker "$repository" "$base" "$revision" "$scratch/accepted"
rename_base=$revision
git -C "$repository" mv a/file b/moved
git -C "$repository" commit --quiet -am rename
revision=$(git -C "$repository" rev-parse HEAD)
reject sh "$checker" commit "$scratch/lanes" worker "$repository" "$rename_base" "$revision" "$scratch/accepted"
reject sh "$checker" commit "$scratch/lanes" other "$repository" "$rename_base" "$revision" "$scratch/accepted"
cat > "$scratch/bin/mbx" <<'STUB'
#!/bin/sh
if [ "$*" = --version ]; then echo 'mbx 1.8.1'; exit 0; fi
[ "${ADVANCE_TEST_FAIL:-}" != "$1" ] || exit 7
exit 0
STUB
cp "$scratch/bin/mbx" "$scratch/bin/cargo"
chmod +x "$scratch/bin/mbx" "$scratch/bin/cargo"
PATH="$scratch/bin:$PATH"; export PATH
pass sh "$checker" gates "$repository" "$scratch/green"
pass sh "$checker" green "$repository" "$revision" "$scratch/green"
reject sh "$checker" gates "$repository" "$scratch/green"
reject sh "$checker" green "$repository" "$base" "$scratch/green"
ADVANCE_TEST_FAIL=clippy; export ADVANCE_TEST_FAIL
reject sh "$checker" gates "$repository" "$scratch/red"
reject sh "$checker" green "$repository" "$revision" "$scratch/red"
[ "$(wc -l < "$scratch/red/results.txt")" -eq 5 ]
[ ! -f "$scratch/red/GREEN" ]
unset ADVANCE_TEST_FAIL
sed -i '/ architecture /d' "$scratch/green/results.txt"
reject sh "$checker" green "$repository" "$revision" "$scratch/green"
printf '%s architecture 0\n%s architecture 0\n' "$revision" "$revision" >> "$scratch/green/results.txt"
reject sh "$checker" green "$repository" "$revision" "$scratch/green"
harness=$(dirname "$checker")/../evals/harness.sh
OMEGA_EVAL_ROOT=$scratch; export OMEGA_EVAL_ROOT
pass sh "$harness" regate a "$scratch/harness-green"
ADVANCE_TEST_FAIL=clippy; export ADVANCE_TEST_FAIL
reject sh "$harness" regate a "$scratch/harness-red"
printf '%s admission checks passed; fixtures retained at %s\n' "$count" "$scratch"
