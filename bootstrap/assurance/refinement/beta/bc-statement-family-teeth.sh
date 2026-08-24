#!/usr/bin/env sh
# Phase-isolated canaries for the independent statement-family shape tranche.

statement_family_build_tooth() { # name exact-old exact-new
  statement_tooth_name=$1
  statement_tooth_old=$2
  statement_tooth_new=$3
  statement_tooth_count=$(grep -F -c -- "$statement_tooth_old" \
    "$T/statement-family-shape.alpha" || true)
  if [ "$statement_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $statement_tooth_name anchor count $statement_tooth_count" >&2
    exit 1
  fi
  awk -v old="$statement_tooth_old" -v new="$statement_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/statement-family-shape.alpha" > "$T/$statement_tooth_name.alpha"
  "$ASM" < "$T/$statement_tooth_name.alpha" > "$T/$statement_tooth_name.tape"
  stamp_seed "$T/$statement_tooth_name.tape" "$SEED" \
    "$T/$statement_tooth_name" >/dev/null
}

statement_family_build_teeth() {
  statement_family_build_tooth statement-shape-wrong-family-count \
    'imm r2, 7                       ; exact procedure family cardinality' \
    'imm r2, 6                       ; exact procedure family cardinality'
  statement_family_build_tooth statement-shape-wrong-gen-store-entry \
    'imm r21, 18185' 'imm r21, 18186'
  statement_family_build_tooth statement-shape-wrong-child-cutpoint \
    'imm r22, 44956' 'imm r22, 44957'
  statement_family_build_tooth statement-shape-wrong-gen-stmts-target \
    'imm r23, 44066' 'imm r23, 44067'
  statement_family_build_tooth statement-shape-wrong-state-label-call \
    'imm r21, 45564' 'imm r21, 45565'
  statement_family_build_tooth statement-shape-wrong-block-call \
    'imm r23, 45275' 'imm r23, 45276'
  statement_family_build_tooth statement-shape-wrong-to-guard \
    'imm r24, 46356' 'imm r24, 46357'
  statement_family_build_tooth statement-shape-wrong-state-dispatch \
    'imm r24, 45774' 'imm r24, 45775'
  statement_family_build_tooth statement-shape-wrong-final-return \
    'imm r20, 576' 'imm r20, 577'
  statement_family_build_tooth statement-shape-wrong-memory-census \
    'imm r25, 83' 'imm r25, 82'
  statement_family_build_tooth statement-shape-wrong-publication \
    'imm r2, 1279808339              ; SSHL' \
    'imm r2, 1279808338              ; SSHL'
}

statement_family_reject_teeth() {
  for statement_tooth_name in \
    statement-shape-wrong-family-count \
    statement-shape-wrong-gen-store-entry \
    statement-shape-wrong-child-cutpoint \
    statement-shape-wrong-gen-stmts-target \
    statement-shape-wrong-state-label-call \
    statement-shape-wrong-block-call \
    statement-shape-wrong-to-guard \
    statement-shape-wrong-state-dispatch \
    statement-shape-wrong-final-return \
    statement-shape-wrong-memory-census \
    statement-shape-wrong-publication
  do
    set +e
    "$T/$statement_tooth_name" < "$T/control.bundle" > "$T/stdout"
    statement_tooth_status=$?
    set -e
    if [ "$statement_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $statement_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
