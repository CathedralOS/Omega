#!/usr/bin/env sh
# Phase-isolated canaries for procedures15..23 fixed-keyword shape and meaning.

fixed_keyword_build_tooth() { # name exact-old exact-new
  fixed_keyword_tooth_name=$1
  fixed_keyword_tooth_old=$2
  fixed_keyword_tooth_new=$3
  fixed_keyword_tooth_count=$(grep -F -c -- "$fixed_keyword_tooth_old" "$T/control-check.alpha" || true)
  if [ "$fixed_keyword_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $fixed_keyword_tooth_name anchor count $fixed_keyword_tooth_count" >&2
    exit 1
  fi
  sed "s|$fixed_keyword_tooth_old|$fixed_keyword_tooth_new|" \
    "$T/control-check.alpha" > "$T/$fixed_keyword_tooth_name.alpha"
  "$ASM" < "$T/$fixed_keyword_tooth_name.alpha" > "$T/$fixed_keyword_tooth_name.tape"
  stamp_seed "$T/$fixed_keyword_tooth_name.tape" "$SEED" "$T/$fixed_keyword_tooth_name" >/dev/null
}

fixed_keyword_build_teeth() {
  fixed_keyword_build_tooth fixed-keyword-wrong-return-pc \
    'imm r22, 6617                 ; return procedure pc' \
    'imm r22, 6618                 ; return procedure pc'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-length \
    'imm r21, 6                    ; return keyword length' \
    'imm r21, 7                    ; return keyword length'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-block \
    'imm r23, 54                   ; return first block' \
    'imm r23, 55                   ; return first block'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-transition \
    'imm r24, 40                   ; return first transition' \
    'imm r24, 41                   ; return first transition'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-event \
    'imm r25, 46                   ; return first event' \
    'imm r25, 47                   ; return first event'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-memory \
    'imm r26, 24                   ; return memory row' \
    'imm r26, 25                   ; return memory row'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-primitive \
    'imm r27, 116                  ; return primitive row' \
    'imm r27, 117                  ; return primitive row'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-binary \
    'imm r28, 33                   ; return binary-push row' \
    'imm r28, 34                   ; return binary-push row'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-argument \
    'imm r29, 243                  ; return argument-push row' \
    'imm r29, 244                  ; return argument-push row'
  fixed_keyword_build_tooth fixed-keyword-wrong-spelling \
    'db "return"                       ; fixed-keyword descriptor spelling' \
    'db "retvrn"                       ; fixed-keyword descriptor spelling'
  fixed_keyword_build_tooth fixed-keyword-wrong-idchar-target \
    'imm r25, 5634                ; exact IDCH target' \
    'imm r25, 5635                ; exact IDCH target'
  fixed_keyword_build_tooth fixed-keyword-wrong-call-continuation \
    'add r26, r2                  ; exact call continuation delta' \
    'sub r26, r2                  ; exact call continuation delta'
  fixed_keyword_build_tooth fixed-keyword-drop-event-index-reload \
    'load r30, r1                 ; call checker clobbers the loop index' \
    'load r29, r1                 ; call checker clobbers the loop index'
  fixed_keyword_build_tooth fixed-keyword-wrong-idlen-address \
    'imm r23, 2097112             ; exact IDLEN address' \
    'imm r23, 2097120             ; exact IDLEN address'
  fixed_keyword_build_tooth fixed-keyword-wrong-length-equality \
    'imm r23, 10                  ; length full-word equality' \
    'imm r23, 9                   ; length full-word equality'
  fixed_keyword_build_tooth fixed-keyword-wrong-spelling-width \
    'loadb r23, r23                ; exact spelling byte' \
    'load r23, r23                 ; exact spelling byte'
  fixed_keyword_build_tooth fixed-keyword-wrong-match-literal \
    'imm r23, 1                   ; full-match one literal' \
    'imm r23, 0                   ; full-match one literal'
  fixed_keyword_build_tooth fixed-keyword-wrong-call-census \
    'load r22, r1                 ; exact IDCH call count' \
    'load r23, r1                 ; exact IDCH call count'
  fixed_keyword_build_tooth fixed-keyword-wrong-return-census \
    'add r23, r2                  ; exact explicit+synthetic return count' \
    'sub r23, r2                  ; exact explicit+synthetic return count'
  fixed_keyword_build_tooth fixed-keyword-wrong-store-census \
    'add r24, r2                  ; exact decoded store count' \
    'sub r24, r2                  ; exact decoded store count'
  fixed_keyword_build_tooth fixed-keyword-wrong-store-push-boundary \
    'imm r29, 381                 ; exact empty store-push interval' \
    'imm r29, 382                 ; exact empty store-push interval'

  fixed_keyword_build_tooth fixed-keyword-collapse-byte-spec \
    'jeq r26, r25, fixed_keyword_cases_spec_equal' \
    'jeq r26, r26, fixed_keyword_cases_spec_equal'
  fixed_keyword_build_tooth fixed-keyword-wrong-singleton-count \
    'imm r21, 1                    ; singleton descriptor byte' \
    'imm r21, 2                    ; singleton descriptor byte'
  fixed_keyword_build_tooth fixed-keyword-wrong-complement-count \
    'imm r21, 255                  ; exact byte complement' \
    'imm r21, 254                  ; exact byte complement'
  fixed_keyword_build_tooth fixed-keyword-wrong-position-base \
    'imm r4, 6                    ; state position-table base' \
    'imm r4, 7                    ; state position-table base'
  fixed_keyword_build_tooth fixed-keyword-drop-position-completion \
    'store r4, r3                  ; persistent descriptor-position proof' \
    'store r4, r5                  ; persistent descriptor-position proof'
  fixed_keyword_build_tooth fixed-keyword-wrong-position-total \
    'imm r21, 48                   ; every descriptor position' \
    'imm r21, 47                   ; every descriptor position'

  fixed_keyword_build_tooth fixed-keyword-drop-byte-theorem \
    'imm r21, 1363888966           ; FKEQ byte singleton/complement theorem' \
    'imm r21, 1363888965           ; FKEQ byte singleton/complement theorem'
  fixed_keyword_build_tooth fixed-keyword-length-accesses-byte \
    'imm r2, 0                    ; no IDCH byte access' \
    'imm r2, 1                    ; no IDCH byte access'
  fixed_keyword_build_tooth fixed-keyword-drop-prior-equal \
    'imm r26, 2                    ; consumed prior equal relation' \
    'imm r26, 1                    ; consumed prior equal relation'
  fixed_keyword_build_tooth fixed-keyword-drop-terminal-unequal \
    'imm r26, 1                    ; consumed terminal unequal relation' \
    'imm r26, 2                    ; consumed terminal unequal relation'
  fixed_keyword_build_tooth fixed-keyword-drop-full-match-equal \
    'imm r26, 2                    ; consumed full-match equal relation' \
    'imm r26, 1                    ; consumed full-match equal relation'
  fixed_keyword_build_tooth fixed-keyword-wrong-expected-table \
    'imm r4, 531760               ; recorded expected-byte table' \
    'imm r4, 531752               ; recorded expected-byte table'
  fixed_keyword_build_tooth fixed-keyword-wrong-mismatch-identity \
    'add r20, r2                  ; procedure-local first-mismatch outcome' \
    'sub r20, r2                  ; procedure-local first-mismatch outcome'
  fixed_keyword_build_tooth fixed-keyword-wrong-outcome-base \
    'imm r4, 8                    ; state outcome-table base' \
    'imm r4, 9                    ; state outcome-table base'
  fixed_keyword_build_tooth fixed-keyword-drop-outcome-completion \
    'store r4, r3                  ; persistent selected outcome completion' \
    'store r4, r5                  ; persistent selected outcome completion'
  fixed_keyword_build_tooth fixed-keyword-wrong-outcome-total \
    'imm r21, 66                   ; exact aggregate outcome count' \
    'imm r21, 65                   ; exact aggregate outcome count'
  fixed_keyword_build_tooth fixed-keyword-wrong-procedure-total \
    'imm r21, 9                    ; all recognizers complete' \
    'imm r21, 8                    ; all recognizers complete'
  fixed_keyword_build_tooth fixed-keyword-wrong-descriptor-join \
    'imm r20, 530664              ; selected descriptor-length join' \
    'imm r20, 530656              ; selected descriptor-length join'
  fixed_keyword_build_tooth fixed-keyword-drop-cursor-preservation \
    'store r1, r2                  ; CUR unchanged' \
    'store r1, r1                  ; CUR unchanged'
  fixed_keyword_build_tooth fixed-keyword-wrong-procedure-outcomes \
    'add r21, r2                  ; exact L+2 procedure outcomes' \
    'sub r21, r2                  ; exact L+2 procedure outcomes'
  fixed_keyword_build_tooth fixed-keyword-overlap-byte-join-reset \
    'imm r1, 531680' 'imm r1, 531688'
}

fixed_keyword_reject_teeth() {
  for fixed_keyword_tooth_name in \
    fixed-keyword-wrong-return-pc fixed-keyword-wrong-return-length \
    fixed-keyword-wrong-return-block fixed-keyword-wrong-return-transition \
    fixed-keyword-wrong-return-event fixed-keyword-wrong-return-memory \
    fixed-keyword-wrong-return-primitive fixed-keyword-wrong-return-binary \
    fixed-keyword-wrong-return-argument fixed-keyword-wrong-spelling \
    fixed-keyword-wrong-idchar-target fixed-keyword-wrong-call-continuation \
    fixed-keyword-drop-event-index-reload fixed-keyword-wrong-idlen-address \
    fixed-keyword-wrong-length-equality fixed-keyword-wrong-spelling-width \
    fixed-keyword-wrong-match-literal fixed-keyword-wrong-call-census \
    fixed-keyword-wrong-return-census fixed-keyword-wrong-store-census \
    fixed-keyword-wrong-store-push-boundary fixed-keyword-collapse-byte-spec \
    fixed-keyword-wrong-singleton-count fixed-keyword-wrong-complement-count \
    fixed-keyword-wrong-position-base fixed-keyword-drop-position-completion \
    fixed-keyword-wrong-position-total fixed-keyword-drop-byte-theorem \
    fixed-keyword-length-accesses-byte fixed-keyword-drop-prior-equal \
    fixed-keyword-drop-terminal-unequal fixed-keyword-drop-full-match-equal \
    fixed-keyword-wrong-expected-table fixed-keyword-wrong-mismatch-identity \
    fixed-keyword-wrong-outcome-base fixed-keyword-drop-outcome-completion \
    fixed-keyword-wrong-outcome-total fixed-keyword-wrong-procedure-total \
    fixed-keyword-wrong-descriptor-join fixed-keyword-drop-cursor-preservation \
    fixed-keyword-wrong-procedure-outcomes fixed-keyword-overlap-byte-join-reset
  do
    set +e
    "$T/$fixed_keyword_tooth_name" < "$T/control.bundle" > "$T/stdout"
    fixed_keyword_tooth_status=$?
    set -e
    if [ "$fixed_keyword_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $fixed_keyword_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
