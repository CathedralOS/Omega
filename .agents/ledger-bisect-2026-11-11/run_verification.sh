#!/bin/bash
set -euo pipefail
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
cd "$ROOT"
PIN=51e8c45c793007f211d0d9baee7de16767172444
export PIN_COMMIT="$PIN"
OUT="${VERIFY_OUT:-/tmp/verification-part-22.md}"
cp -a "$ROOT/.agents/ledger-bisect-2026-11-11/tools" /tmp/ledger-bisect-verify-tools
cp "$ROOT/.agents/ledger-bisect-2026-11-11/pin_observed.txt" /tmp/ledger_bisect_pin_observed.txt
git show "$PIN:tests/omega/corpus_outcomes.txt" >/tmp/ledger_bisect_corpus_outcomes.txt
export VERIFY_TOOLS=/tmp/ledger-bisect-verify-tools
export PIN_OBSERVED=/tmp/ledger_bisect_pin_observed.txt
export PIN_LEDGER=/tmp/ledger_bisect_corpus_outcomes.txt
TOOLS="$VERIFY_TOOLS"
chmod +x "$TOOLS"/*.sh

git checkout -f work/ledger-bisect-part-22

run_triples() {
  local commit="$1" witness="$2" label="$3"
  local a b c
  set +e
  "$TOOLS/verify_at_commit.sh" "$commit" "$witness"
  a=$?
  export VERIFY_NO_BUILD=1
  "$TOOLS/verify_at_commit.sh" "$commit" "$witness"
  b=$?
  "$TOOLS/verify_at_commit.sh" "$commit" "$witness"
  c=$?
  unset VERIFY_NO_BUILD
  set -e
  echo "- **$label** \`$commit\` witness \`$witness\`: $a,$b,$c" | tee -a "$OUT"
}

member_row() {
  local parent="$1" bad="$2" fixture="$3"
  local pg bd
  set +e
  export VERIFY_NO_BUILD=1
  "$TOOLS/verify_at_commit.sh" "$parent" "$fixture"
  pg=$?
  "$TOOLS/verify_at_commit.sh" "$bad" "$fixture"
  bd=$?
  unset VERIFY_NO_BUILD
  set -e
  echo "| \`$fixture\` | $pg | $bd |" | tee -a "$OUT"
}

: >"$OUT"
cat >>"$OUT" <<HDR
# Part 22 first-bad verification

PIN: \`$PIN\`
Strict test: exit 0 = meets PIN ledger; exit 1 = PIN status **and** PIN diagnostic class; each build uses \`target/verify-<sha>/\`.

HDR

declare -A CLUSTERS
CLUSTERS[c1_bad]=f2c75870ebaf890328b787fa6d8ed50e868edbb8
CLUSTERS[c2_bad]=55f4a84e523c0f1b64f7d72d80f4cb83e84e7017
CLUSTERS[c3_bad]=0a85200733ef1f32849c682936c53c22fdff3226
CLUSTERS[c1_good]=576c5c9433a95c1f4dcb4f18197f408afbfe92cd
CLUSTERS[c2_good]=bed3b818b169b1ff86fe30b5ec27fda873ffed5f
CLUSTERS[c3_good]=6f4ad2d1e228899c7cd4a54b80a6f591764c0794
CLUSTERS[c1_w]=pass/borrows/returned_slice_record_reference_custody_exit
CLUSTERS[c2_w]=pass/build/explicit_provider_storage_extent_exit
CLUSTERS[c3_w]=pass/termination/runtime_named_state_remainder_endpoint_exit

for id in c1 c2 c3; do
  bad="${CLUSTERS[${id}_bad]}"
  good="${CLUSTERS[${id}_good]}"
  w="${CLUSTERS[${id}_w]}"
  parent=$(git rev-parse "${bad}^1")
  subj=$(git log -1 --format='%s' "$bad")
  pr=$(echo "$subj" | sed -n 's/.*(#\([0-9]*\)).*/\1/p')
  echo "## Cluster $id" | tee -a "$OUT"
  echo "- Witness: \`$w\`" | tee -a "$OUT"
  echo "- Good anchor (full): \`$good\` — $(git log -1 --format='%s' "$good")" | tee -a "$OUT"
  echo "- First bad (full): \`$bad\`" | tee -a "$OUT"
  echo "- First parent (full): \`$parent\` — $(git log -1 --format='%s' "$parent")" | tee -a "$OUT"
  echo "- Merge subject: $subj" | tee -a "$OUT"
  echo "- PR from subject: ${pr:-not in subject}" | tee -a "$OUT"
  echo "### Witness stability (strict)" | tee -a "$OUT"
  run_triples "$parent" "$w" "parent"
  run_triples "$bad" "$w" "first-bad"
  echo "### Diagnostic at first-bad vs PIN" | tee -a "$OUT"
  "$TOOLS/diag_at_commit.sh" "$bad" "$w" | tee -a "$OUT"
  echo "### Good anchor check" | tee -a "$OUT"
  run_triples "$good" "$w" "good-anchor"
  echo "### Member flip table" | tee -a "$OUT"
  echo "| fixture | parent | first-bad |" | tee -a "$OUT"
  echo "| --- | ---: | ---: |" | tee -a "$OUT"
  case "$id" in
    c1)
      for f in pass/borrows/returned_slice_record_reference_custody_exit \
        pass/borrows/runtime_returned_stored_view_reborrow_exit \
        pass/slices/runtime_mutable_slice_result_whole_forward_exit \
        pass/wire/wire_preserving_decode_relay_exit; do
        member_row "$parent" "$bad" "$f"
      done
      ;;
    c2) member_row "$parent" "$bad" "$w" ;;
    c3) member_row "$parent" "$bad" "$w" ;;
  esac
  echo "### Diff hunk (first-bad)" | tee -a "$OUT"
  case "$id" in
    c1)
      main=omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks/borrows/resources/formation_sources/returned_projections.rs
      ;;
    c3)
      main=omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks/termination.rs
      ;;
    *)
      main=$(git diff-tree --no-commit-id --name-only -r "$bad" | rg -v '(corpus_outcomes|\.md$|TASKS)' | head -1)
      ;;
  esac
  if [[ -z "$main" ]]; then
    echo 'mechanism not established (no non-ledger compiler diff)' | tee -a "$OUT"
  else
    echo "\`$main\`:" | tee -a "$OUT"
    git show "$bad" -- "$main" | head -n 80 | tee -a "$OUT"
  fi
  echo "" | tee -a "$OUT"
done

git checkout -f work/ledger-bisect-part-22
mkdir -p "$ROOT/.agents/ledger-bisect-2026-11-11"
cp "$OUT" "$ROOT/.agents/ledger-bisect-2026-11-11/verification-part-22.md"
echo DONE | tee -a "$OUT"
