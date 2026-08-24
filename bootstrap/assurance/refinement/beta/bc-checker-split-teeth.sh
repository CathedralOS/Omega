#!/usr/bin/env sh
# Phase-isolated canaries for the two post-stack continuation bridges.

checker_split_build_fixed_tooth() {
  checker_split_count=$(grep -F -c -- 'imm r2, 1481003091             ; SPFX' "$T/control-check.alpha" || true)
  if [ "$checker_split_count" != 1 ]; then
    echo "bc block control FAIL — fixed continuation anchor count $checker_split_count" >&2
    exit 1
  fi
  sed 's/imm r2, 1481003091             ; SPFX/imm r2, 1481003090             ; SPFX/' \
    "$T/control-check.alpha" > "$T/fixed-wrong-continuation.alpha"
  "$ASM" < "$T/fixed-wrong-continuation.alpha" > "$T/fixed-wrong-continuation.tape"
  stamp_seed "$T/fixed-wrong-continuation.tape" "$SEED" "$T/fixed-wrong-continuation" >/dev/null
}

checker_split_reject_fixed_tooth() {
  set +e
  "$T/fixed-wrong-continuation" < "$T/control.bundle" > "$T/stdout"
  checker_split_status=$?
  set -e
  if [ "$checker_split_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — fixed continuation tooth was not rejected" >&2
    exit 1
  fi
}

checker_split_build_name_tooth() {
  checker_split_count=$(grep -F -c -- 'imm r2, 1162760275             ; SPNE' "$T/name-eq-check.alpha" || true)
  if [ "$checker_split_count" != 1 ]; then
    echo "bc block control FAIL — name_eq continuation anchor count $checker_split_count" >&2
    exit 1
  fi
  sed 's/imm r2, 1162760275             ; SPNE/imm r2, 1162760274             ; SPNE/' \
    "$T/name-eq-check.alpha" > "$T/name-eq-wrong-continuation.alpha"
  "$ASM" < "$T/name-eq-wrong-continuation.alpha" > "$T/name-eq-wrong-continuation.tape"
  stamp_seed "$T/name-eq-wrong-continuation.tape" "$SEED" "$T/name-eq-wrong-continuation" >/dev/null
}

checker_split_reject_name_tooth() {
  set +e
  "$T/name-eq-wrong-continuation" < "$T/control.bundle" > "$T/stdout"
  checker_split_status=$?
  set -e
  if [ "$checker_split_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — name_eq continuation tooth was not rejected" >&2
    exit 1
  fi
}
