#!/bin/sh
# Representative seed-lineage joins for the exhaustive Python OMGRFN17 owners.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN17 Beta join: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in python3 codesign; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN17 Beta join: skipped ($TOOL absent)"; exit 0; }; done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" python3 -B "$R/omgrfn17-materialize-beta.py" "$T/materialized"
for PROFILE in recurrent one-byte empty; do
  for OWNER in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result r5-elf; do
    PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" \
      python3 -B "$R/omgrfn17-$OWNER.py" < "$T/materialized/$PROFILE.rfn" > "$T/$PROFILE-$OWNER-python.out"
    [ ! -s "$T/$PROFILE-$OWNER-python.out" ] || exit 1
  done
done

SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1

TAB=$(printf '\t')
COUNT=0
MAX_TAPE=0
TAPE_REPORT=
while IFS="$TAB" read -r OWNER PROFILE REJECT_AT; do
  [ -n "$OWNER" ] || continue
  "$T/bc0" < "$T/materialized/$OWNER.beta" > "$T/$OWNER.native.asm"
  "$T/bc1" < "$T/materialized/$OWNER.beta" > "$T/$OWNER.self.asm"
  cmp "$T/$OWNER.native.asm" "$T/$OWNER.self.asm" >/dev/null
  "$ASM" < "$T/$OWNER.native.asm" > "$T/$OWNER.tape"
  BYTES=$(wc -c < "$T/$OWNER.tape" | tr -d ' ')
  [ "$BYTES" -le 262140 ] || { echo "OMGRFN17 Beta join: $OWNER tape $BYTES exceeds ceiling" >&2; exit 1; }
  [ "$BYTES" -le "$MAX_TAPE" ] || MAX_TAPE=$BYTES
  TAPE_REPORT="${TAPE_REPORT}${OWNER}=${BYTES} "
  stamp_seed "$T/$OWNER.tape" "$SEED" "$T/$OWNER.native" >/dev/null 2>&1
  stamp_seed "$T/$OWNER.tape" "$SEED" "$T/$OWNER.self" >/dev/null 2>&1
  for MODE in native self; do
    "$T/$OWNER.$MODE" < "$T/materialized/$PROFILE.rfn" > "$T/$OWNER-$MODE-positive.out"
    [ ! -s "$T/$OWNER-$MODE-positive.out" ] || exit 1
    set +e
    "$T/$OWNER.$MODE" < "$T/materialized/$OWNER-reject.rfn" > "$T/$OWNER-$MODE-reject.out"
    STATUS=$?
    set -e
    [ "$STATUS" -eq 251 ] && [ ! -s "$T/$OWNER-$MODE-reject.out" ] || {
      echo "OMGRFN17 Beta join: $OWNER $MODE local reject returned $STATUS" >&2; exit 1;
    }
  done
  COUNT=$((COUNT + 1))
done < "$T/materialized/manifest.tsv"
echo "OMGRFN17 Beta join: $COUNT modular owners native/self fixed; recurrent + one-byte + empty representatives; max tape $MAX_TAPE bytes"
echo "OMGRFN17 Beta tapes: $TAPE_REPORT"
