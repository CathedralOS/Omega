#!/usr/bin/env sh
# Phase-isolated seam canaries for the remaining-declaration budget theorem.

declaration_budget_build_tooth() { # name exact-old exact-new
  declaration_budget_tooth_name=$1
  declaration_budget_tooth_old=$2
  declaration_budget_tooth_new=$3
  declaration_budget_tooth_count=$(grep -F -c -- \
    "$declaration_budget_tooth_old" "$T/declaration-budget.alpha" || true)
  if [ "$declaration_budget_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $declaration_budget_tooth_name anchor count $declaration_budget_tooth_count" >&2
    exit 1
  fi
  awk -v old="$declaration_budget_tooth_old" \
      -v new="$declaration_budget_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/declaration-budget.alpha" > \
    "$T/$declaration_budget_tooth_name.alpha"
  "$ASM" < "$T/$declaration_budget_tooth_name.alpha" > \
    "$T/$declaration_budget_tooth_name.tape"
  stamp_seed "$T/$declaration_budget_tooth_name.tape" "$SEED" \
    "$T/$declaration_budget_tooth_name" >/dev/null
}

declaration_budget_build_teeth() {
  declaration_budget_build_tooth declaration-budget-wrong-cnts-schema \
    'imm r12, 1129206867             ; CNTS tag' \
    'imm r12, 1129206866             ; CNTS tag'
  declaration_budget_build_tooth declaration-budget-wrong-pcap-room \
    'imm r13, 528168                 ; nslots<=1024 room completion' \
    'imm r13, 528176                 ; nslots<=1024 room completion'
  declaration_budget_build_tooth declaration-budget-wrong-srel-schema \
    'imm r12, 1279611475             ; SREL tag' \
    'imm r12, 1279611474             ; SREL tag'
  declaration_budget_build_tooth declaration-budget-wrong-count-call \
    'imm r22, 50142                 ; exact count_lets call PC' \
    'imm r22, 50143                 ; exact count_lets call PC'
  declaration_budget_build_tooth declaration-budget-wrong-declare-target \
    'imm r25, 24145' \
    'imm r25, 24146'
  declaration_budget_build_tooth declaration-budget-drop-occurrence-custody \
    'imm r13, 1                      ; ordered distinct occurrence' \
    'imm r13, 0                      ; ordered distinct occurrence'
  declaration_budget_build_tooth declaration-budget-wrong-parameter-domain \
    'imm r2, 5                      ; exact N=0..4 row count' \
    'imm r2, 4                      ; exact N=0..4 row count'
  declaration_budget_build_tooth declaration-budget-admit-nloc-1024 \
    'imm r1, 1023                   ; universal pre-declare NLOC maximum' \
    'imm r1, 1024                   ; universal pre-declare NLOC maximum'
  declaration_budget_build_tooth declaration-budget-drop-let-advance \
    'imm r12, 1                    ; P advances once for real let' \
    'imm r12, 0                    ; P advances once for real let'
  declaration_budget_build_tooth declaration-budget-drop-remaining-rank \
    'imm r13, 1                    ; R decreases by exactly one' \
    'imm r13, 0                    ; R decreases by exactly one'
  declaration_budget_build_tooth declaration-budget-add-productivity \
    'store r20, r2                  ; no cursor/output productivity premise' \
    'store r20, r15                 ; no cursor/output productivity premise'
  declaration_budget_build_tooth declaration-budget-admit-full-branch \
    'store r1, r2                  ; DCLS full branch impossible' \
    'store r1, r10                 ; DCLS full branch impossible'
  declaration_budget_build_tooth declaration-budget-admit-origin3 \
    'store r1, r2                  ; origin3 root-unreachable' \
    'store r1, r10                 ; origin3 root-unreachable'
  declaration_budget_build_tooth declaration-budget-wrong-publication \
    'imm r2, 1296388676             ; DREM' \
    'imm r2, 1296388675             ; DREM'
}

declaration_budget_reject_teeth() {
  for declaration_budget_tooth_name in \
    declaration-budget-wrong-cnts-schema \
    declaration-budget-wrong-pcap-room \
    declaration-budget-wrong-srel-schema \
    declaration-budget-wrong-count-call \
    declaration-budget-wrong-declare-target \
    declaration-budget-drop-occurrence-custody \
    declaration-budget-wrong-parameter-domain \
    declaration-budget-admit-nloc-1024 \
    declaration-budget-drop-let-advance \
    declaration-budget-drop-remaining-rank \
    declaration-budget-add-productivity \
    declaration-budget-admit-full-branch \
    declaration-budget-admit-origin3 \
    declaration-budget-wrong-publication
  do
    set +e
    "$T/$declaration_budget_tooth_name" \
      < "$T/control.bundle" > "$T/stdout"
    declaration_budget_tooth_status=$?
    set -e
    if [ "$declaration_budget_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $declaration_budget_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
