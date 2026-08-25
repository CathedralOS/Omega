#!/usr/bin/env sh
# Extracted Checker-A historical canaries for cursor-leaf.

cursor_leaf_build_teeth() {
# Phase-isolated cursor-leaf teeth preserve the exact procedures and preceding
# summaries while severing source-index provenance, reversing the cbyte miss
# partition, admitting a zero cursor delta, dropping CR, or classifying the
# complement as whitespace in only the new relational phase.
sed 's/imm r21, 1                    ; checked SRC index from local i/imm r21, 2                    ; checked SRC index from local i/' \
  "$T/control-check.alpha" > "$T/cursor-cbyte-wrong-index.alpha"
"$ASM" < "$T/cursor-cbyte-wrong-index.alpha" > "$T/cursor-cbyte-wrong-index.tape"
stamp_seed "$T/cursor-cbyte-wrong-index.tape" "$SEED" "$T/cursor-cbyte-wrong-index" >/dev/null
sed 's/imm r2, 2                    ; checked miss relation LEN<=CUR/imm r2, 1                    ; checked miss relation LEN<=CUR/' \
  "$T/control-check.alpha" > "$T/cursor-cbyte-wrong-boundary.alpha"
"$ASM" < "$T/cursor-cbyte-wrong-boundary.alpha" > "$T/cursor-cbyte-wrong-boundary.tape"
stamp_seed "$T/cursor-cbyte-wrong-boundary.tape" "$SEED" "$T/cursor-cbyte-wrong-boundary" >/dev/null
sed 's/imm r2, 1                    ; checked CUR increment delta/imm r2, 0                    ; checked CUR increment delta/' \
  "$T/control-check.alpha" > "$T/cursor-adv-zero-delta.alpha"
"$ASM" < "$T/cursor-adv-zero-delta.alpha" > "$T/cursor-adv-zero-delta.tape"
stamp_seed "$T/cursor-adv-zero-delta.tape" "$SEED" "$T/cursor-adv-zero-delta" >/dev/null
sed 's/imm r21, 1                    ; checked CR is whitespace/imm r21, 0                    ; checked CR is whitespace/' \
  "$T/control-check.alpha" > "$T/cursor-space-drop-cr.alpha"
"$ASM" < "$T/cursor-space-drop-cr.alpha" > "$T/cursor-space-drop-cr.tape"
stamp_seed "$T/cursor-space-drop-cr.tape" "$SEED" "$T/cursor-space-drop-cr" >/dev/null
sed 's/imm r20, 2                    ; checked other-result kind/imm r20, 1                    ; checked other-result kind/' \
  "$T/control-check.alpha" > "$T/cursor-space-zero-is-space.alpha"
"$ASM" < "$T/cursor-space-zero-is-space.alpha" > "$T/cursor-space-zero-is-space.tape"
stamp_seed "$T/cursor-space-zero-is-space.tape" "$SEED" "$T/cursor-space-zero-is-space" >/dev/null
sed 's/imm r23, 17                   ; checked exclusive local row/imm r23, 16                   ; checked exclusive local row/' \
  "$T/control-check.alpha" > "$T/cursor-effect-undercount.alpha"
"$ASM" < "$T/cursor-effect-undercount.alpha" > "$T/cursor-effect-undercount.tape"
stamp_seed "$T/cursor-effect-undercount.tape" "$SEED" "$T/cursor-effect-undercount" >/dev/null

}

cursor_leaf_reject_teeth() {
for cursor_leaf_tooth in cursor-cbyte-wrong-index cursor-cbyte-wrong-boundary cursor-adv-zero-delta cursor-space-drop-cr cursor-space-zero-is-space cursor-effect-undercount; do
  set +e
  "$T/$cursor_leaf_tooth" < "$T/control.bundle" > "$T/stdout"
  cursor_leaf_tooth_status=$?
  set -e
  if [ "$cursor_leaf_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $cursor_leaf_tooth was not rejected" >&2
    exit 1
  fi
done
}
