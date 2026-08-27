#!/bin/sh
# Representative seed-lineage projections for the general OMGRFN21 owners.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "OMGRFN21 Beta join: skipped (requires Darwin arm64)"; exit 0;; esac
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" python3 -B "$R/omgrfn21-materialize-beta.py" "$T/m"
for OWNER in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result r5-elf; do
  PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" python3 -B "$R/omgrfn21-$OWNER.py" < "$T/m/canonical.rfn" > "$T/$OWNER.out"
  [ ! -s "$T/$OWNER.out" ] || exit 1
done
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
TAB=$(printf '\t'); COUNT=0; MAX=0; REPORT=
while IFS="$TAB" read -r OWNER REJECT_AT; do
  "$T/bc0" < "$T/m/$OWNER.beta" > "$T/$OWNER.0.asm"
  "$T/bc1" < "$T/m/$OWNER.beta" > "$T/$OWNER.1.asm"
  cmp "$T/$OWNER.0.asm" "$T/$OWNER.1.asm" >/dev/null
  "$ASM" < "$T/$OWNER.0.asm" > "$T/$OWNER.tape"
  BYTES=$(wc -c < "$T/$OWNER.tape" | tr -d ' ')
  [ "$BYTES" -le 262140 ] || { echo "OMGRFN21 Beta join: $OWNER tape $BYTES exceeds ceiling" >&2; exit 1; }
  [ "$BYTES" -le "$MAX" ] || MAX=$BYTES
  REPORT="${REPORT}${OWNER}=${BYTES} "
  stamp_seed "$T/$OWNER.tape" "$SEED" "$T/$OWNER.0" >/dev/null 2>&1
  stamp_seed "$T/$OWNER.tape" "$SEED" "$T/$OWNER.1" >/dev/null 2>&1
  for MODE in 0 1; do
    "$T/$OWNER.$MODE" < "$T/m/canonical.rfn" > "$T/positive"
    [ ! -s "$T/positive" ] || exit 1
    set +e; "$T/$OWNER.$MODE" < "$T/m/$OWNER-reject.rfn" > "$T/reject"; STATUS=$?; set -e
    [ "$STATUS" -eq 251 ] && [ ! -s "$T/reject" ] || exit 1
  done
  COUNT=$((COUNT + 1))
done < "$T/m/manifest.tsv"
echo "OMGRFN21 Beta join: $COUNT projections fixed; max tape $MAX bytes"
echo "OMGRFN21 Beta tapes: $REPORT"
