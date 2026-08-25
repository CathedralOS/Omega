#!/usr/bin/env sh
# Extracted Checker-A historical canaries for literal-skip.

literal_skip_build_teeth() {
# Phase-isolated literal-skip teeth sever exact CFG/call/census facts, the
# bounded ADVX consequence, malformed-tail deltas, or a string fixed-point
# case. Each leaves the exact source, artifact, witness, and prior phases intact.
sed 's/imm r24, 26809              ; checked char backslash guard/imm r24, 26810              ; checked char backslash guard/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-char-guard.alpha"
"$ASM" < "$T/literal-skip-wrong-char-guard.alpha" > "$T/literal-skip-wrong-char-guard.tape"
stamp_seed "$T/literal-skip-wrong-char-guard.tape" "$SEED" "$T/literal-skip-wrong-char-guard" >/dev/null
sed 's/imm r26, 26873              ; checked final char advance continuation/imm r26, 26874              ; checked final char advance continuation/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-char-continuation.alpha"
"$ASM" < "$T/literal-skip-wrong-char-continuation.alpha" > "$T/literal-skip-wrong-char-continuation.tape"
stamp_seed "$T/literal-skip-wrong-char-continuation.tape" "$SEED" "$T/literal-skip-wrong-char-continuation" >/dev/null
sed 's/imm r26, 27365              ; checked escape tail continuation/imm r26, 27366              ; checked escape tail continuation/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-escape-continuation.alpha"
"$ASM" < "$T/literal-skip-wrong-escape-continuation.alpha" > "$T/literal-skip-wrong-escape-continuation.tape"
stamp_seed "$T/literal-skip-wrong-escape-continuation.tape" "$SEED" "$T/literal-skip-wrong-escape-continuation" >/dev/null
sed 's/imm r23, 34                  ; checked closing quote/imm r23, 35                  ; checked closing quote/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-closing-quote.alpha"
"$ASM" < "$T/literal-skip-wrong-closing-quote.alpha" > "$T/literal-skip-wrong-closing-quote.tape"
stamp_seed "$T/literal-skip-wrong-closing-quote.tape" "$SEED" "$T/literal-skip-wrong-closing-quote" >/dev/null
sed 's/imm r29, 297                 ; checked exclusive string event row/imm r29, 296                 ; checked exclusive string event row/' \
  "$T/control-check.alpha" > "$T/literal-skip-event-undercount.alpha"
"$ASM" < "$T/literal-skip-event-undercount.alpha" > "$T/literal-skip-event-undercount.tape"
stamp_seed "$T/literal-skip-event-undercount.tape" "$SEED" "$T/literal-skip-event-undercount" >/dev/null
sed 's/imm r23, 496                 ; checked exclusive string primitive row/imm r23, 495                 ; checked exclusive string primitive row/' \
  "$T/control-check.alpha" > "$T/literal-skip-primitive-undercount.alpha"
"$ASM" < "$T/literal-skip-primitive-undercount.alpha" > "$T/literal-skip-primitive-undercount.tape"
stamp_seed "$T/literal-skip-primitive-undercount.tape" "$SEED" "$T/literal-skip-primitive-undercount" >/dev/null
sed 's/imm r2, 1                    ; checked 0<=CUR<=CAP+1/imm r2, 0                    ; checked 0<=CUR<=CAP+1/' \
  "$T/control-check.alpha" > "$T/literal-skip-drop-advx-bound.alpha"
"$ASM" < "$T/literal-skip-drop-advx-bound.alpha" > "$T/literal-skip-drop-advx-bound.tape"
stamp_seed "$T/literal-skip-drop-advx-bound.tape" "$SEED" "$T/literal-skip-drop-advx-bound" >/dev/null
sed 's/store r1, r2                  ; checked exact CUR+1<=CAP+2/store r1, r1                  ; checked exact CUR+1<=CAP+2/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-advx-successor.alpha"
"$ASM" < "$T/literal-skip-wrong-advx-successor.alpha" > "$T/literal-skip-wrong-advx-successor.tape"
stamp_seed "$T/literal-skip-wrong-advx-successor.tape" "$SEED" "$T/literal-skip-wrong-advx-successor" >/dev/null
sed 's/imm r2, 3                    ; checked ordinary total delta/imm r2, 4                    ; checked ordinary total delta/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-char-delta.alpha"
"$ASM" < "$T/literal-skip-wrong-char-delta.alpha" > "$T/literal-skip-wrong-char-delta.tape"
stamp_seed "$T/literal-skip-wrong-char-delta.tape" "$SEED" "$T/literal-skip-wrong-char-delta" >/dev/null
sed 's/imm r2, 1                    ; checked ordinary final CUR<=LEN+2/imm r2, 2                    ; checked ordinary final CUR<=LEN+2/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-char-bound.alpha"
"$ASM" < "$T/literal-skip-wrong-char-bound.alpha" > "$T/literal-skip-wrong-char-bound.tape"
stamp_seed "$T/literal-skip-wrong-char-bound.tape" "$SEED" "$T/literal-skip-wrong-char-bound" >/dev/null
sed 's/imm r2, 2                    ; checked cursor preserved/imm r2, 1                    ; checked cursor preserved/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-zero-cursor.alpha"
"$ASM" < "$T/literal-skip-wrong-zero-cursor.alpha" > "$T/literal-skip-wrong-zero-cursor.tape"
stamp_seed "$T/literal-skip-wrong-zero-cursor.tape" "$SEED" "$T/literal-skip-wrong-zero-cursor" >/dev/null
sed 's/imm r2, 1                    ; checked rank decrease by one/imm r2, 0                    ; checked rank decrease by one/' \
  "$T/control-check.alpha" > "$T/literal-skip-zero-ordinary-rank.alpha"
"$ASM" < "$T/literal-skip-zero-ordinary-rank.alpha" > "$T/literal-skip-zero-ordinary-rank.tape"
stamp_seed "$T/literal-skip-zero-ordinary-rank.tape" "$SEED" "$T/literal-skip-zero-ordinary-rank" >/dev/null
sed 's/imm r2, 2                    ; checked rank decrease by two/imm r2, 1                    ; checked rank decrease by two/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-escape-rank.alpha"
"$ASM" < "$T/literal-skip-wrong-escape-rank.alpha" > "$T/literal-skip-wrong-escape-rank.tape"
stamp_seed "$T/literal-skip-wrong-escape-rank.tape" "$SEED" "$T/literal-skip-wrong-escape-rank" >/dev/null
sed 's/store r1, r2                  ; checked smaller rank renamed/store r1, r1                  ; checked smaller rank renamed/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-backedge-rename.alpha"
"$ASM" < "$T/literal-skip-wrong-backedge-rename.alpha" > "$T/literal-skip-wrong-backedge-rename.tape"
stamp_seed "$T/literal-skip-wrong-backedge-rename.tape" "$SEED" "$T/literal-skip-wrong-backedge-rename" >/dev/null
}

literal_skip_reject_teeth() {
for literal_skip_tooth in literal-skip-wrong-char-guard literal-skip-wrong-char-continuation literal-skip-wrong-escape-continuation literal-skip-wrong-closing-quote literal-skip-event-undercount literal-skip-primitive-undercount literal-skip-drop-advx-bound literal-skip-wrong-advx-successor literal-skip-wrong-char-delta literal-skip-wrong-char-bound literal-skip-wrong-zero-cursor literal-skip-zero-ordinary-rank literal-skip-wrong-escape-rank literal-skip-wrong-backedge-rename; do
  set +e
  "$T/$literal_skip_tooth" < "$T/control.bundle" > "$T/stdout"
  literal_skip_tooth_status=$?
  set -e
  if [ "$literal_skip_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $literal_skip_tooth was not rejected" >&2
    exit 1
  fi
done
}
