#!/usr/bin/env sh
# Extracted Checker-A historical canaries for main-slurp.

main_slurp_build_teeth() {
# Phase-isolated main/slurp bridge teeth retain the exact source, artifact,
# slurp theorem, and every preceding phase.  They respectively sever the
# returned-r0/local association, admit zero==one, and relabel status 253 as 252.
sed 's/call main_slurp_value_load_local             ; checked returned-r0 flow/call main_slurp_value_one                   ; checked returned-r0 flow/' \
  "$T/control-check.alpha" > "$T/main-slurp-wrong-local.alpha"
"$ASM" < "$T/main-slurp-wrong-local.alpha" > "$T/main-slurp-wrong-local.tape"
stamp_seed "$T/main-slurp-wrong-local.tape" "$SEED" "$T/main-slurp-wrong-local" >/dev/null
sed 's/imm r21, 0                    ; checked zero != one result/imm r21, 1                    ; checked zero != one result/' \
  "$T/control-check.alpha" > "$T/main-slurp-wrong-branch.alpha"
"$ASM" < "$T/main-slurp-wrong-branch.alpha" > "$T/main-slurp-wrong-branch.tape"
stamp_seed "$T/main-slurp-wrong-branch.tape" "$SEED" "$T/main-slurp-wrong-branch" >/dev/null
sed 's/imm r20, 253                  ; checked concrete failure value/imm r20, 252                  ; checked concrete failure value/' \
  "$T/control-check.alpha" > "$T/main-slurp-wrong-status.alpha"
"$ASM" < "$T/main-slurp-wrong-status.alpha" > "$T/main-slurp-wrong-status.tape"
stamp_seed "$T/main-slurp-wrong-status.tape" "$SEED" "$T/main-slurp-wrong-status" >/dev/null
sed 's/imm r1, 525744                ; import success clause/imm r1, 525752                ; import success clause/' \
  "$T/control-check.alpha" > "$T/main-slurp-wrong-clause.alpha"
"$ASM" < "$T/main-slurp-wrong-clause.alpha" > "$T/main-slurp-wrong-clause.tape"
stamp_seed "$T/main-slurp-wrong-clause.tape" "$SEED" "$T/main-slurp-wrong-clause" >/dev/null

}

main_slurp_reject_teeth() {
for main_slurp_tooth in main-slurp-wrong-local main-slurp-wrong-branch main-slurp-wrong-status main-slurp-wrong-clause; do
  set +e
  "$T/$main_slurp_tooth" < "$T/control.bundle" > "$T/stdout"
  main_slurp_tooth_status=$?
  set -e
  if [ "$main_slurp_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $main_slurp_tooth was not rejected" >&2
    exit 1
  fi
done
}
