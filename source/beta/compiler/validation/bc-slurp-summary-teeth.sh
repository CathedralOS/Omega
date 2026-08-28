#!/usr/bin/env sh
# Extracted Checker-A historical canaries for slurp-summary.

slurp_summary_build_teeth() {
# Phase-isolated slurp-summary teeth mutate only the checker proof script. The
# first derives the endpoint payload from n instead of c, the second admits a
# zero rank delta, the third breaks backedge renaming, and the fourth feeds zero
# rather than n to LEN.
sed 's/imm r20, 4                    ; derived c payload kind/imm r20, 2                    ; derived c payload kind/' \
  "$T/control-check.alpha" > "$T/slurp-wrong-payload.alpha"
"$ASM" < "$T/slurp-wrong-payload.alpha" > "$T/slurp-wrong-payload.tape"
stamp_seed "$T/slurp-wrong-payload.tape" "$SEED" "$T/slurp-wrong-payload" >/dev/null
sed 's/imm r3, 1                     ; checked inverse-successor delta/imm r3, 0                     ; checked inverse-successor delta/' \
  "$T/control-check.alpha" > "$T/slurp-zero-rank.alpha"
"$ASM" < "$T/slurp-zero-rank.alpha" > "$T/slurp-zero-rank.tape"
stamp_seed "$T/slurp-zero-rank.tape" "$SEED" "$T/slurp-zero-rank" >/dev/null
sed 's/imm r2, 2                    ; checked renamed cursor successor/imm r2, 1                    ; checked renamed cursor successor/' \
  "$T/control-check.alpha" > "$T/slurp-wrong-rename.alpha"
"$ASM" < "$T/slurp-wrong-rename.alpha" > "$T/slurp-wrong-rename.tape"
stamp_seed "$T/slurp-wrong-rename.tape" "$SEED" "$T/slurp-wrong-rename" >/dev/null
sed 's/call slurp_sv_load_n             ; checked LEN payload flow/call slurp_sv_zero               ; checked LEN payload flow/' \
  "$T/control-check.alpha" > "$T/slurp-wrong-len.alpha"
"$ASM" < "$T/slurp-wrong-len.alpha" > "$T/slurp-wrong-len.tape"
stamp_seed "$T/slurp-wrong-len.tape" "$SEED" "$T/slurp-wrong-len" >/dev/null
}

slurp_summary_reject_teeth() {
for slurp_tooth in slurp-wrong-payload slurp-zero-rank slurp-wrong-rename slurp-wrong-len; do
  set +e
  "$T/$slurp_tooth" < "$T/control.bundle" > "$T/stdout"
  slurp_tooth_status=$?
  set -e
  if [ "$slurp_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $slurp_tooth was not rejected" >&2
    exit 1
  fi
done
}
