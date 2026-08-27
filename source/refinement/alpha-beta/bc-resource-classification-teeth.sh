#!/usr/bin/env sh
# Phase-isolated canaries for exact checked-resource classification.

resource_classification_build_tooth() { # name exact-old exact-new
  resource_tooth_name=$1
  resource_tooth_old=$2
  resource_tooth_new=$3
  resource_tooth_count=$(grep -F -c -- "$resource_tooth_old" \
    "$T/resource-classification.alpha" || true)
  if [ "$resource_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $resource_tooth_name anchor count $resource_tooth_count" >&2
    exit 1
  fi
  awk -v old="$resource_tooth_old" -v new="$resource_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/resource-classification.alpha" > "$T/$resource_tooth_name.alpha"
  "$ASM" < "$T/$resource_tooth_name.alpha" > "$T/$resource_tooth_name.tape"
  stamp_seed "$T/$resource_tooth_name.tape" "$SEED" \
    "$T/$resource_tooth_name" >/dev/null
}

resource_classification_build_teeth() {
  resource_classification_build_tooth resource-wrong-source-guard \
    'imm r24, 401                 ; exact n==capacity guard' \
    'imm r24, 402                 ; exact n==capacity guard'
  resource_classification_build_tooth resource-wrong-call-limit-shape \
    'imm r24, 22504' 'imm r24, 22505'
  resource_classification_build_tooth resource-wrong-local-limit-shape \
    'imm r24, 24244' 'imm r24, 24245'
  resource_classification_build_tooth resource-wrong-expression-limit-shape \
    'imm r24, 43233' 'imm r24, 43234'
  resource_classification_build_tooth resource-wrong-block-limit-shape \
    'imm r24, 44165' 'imm r24, 44166'
  resource_classification_build_tooth resource-wrong-parameter-limit-shape \
    'imm r24, 49519' 'imm r24, 49520'
  resource_classification_build_tooth resource-wrong-preflight-limit-shape \
    'imm r24, 50227' 'imm r24, 50228'

  resource_classification_build_tooth resource-wrong-source-profile \
    'imm r12, 1048576                ; SourceBytes' \
    'imm r12, 1048575                ; SourceBytes'
  resource_classification_build_tooth resource-wrong-local-profile \
    'imm r12, 1024                   ; ProcedureLocalSlots' \
    'imm r12, 1023                   ; ProcedureLocalSlots'
  resource_classification_build_tooth resource-wrong-call-profile \
    'imm r12, 4                      ; CallArity' \
    'imm r12, 3                      ; CallArity'
  resource_classification_build_tooth resource-wrong-expression-profile \
    'imm r12, 64                     ; ExpressionCodegenDepth' \
    'imm r12, 63                     ; ExpressionCodegenDepth'
  resource_classification_build_tooth resource-wrong-block-profile \
    'imm r12, 64                     ; BlockCodegenDepth' \
    'imm r12, 63                     ; BlockCodegenDepth'

  resource_classification_build_tooth resource-collapse-source-kind \
    'imm r11, 1                    ; SourceBytes' \
    'imm r11, 2                    ; SourceBytes'
  resource_classification_build_tooth resource-collapse-call-kind \
    'imm r11, 3                    ; CallArity / actual arguments' \
    'imm r11, 4                    ; CallArity / actual arguments'
  resource_classification_build_tooth resource-collapse-declare-kind \
    'imm r11, 2                    ; ProcedureLocalSlots / declare' \
    'imm r11, 3                    ; ProcedureLocalSlots / declare'
  resource_classification_build_tooth resource-swap-expression-kind \
    'imm r11, 4                    ; ExpressionCodegenDepth' \
    'imm r11, 5                    ; ExpressionCodegenDepth'
  resource_classification_build_tooth resource-swap-block-kind \
    'imm r11, 5                    ; BlockCodegenDepth' \
    'imm r11, 4                    ; BlockCodegenDepth'

  resource_classification_build_tooth resource-wrong-source-request \
    'imm r14, 1048577               ; first inadmissible source byte' \
    'imm r14, 1048576               ; first inadmissible source byte'
  resource_classification_build_tooth resource-wrong-call-request \
    'imm r14, 5                     ; fifth actual argument' \
    'imm r14, 4                     ; fifth actual argument'
  resource_classification_build_tooth resource-wrong-declare-request \
    'imm r14, 1025                  ; first unavailable declaration slot' \
    'imm r14, 1024                  ; first unavailable declaration slot'
  resource_classification_build_tooth resource-wrong-expression-request \
    'imm r14, 65                    ; first unavailable expression depth' \
    'imm r14, 64                    ; first unavailable expression depth'
  resource_classification_build_tooth resource-wrong-block-request \
    'imm r14, 65                    ; first unavailable block depth' \
    'imm r14, 64                    ; first unavailable block depth'
  resource_classification_build_tooth resource-wrong-parameter-request \
    'imm r14, 5                     ; fifth formal parameter' \
    'imm r14, 4                     ; fifth formal parameter'
  resource_classification_build_tooth resource-collapse-symbolic-request \
    'imm r13, 2                    ; exact symbolic request' \
    'imm r13, 1                    ; exact symbolic request'
  resource_classification_build_tooth resource-clamp-symbolic-upper \
    'imm r15, 1048580               ; proved nonwrapping nslots upper bound' \
    'imm r15, 1025                  ; proved nonwrapping nslots upper bound'
  resource_classification_build_tooth resource-break-symbolic-identity \
    'imm r16, 528128               ; Requested(nparams+count_lets())' \
    'imm r16, 528129               ; Requested(nparams+count_lets())'

  resource_classification_build_tooth resource-infer-source-from-252 \
    'imm r18, 253                  ; separate process projection' \
    'imm r18, 252                  ; separate process projection'
  resource_classification_build_tooth resource-drop-pre-overlap \
    'store r20, r2                  ; checked before overlapping write' \
    'store r20, r3                  ; checked before overlapping write'
  resource_classification_build_tooth resource-drop-sticky-provenance \
    'store r20, r2                  ; exact origin remains sticky' \
    'store r20, r3                  ; exact origin remains sticky'
  resource_classification_build_tooth resource-drop-owner-custody \
    'store r20, r2                  ; owner trace/state/request custody' \
    'store r20, r3                  ; owner trace/state/request custody'
  resource_classification_build_tooth resource-use-status-as-basis \
    'store r20, r2                  ; basis is guard+profile+request' \
    'store r20, r3                  ; basis is guard+profile+request'
  resource_classification_build_tooth resource-wrong-writer-census \
    'imm r20, 601248               ; XRSC writer-count antecedent value' \
    'imm r20, 601224               ; XRSC writer-count antecedent value'
  resource_classification_build_tooth resource-wrong-origin-count \
    'imm r2, 7                    ; exact checked origin count' \
    'imm r2, 6                    ; exact checked origin count'
  resource_classification_build_tooth resource-wrong-kind-count \
    'imm r2, 5                    ; exact ResourceKind count' \
    'imm r2, 4                    ; exact ResourceKind count'
  resource_classification_build_tooth resource-drop-nonstatus-basis \
    'imm r2, 1                    ; never infer kind from projection' \
    'imm r2, 0                    ; never infer kind from projection'
  resource_classification_build_tooth resource-wrong-publication \
    'imm r2, 1397506898             ; RCLS' \
    'imm r2, 1397506897             ; RCLS'
}

resource_classification_reject_teeth() {
  for resource_tooth_name in \
    resource-wrong-source-guard \
    resource-wrong-call-limit-shape \
    resource-wrong-local-limit-shape \
    resource-wrong-expression-limit-shape \
    resource-wrong-block-limit-shape \
    resource-wrong-parameter-limit-shape \
    resource-wrong-preflight-limit-shape \
    resource-wrong-source-profile \
    resource-wrong-local-profile \
    resource-wrong-call-profile \
    resource-wrong-expression-profile \
    resource-wrong-block-profile \
    resource-collapse-source-kind \
    resource-collapse-call-kind \
    resource-collapse-declare-kind \
    resource-swap-expression-kind \
    resource-swap-block-kind \
    resource-wrong-source-request \
    resource-wrong-call-request \
    resource-wrong-declare-request \
    resource-wrong-expression-request \
    resource-wrong-block-request \
    resource-wrong-parameter-request \
    resource-collapse-symbolic-request \
    resource-clamp-symbolic-upper \
    resource-break-symbolic-identity \
    resource-infer-source-from-252 \
    resource-drop-pre-overlap \
    resource-drop-sticky-provenance \
    resource-drop-owner-custody \
    resource-use-status-as-basis \
    resource-wrong-writer-census \
    resource-wrong-origin-count \
    resource-wrong-kind-count \
    resource-drop-nonstatus-basis \
    resource-wrong-publication
  do
    set +e
    "$T/$resource_tooth_name" < "$T/control.bundle" > "$T/stdout"
    resource_tooth_status=$?
    set -e
    if [ "$resource_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $resource_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
