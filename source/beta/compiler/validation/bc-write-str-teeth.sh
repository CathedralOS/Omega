#!/usr/bin/env sh
# Extracted Checker-A historical canaries for write-str.

write_str_build_teeth() {
# Phase-isolated __write_str-summary teeth keep the exact helper and all 113
# emit macros unchanged while breaking byte provenance, rank, backedge rename,
# or exhaustive literal-byte accounting inside only the new relational phase.
sed 's/imm r2, 1                    ; checked load endpoint K/imm r2, 2                    ; checked load endpoint K/' \
  "$T/control-check.alpha" > "$T/write-str-wrong-byte.alpha"
"$ASM" < "$T/write-str-wrong-byte.alpha" > "$T/write-str-wrong-byte.tape"
stamp_seed "$T/write-str-wrong-byte.tape" "$SEED" "$T/write-str-wrong-byte" >/dev/null
sed 's/imm r3, 1                    ; checked rank delta/imm r3, 0                    ; checked rank delta/' \
  "$T/control-check.alpha" > "$T/write-str-zero-rank.alpha"
"$ASM" < "$T/write-str-zero-rank.alpha" > "$T/write-str-zero-rank.tape"
stamp_seed "$T/write-str-zero-rank.tape" "$SEED" "$T/write-str-zero-rank" >/dev/null
sed 's/imm r2, 1                    ; checked renamed output segment/imm r2, 2                    ; checked renamed output segment/' \
  "$T/control-check.alpha" > "$T/write-str-wrong-rename.alpha"
"$ASM" < "$T/write-str-wrong-rename.alpha" > "$T/write-str-wrong-rename.tape"
stamp_seed "$T/write-str-wrong-rename.tape" "$SEED" "$T/write-str-wrong-rename" >/dev/null
sed '/write_str_emit_byte_count:/,/write_str_emit_done:/{s/imm r1, 829/imm r1, 828/;}' \
  "$T/control-check.alpha" > "$T/write-str-wrong-total.alpha"
"$ASM" < "$T/write-str-wrong-total.alpha" > "$T/write-str-wrong-total.tape"
stamp_seed "$T/write-str-wrong-total.tape" "$SEED" "$T/write-str-wrong-total" >/dev/null
sed 's/imm r20, 70                   ; checked positive cost step/imm r20, 69                   ; checked positive cost step/' \
  "$T/control-check.alpha" > "$T/write-str-wrong-cost.alpha"
"$ASM" < "$T/write-str-wrong-cost.alpha" > "$T/write-str-wrong-cost.tape"
stamp_seed "$T/write-str-wrong-cost.tape" "$SEED" "$T/write-str-wrong-cost" >/dev/null

}

write_str_reject_teeth() {
for write_str_tooth in write-str-wrong-byte write-str-zero-rank write-str-wrong-rename write-str-wrong-total write-str-wrong-cost; do
  set +e
  "$T/$write_str_tooth" < "$T/control.bundle" > "$T/stdout"
  write_str_tooth_status=$?
  set -e
  if [ "$write_str_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $write_str_tooth was not rejected" >&2
    exit 1
  fi
done
}
