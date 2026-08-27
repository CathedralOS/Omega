#!/usr/bin/env sh
# Phase-isolated seam canaries for the whole-root maximal observable theorem.

root_observation_build_tooth() { # name exact-old exact-new
  root_observation_tooth_name=$1
  root_observation_tooth_old=$2
  root_observation_tooth_new=$3
  root_observation_tooth_count=$(grep -F -c -- \
    "$root_observation_tooth_old" "$T/root-observation.alpha" || true)
  if [ "$root_observation_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $root_observation_tooth_name anchor count $root_observation_tooth_count" >&2
    exit 1
  fi
  awk -v old="$root_observation_tooth_old" \
      -v new="$root_observation_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/root-observation.alpha" > \
    "$T/$root_observation_tooth_name.alpha"
  "$ASM" < "$T/$root_observation_tooth_name.alpha" > \
    "$T/$root_observation_tooth_name.tape"
  stamp_seed "$T/$root_observation_tooth_name.tape" "$SEED" \
    "$T/$root_observation_tooth_name" >/dev/null
}

root_observation_build_antecedent_teeth() {
  root_observation_build_tooth root-observation-wrong-pprc-tag \
    'imm r12, 1129467984             ; PPRC' \
    'imm r12, 1129467983             ; PPRC'
  root_observation_build_tooth root-observation-wrong-rcls-tag \
    'imm r12, 1397506898             ; RCLS' \
    'imm r12, 1397506897             ; RCLS'
  root_observation_build_tooth root-observation-wrong-swsx-tag \
    'imm r12, 1398231896             ; SWSX' \
    'imm r12, 1398231895             ; SWSX'
  root_observation_build_tooth root-observation-wrong-astr-tag \
    'imm r12, 1381258049             ; ASTR' \
    'imm r12, 1381258048             ; ASTR'
  root_observation_build_tooth root-observation-wrong-spfx-tag \
    'imm r12, 1481003091             ; SPFX' \
    'imm r12, 1481003090             ; SPFX'
  root_observation_build_tooth root-observation-wrong-decw-tag \
    'imm r12, 1464026436             ; DECW' \
    'imm r12, 1464026435             ; DECW'
}

root_observation_build_shape_teeth() {
  root_observation_build_tooth root-observation-wrong-parse-target \
    'imm r25, 48993                 ; exact parse_proc target' \
    'imm r25, 48994                 ; exact parse_proc target'
  root_observation_build_tooth root-observation-wrong-resource-target \
    'imm r23, 354                   ; exact resource block target' \
    'imm r23, 353                   ; exact resource block target'
  root_observation_build_tooth root-observation-wrong-success-backedge \
    'imm r21, 51262                 ; exact success backedge target' \
    'imm r21, 51263                 ; exact success backedge target'
}

root_observation_build_resource_teeth() {
  root_observation_build_tooth root-observation-status-as-source-kind \
    'imm r11, 1                    ; SourceBytes' \
    'imm r11, 2                    ; SourceBytes'
  root_observation_build_tooth root-observation-invert-source-projection \
    'imm r17, 253                  ; raw source projection only' \
    'imm r17, 252                  ; raw source projection only'
  root_observation_build_tooth root-observation-drop-sticky-provenance \
    'store r20, r2                ; exact rho remains sticky' \
    'store r20, r17               ; exact rho remains sticky'
  root_observation_build_tooth root-observation-clamp-symbolic-request \
    'imm r19, 1048580              ; producer symbolic request upper' \
    'imm r19, 1025                 ; producer symbolic request upper'
}

root_observation_build_resource_join_teeth() {
  root_observation_build_tooth root-observation-drop-resource-join \
    'imm r2, 1313819218             ; RJON' \
    'imm r2, 1313819217             ; RJON'
  root_observation_build_tooth root-observation-cleanup-emits-byte \
    'imm r2, 0                      ; unique anchor: cleanup stdout delta zero' \
    'imm r2, 1                      ; unique anchor: cleanup stdout delta zero'
  root_observation_build_tooth root-observation-resource-origin-fk-mismatch \
    'imm r13, 2                    ; producer origin2 RCLS foreign key' \
    'imm r13, 3                    ; producer origin2 RCLS foreign key'
  root_observation_build_tooth root-observation-resource-status-fk-mismatch \
    'imm r24, 252                  ; producer RCLS process projection' \
    'imm r24, 253                  ; producer RCLS process projection'
  root_observation_build_tooth root-observation-drop-resource-join-row \
    'imm r2, 5                      ; unique anchor: exact join row count' \
    'imm r2, 4                      ; unique anchor: exact join row count'
  root_observation_build_tooth root-observation-truncate-cleanup-trace \
    'store r20, r2                  ; maximal trace plus cleanup epsilon' \
    'store r20, r10                 ; maximal trace plus cleanup epsilon'
}

root_observation_build_iteration_teeth() {
  root_observation_build_tooth root-observation-collapse-miss-domain \
    'imm r30, 2                    ; LEN<=CUR<=LEN+2' \
    'imm r30, 1                    ; LEN<=CUR<=LEN+2'
  root_observation_build_tooth root-observation-reuse-swsq-on-overshoot \
    'imm r12, 2                    ; SWSX, LEN<CUR<=LEN+2' \
    'imm r12, 1                    ; SWSX, LEN<CUR<=LEN+2'
  root_observation_build_tooth root-observation-drop-iteration-guard \
    'imm r17, 1                    ; completed parse/guard/skip/backedge Tau' \
    'imm r17, 0                    ; completed parse/guard/skip/backedge Tau'
  root_observation_build_tooth root-observation-loop-after-resource \
    'imm r17, 0                    ; terminal, not a coinductive step' \
    'imm r17, 1                    ; terminal, not a coinductive step'
  root_observation_build_tooth root-observation-append-div-suffix \
    'imm r12, 0                    ; Div appends no suffix' \
    'imm r12, 1                    ; Div appends no suffix'
  root_observation_build_tooth root-observation-truncate-div-trace \
    'imm r15, 2                    ; maximal PPRC divergence trace' \
    'imm r15, 1                    ; maximal PPRC divergence trace'
  root_observation_build_tooth root-observation-wrong-resource-iteration-join \
    'imm r19, 5                    ; RJON row5' \
    'imm r19, 4                    ; RJON row5'
  root_observation_build_tooth root-observation-drop-resource-terminal \
    'imm r14, 2                    ; typed Exhaust after joined cleanup' \
    'imm r14, 1                    ; typed Exhaust after joined cleanup'
  root_observation_build_tooth root-observation-drop-first-failure \
    'imm r16, 1                    ; sticky first-failure provenance' \
    'imm r16, 0                    ; sticky first-failure provenance'
}

root_observation_build_gfp_teeth() {
  root_observation_build_tooth root-observation-use-least-fixed-point \
    'imm r2, 2                    ; greatest fixed point, not least' \
    'imm r2, 1                    ; greatest fixed point, not least'
  root_observation_build_tooth root-observation-add-cursor-productivity \
    'store r1, r2                  ; cursor-productivity premise absent' \
    'store r1, r15                 ; cursor-productivity premise absent'
  root_observation_build_tooth root-observation-drop-source-exhaust \
    'imm r2, 1                    ; source overflow -> origin1 Exhaust' \
    'imm r2, 0                    ; source overflow -> origin1 Exhaust'
}

root_observation_build_observable_teeth() {
  root_observation_build_tooth root-observation-admit-trap \
    'store r20, r2                ; no Trap under named bases' \
    'store r20, r10               ; no Trap under named bases'
  root_observation_build_tooth root-observation-admit-stuck \
    'store r20, r2                ; no stuck/OOB under named bases' \
    'store r20, r10               ; no stuck/OOB under named bases'
  root_observation_build_tooth root-observation-drop-spfx-safety-basis \
    'store r20, r2                ; stable SPFX stack antecedent' \
    'store r20, r10               ; stable SPFX stack antecedent'
  root_observation_build_tooth root-observation-drop-decw-safety-basis \
    'imm r15, 1                     ; DECW closes reachable div/rem' \
    'imm r15, 0                     ; DECW closes reachable div/rem'
  root_observation_build_tooth root-observation-break-trace-equality \
    'store r20, r2                ; identical trace DAG/supremum' \
    'store r20, r10               ; identical trace DAG/supremum'
  root_observation_build_tooth root-observation-truncate-maximal-observation \
    'store r20, r2                ; maximal, not arbitrary finite prefix' \
    'store r20, r10               ; maximal, not arbitrary finite prefix'
  root_observation_build_tooth root-observation-wrong-publication \
    'imm r2, 1414483794             ; ROOT' \
    'imm r2, 1414483793             ; ROOT'
}

root_observation_build_memory_safety_teeth() {
  root_observation_build_tooth root-observation-memory-safety-missing-cbyte \
    'imm r11, 13                   ; producer row1 cbyte memory row' \
    'imm r11, 14                   ; producer row1 cbyte memory row'
  root_observation_build_tooth root-observation-memory-safety-duplicate-row \
    'imm r11, 54                   ; producer emit-ident memory row' \
    'imm r11, 50                   ; producer emit-ident memory row'
  root_observation_build_tooth root-observation-memory-safety-wrong-pc \
    'imm r14, 1896                 ; producer row1 exact load PC' \
    'imm r14, 1897                 ; producer row1 exact load PC'
  root_observation_build_tooth root-observation-memory-safety-wrong-idch-class \
    'imm r15, 2                    ; producer IDCH dynamic class' \
    'imm r15, 1                    ; producer IDCH dynamic class'
  root_observation_build_tooth root-observation-memory-safety-source-oob \
    'imm r22, 1048575              ; producer source maximum index' \
    'imm r22, 1048576              ; producer source maximum index'
  root_observation_build_tooth root-observation-memory-safety-table-oob \
    'imm r22, 1023                 ; producer table maximum index' \
    'imm r22, 1024                 ; producer table maximum index'
  root_observation_build_tooth root-observation-memory-safety-wrong-table-base \
    'imm r17, 3153920              ; producer NAMELEN base' \
    'imm r17, 3145728              ; producer NAMELEN base'
  root_observation_build_tooth root-observation-memory-safety-wrong-idch-tag \
    'imm r25, 1212367945           ; producer IDCH semantic tag' \
    'imm r25, 1128421445           ; producer IDCH semantic tag'
  root_observation_build_tooth root-observation-memory-safety-wrong-name-guard \
    'imm r27, 1296323662           ; producer NTDM guard tag' \
    'imm r27, 1396984146           ; producer NTDM guard tag'
  root_observation_build_tooth root-observation-memory-safety-undercount \
    'imm r1, 7                    ; unique anchor: exact dynamic load count' \
    'imm r1, 6                    ; unique anchor: exact dynamic load count'
  root_observation_build_tooth root-observation-memory-safety-wrong-publication \
    'imm r2, 1178686285             ; MSAF' \
    'imm r2, 1178686284             ; MSAF'
}

root_observation_build_teeth() {
  root_observation_build_antecedent_teeth
  root_observation_build_shape_teeth
  root_observation_build_resource_teeth
  root_observation_build_resource_join_teeth
  root_observation_build_iteration_teeth
  root_observation_build_gfp_teeth
  root_observation_build_memory_safety_teeth
  root_observation_build_observable_teeth
}

root_observation_reject_teeth() {
  for root_observation_tooth_name in \
    root-observation-wrong-pprc-tag \
    root-observation-wrong-rcls-tag \
    root-observation-wrong-swsx-tag \
    root-observation-wrong-astr-tag \
    root-observation-wrong-spfx-tag \
    root-observation-wrong-decw-tag \
    root-observation-wrong-parse-target \
    root-observation-wrong-resource-target \
    root-observation-wrong-success-backedge \
    root-observation-status-as-source-kind \
    root-observation-invert-source-projection \
    root-observation-drop-sticky-provenance \
    root-observation-clamp-symbolic-request \
    root-observation-drop-resource-join \
    root-observation-cleanup-emits-byte \
    root-observation-resource-origin-fk-mismatch \
    root-observation-resource-status-fk-mismatch \
    root-observation-drop-resource-join-row \
    root-observation-truncate-cleanup-trace \
    root-observation-collapse-miss-domain \
    root-observation-reuse-swsq-on-overshoot \
    root-observation-drop-iteration-guard \
    root-observation-loop-after-resource \
    root-observation-append-div-suffix \
    root-observation-truncate-div-trace \
    root-observation-wrong-resource-iteration-join \
    root-observation-drop-resource-terminal \
    root-observation-drop-first-failure \
    root-observation-use-least-fixed-point \
    root-observation-add-cursor-productivity \
    root-observation-drop-source-exhaust \
    root-observation-memory-safety-missing-cbyte \
    root-observation-memory-safety-duplicate-row \
    root-observation-memory-safety-wrong-pc \
    root-observation-memory-safety-wrong-idch-class \
    root-observation-memory-safety-source-oob \
    root-observation-memory-safety-table-oob \
    root-observation-memory-safety-wrong-table-base \
    root-observation-memory-safety-wrong-idch-tag \
    root-observation-memory-safety-wrong-name-guard \
    root-observation-memory-safety-undercount \
    root-observation-memory-safety-wrong-publication \
    root-observation-admit-trap \
    root-observation-admit-stuck \
    root-observation-drop-spfx-safety-basis \
    root-observation-drop-decw-safety-basis \
    root-observation-break-trace-equality \
    root-observation-truncate-maximal-observation \
    root-observation-wrong-publication
  do
    set +e
    "$T/$root_observation_tooth_name" \
      < "$T/control.bundle" > "$T/stdout"
    root_observation_tooth_status=$?
    set -e
    if [ "$root_observation_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $root_observation_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
