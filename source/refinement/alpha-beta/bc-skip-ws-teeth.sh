#!/usr/bin/env sh
# Extracted Checker-A historical canaries for skip-ws.

skip_ws_build_teeth() {
# Phase-isolated skip_ws teeth retain both exact procedures and every imported
# cursor-leaf clause while breaking one continuation, value handoff, same-cursor
# fact, terminal result, inner/outer progress fact, or exhaustive event census.
sed 's/imm r22, 4305                 ; checked skip call continuation/imm r22, 4306                 ; checked skip call continuation/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-continuation.alpha"
"$ASM" < "$T/skip-ws-wrong-continuation.alpha" > "$T/skip-ws-wrong-continuation.tape"
stamp_seed "$T/skip-ws-wrong-continuation.tape" "$SEED" "$T/skip-ws-wrong-continuation" >/dev/null
sed 's/imm r1, 526408                ; checked is_space argument handoff/imm r1, 526416                ; checked is_space argument handoff/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-argument.alpha"
"$ASM" < "$T/skip-ws-wrong-argument.alpha" > "$T/skip-ws-wrong-argument.tape"
stamp_seed "$T/skip-ws-wrong-argument.tape" "$SEED" "$T/skip-ws-wrong-argument" >/dev/null
sed 's/imm r2, 1                    ; checked same-cursor cbyte pair/imm r2, 2                    ; checked same-cursor cbyte pair/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-cursor.alpha"
"$ASM" < "$T/skip-ws-wrong-cursor.alpha" > "$T/skip-ws-wrong-cursor.tape"
stamp_seed "$T/skip-ws-wrong-cursor.tape" "$SEED" "$T/skip-ws-wrong-cursor" >/dev/null
sed 's/imm r20, 1                    ; checked comment-newline result/imm r20, 0                    ; checked comment-newline result/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-newline.alpha"
"$ASM" < "$T/skip-ws-wrong-newline.alpha" > "$T/skip-ws-wrong-newline.tape"
stamp_seed "$T/skip-ws-wrong-newline.tape" "$SEED" "$T/skip-ws-wrong-newline" >/dev/null
sed 's/imm r20, 0                    ; checked comment-zero result/imm r20, 1                    ; checked comment-zero result/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-zero.alpha"
"$ASM" < "$T/skip-ws-wrong-zero.alpha" > "$T/skip-ws-wrong-zero.tape"
stamp_seed "$T/skip-ws-wrong-zero.tape" "$SEED" "$T/skip-ws-wrong-zero" >/dev/null
sed 's/imm r2, 1                    ; checked comment rank delta/imm r2, 0                    ; checked comment rank delta/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-inner-rank.alpha"
"$ASM" < "$T/skip-ws-zero-inner-rank.alpha" > "$T/skip-ws-zero-inner-rank.tape"
stamp_seed "$T/skip-ws-zero-inner-rank.tape" "$SEED" "$T/skip-ws-zero-inner-rank" >/dev/null
sed 's/imm r21, 2                    ; checked result-one cursor progress/imm r21, 1                    ; checked result-one cursor progress/' \
  "$T/control-check.alpha" > "$T/skip-ws-no-step-progress.alpha"
"$ASM" < "$T/skip-ws-no-step-progress.alpha" > "$T/skip-ws-no-step-progress.tape"
stamp_seed "$T/skip-ws-no-step-progress.tape" "$SEED" "$T/skip-ws-no-step-progress" >/dev/null
sed 's/call skip_outer_exit          ; checked ordinary result-zero exit/call skip_outer_backedge      ; checked ordinary result-zero exit/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-backedge.alpha"
"$ASM" < "$T/skip-ws-zero-backedge.alpha" > "$T/skip-ws-zero-backedge.tape"
stamp_seed "$T/skip-ws-zero-backedge.tape" "$SEED" "$T/skip-ws-zero-backedge" >/dev/null
sed 's/imm r2, 1                    ; checked outer rank decrease/imm r2, 0                    ; checked outer rank decrease/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-outer-rank.alpha"
"$ASM" < "$T/skip-ws-zero-outer-rank.alpha" > "$T/skip-ws-zero-outer-rank.tape"
stamp_seed "$T/skip-ws-zero-outer-rank.tape" "$SEED" "$T/skip-ws-zero-outer-rank" >/dev/null
sed 's/imm r29, 33                   ; checked exclusive step event row/imm r29, 32                   ; checked exclusive step event row/' \
  "$T/control-check.alpha" > "$T/skip-ws-event-undercount.alpha"
"$ASM" < "$T/skip-ws-event-undercount.alpha" > "$T/skip-ws-event-undercount.tape"
stamp_seed "$T/skip-ws-event-undercount.tape" "$SEED" "$T/skip-ws-event-undercount" >/dev/null
sed 's/imm r1, 526320                ; checked domain-preserving reset bound/imm r1, 526328                ; checked domain-preserving reset bound/' \
  "$T/control-check.alpha" > "$T/skip-ws-drop-domain.alpha"
"$ASM" < "$T/skip-ws-drop-domain.alpha" > "$T/skip-ws-drop-domain.tape"
stamp_seed "$T/skip-ws-drop-domain.tape" "$SEED" "$T/skip-ws-drop-domain" >/dev/null
sed 's/imm r1, 526248                ; checked opening local c provenance/imm r1, 526256                ; checked opening local c provenance/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-opening.alpha"
"$ASM" < "$T/skip-ws-wrong-opening.alpha" > "$T/skip-ws-wrong-opening.tape"
stamp_seed "$T/skip-ws-wrong-opening.tape" "$SEED" "$T/skip-ws-wrong-opening" >/dev/null
sed 's/imm r2, 1                    ; checked inner rank premise LEN-CUR/imm r2, 0                    ; checked inner rank premise LEN-CUR/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-inner-premise.alpha"
"$ASM" < "$T/skip-ws-zero-inner-premise.alpha" > "$T/skip-ws-zero-inner-premise.tape"
stamp_seed "$T/skip-ws-zero-inner-premise.tape" "$SEED" "$T/skip-ws-zero-inner-premise" >/dev/null
sed 's/imm r2, 1                    ; checked renamed comment cursor/imm r2, 2                    ; checked renamed comment cursor/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-inner-rename.alpha"
"$ASM" < "$T/skip-ws-wrong-inner-rename.alpha" > "$T/skip-ws-wrong-inner-rename.tape"
stamp_seed "$T/skip-ws-wrong-inner-rename.tape" "$SEED" "$T/skip-ws-wrong-inner-rename" >/dev/null
sed 's/imm r2, 1                    ; checked outer rank premise LEN-CUR/imm r2, 0                    ; checked outer rank premise LEN-CUR/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-outer-premise.alpha"
"$ASM" < "$T/skip-ws-zero-outer-premise.alpha" > "$T/skip-ws-zero-outer-premise.tape"
stamp_seed "$T/skip-ws-zero-outer-premise.tape" "$SEED" "$T/skip-ws-zero-outer-premise" >/dev/null
sed 's/imm r2, 1                    ; checked renamed outer cursor/imm r2, 2                    ; checked renamed outer cursor/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-outer-rename.alpha"
"$ASM" < "$T/skip-ws-wrong-outer-rename.alpha" > "$T/skip-ws-wrong-outer-rename.tape"
stamp_seed "$T/skip-ws-wrong-outer-rename.tape" "$SEED" "$T/skip-ws-wrong-outer-rename" >/dev/null

}

skip_ws_reject_teeth() {
for skip_ws_tooth in skip-ws-wrong-continuation skip-ws-wrong-argument skip-ws-wrong-cursor skip-ws-wrong-newline skip-ws-wrong-zero skip-ws-zero-inner-rank skip-ws-no-step-progress skip-ws-zero-backedge skip-ws-zero-outer-rank skip-ws-event-undercount skip-ws-drop-domain skip-ws-wrong-opening skip-ws-zero-inner-premise skip-ws-wrong-inner-rename skip-ws-zero-outer-premise skip-ws-wrong-outer-rename; do
  set +e
  "$T/$skip_ws_tooth" < "$T/control.bundle" > "$T/stdout"
  skip_ws_tooth_status=$?
  set -e
  if [ "$skip_ws_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $skip_ws_tooth was not rejected" >&2
    exit 1
  fi
done
}
