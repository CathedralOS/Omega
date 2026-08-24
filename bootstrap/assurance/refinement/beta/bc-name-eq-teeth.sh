#!/usr/bin/env sh
# Phase-isolated canaries for the bounded NPFX/QNAME domain and name_eq theorem.

name_eq_build_tooth() { # name exact-old exact-new
  name_eq_tooth_name=$1
  name_eq_tooth_old=$2
  name_eq_tooth_new=$3
  name_eq_tooth_count=$(grep -F -c -- "$name_eq_tooth_old" "$T/name-eq-check.alpha" || true)
  if [ "$name_eq_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $name_eq_tooth_name anchor count $name_eq_tooth_count" >&2
    exit 1
  fi
  sed "s|$name_eq_tooth_old|$name_eq_tooth_new|" \
    "$T/name-eq-check.alpha" > "$T/$name_eq_tooth_name.alpha"
  "$ASM" < "$T/$name_eq_tooth_name.alpha" > "$T/$name_eq_tooth_name.tape"
  stamp_seed "$T/$name_eq_tooth_name.tape" "$SEED" "$T/$name_eq_tooth_name" >/dev/null
}

name_eq_build_teeth() {
  name_eq_build_tooth name-eq-helper-undercount \
    'imm r1, 403                  ; exact expression push-row universe' \
    'imm r1, 129                  ; exact expression push-row universe'
  name_eq_build_tooth name-eq-wrong-frame \
    'imm r1, 48                   ; exact proc35 frame bytes' \
    'imm r1, 40                   ; exact proc35 frame bytes'
  name_eq_build_tooth name-eq-wrong-entry-pc \
    'imm r21, 24914                ; exact proc35 entry block' \
    'imm r21, 24915                ; exact proc35 entry block'
  name_eq_build_tooth name-eq-wrong-transition-pc \
    'imm r24, 25171                ; selected NAMELEN equals IDLEN' \
    'imm r24, 25172                ; selected NAMELEN equals IDLEN'
  name_eq_build_tooth name-eq-wrong-return-pc \
    'imm r23, 25219                ; length mismatch returns zero' \
    'imm r23, 25218                ; length mismatch returns zero'
  name_eq_build_tooth name-eq-wrong-epilogue \
    'imm r20, 25200' 'imm r20, 25201'
  name_eq_build_tooth name-eq-wrong-local \
    'imm r24, 24991                ; chk reads i' \
    'imm r24, 24992                ; chk reads i'
  name_eq_build_tooth name-eq-wrong-source-load \
    'imm r24, 25745                ; selected source byte from a plus k' \
    'imm r24, 25746                ; selected source byte from a plus k'
  name_eq_build_tooth name-eq-wrong-equality-op \
    'imm r23, 10                  ; full-word equality' \
    'imm r23, 11                  ; full-word equality'
  name_eq_build_tooth name-eq-wrong-last-push \
    'imm r23, 25988' 'imm r23, 25989'
  name_eq_build_tooth name-eq-wrong-return-census \
    'imm r23, 4                   ; three explicit plus synthetic return' \
    'imm r23, 3                   ; three explicit plus synthetic return'
  name_eq_build_tooth name-eq-wrong-loadb-census \
    'imm r30, 2                   ; the two bound SRC byte loads' \
    'imm r30, 1                   ; the two bound SRC byte loads'
  name_eq_build_tooth name-eq-wrong-loadb-opcode \
    'imm r3, 8                   ; loadb' \
    'imm r3, 7                   ; loadb'
  name_eq_build_tooth name-eq-wrong-primitive-end \
    'imm r23, 477' 'imm r23, 476'
  name_eq_build_tooth name-eq-wrong-load-class \
    'imm r20, 44                  ; exact NAMELEN table load row' \
    'imm r20, 45                  ; exact NAMELEN table load row'
  name_eq_build_tooth name-eq-wrong-address-total \
    'store r1, r10                 ; persistent exact completion count1024' \
    'store r1, r11                 ; persistent exact completion count1024'
  name_eq_build_tooth name-eq-overlap-name-arrays \
    'imm r2, 3153920              ; exact NAMEOFF exclusive extent' \
    'imm r2, 3153919              ; exact NAMEOFF exclusive extent'
  name_eq_build_tooth name-eq-drop-prefix-premise \
    'store r1, r2                  ; conditional NPFX(n), 0<=n<=1024' \
    'store r1, r1                  ; conditional NPFX(n), 0<=n<=1024'
  name_eq_build_tooth name-eq-drop-domain-quiet \
    'store r1, r2                  ; conditional name-domain state quiet' \
    'store r1, r1                  ; conditional name-domain state quiet'
  name_eq_build_tooth name-eq-wrong-domain-token \
    'imm r2, 1296323662            ; NTDM' \
    'imm r2, 1296323661            ; NTDM'
  name_eq_build_tooth name-eq-drop-entry \
    'store r1, r2                  ; exact proc35 entry / selected i' \
    'store r1, r1                  ; exact proc35 entry / selected i'
  name_eq_build_tooth name-eq-drop-short-circuit \
    'store r1, r2                  ; no NAMEOFF or SRC payload access' \
    'store r1, r1                  ; no NAMEOFF or SRC payload access'
  name_eq_build_tooth name-eq-wrong-length-result \
    'imm r21, 1                    ; zero' \
    'imm r21, 2                    ; zero'
  name_eq_build_tooth name-eq-drop-byte-addresses \
    'store r1, r2                  ; exact SRC+a+k / SRC+b+k chains' \
    'store r1, r1                  ; exact SRC+a+k / SRC+b+k chains'
  name_eq_build_tooth name-eq-collapse-byte-mismatch \
    'imm r21, 1                    ; consumed a-vs-b byte provenance' \
    'imm r21, 2                    ; consumed a-vs-b byte provenance'
  name_eq_build_tooth name-eq-zero-rank-step \
    'imm r21, 1                    ; consumed strict rank decrease' \
    'imm r21, 0                    ; consumed strict rank decrease'
  name_eq_build_tooth name-eq-drop-successor-prefix \
    'imm r21, 2                    ; exact successor-prefix tag' \
    'imm r21, 1                    ; exact successor-prefix tag'
  name_eq_build_tooth name-eq-drop-backedge-rename \
    'store r1, r2                  ; checked successor renaming' \
    'store r1, r1                  ; checked successor renaming'
  name_eq_build_tooth name-eq-wrong-full-result \
    'imm r21, 2                    ; one' \
    'imm r21, 1                    ; one'
  name_eq_build_tooth name-eq-drop-quiet \
    'store r1, r2                  ; carried proc35 state/trace quiet' \
    'store r1, r1                  ; carried proc35 state/trace quiet'
  name_eq_build_tooth name-eq-wrong-summary-token \
    'imm r2, 1397839182            ; NEQS' \
    'imm r2, 1397839181            ; NEQS'
}

name_eq_reject_teeth() {
  for name_eq_tooth_name in \
    name-eq-helper-undercount name-eq-wrong-frame name-eq-wrong-entry-pc \
    name-eq-wrong-transition-pc name-eq-wrong-return-pc \
    name-eq-wrong-epilogue name-eq-wrong-local name-eq-wrong-source-load \
    name-eq-wrong-equality-op name-eq-wrong-last-push \
    name-eq-wrong-return-census name-eq-wrong-loadb-census \
    name-eq-wrong-loadb-opcode name-eq-wrong-primitive-end \
    name-eq-wrong-load-class name-eq-wrong-address-total \
    name-eq-overlap-name-arrays name-eq-drop-prefix-premise \
    name-eq-drop-domain-quiet \
    name-eq-wrong-domain-token name-eq-drop-entry \
    name-eq-drop-short-circuit name-eq-wrong-length-result \
    name-eq-drop-byte-addresses name-eq-collapse-byte-mismatch \
    name-eq-zero-rank-step name-eq-drop-successor-prefix \
    name-eq-drop-backedge-rename \
    name-eq-wrong-full-result name-eq-drop-quiet name-eq-wrong-summary-token
  do
    set +e
    "$T/$name_eq_tooth_name" < "$T/control.bundle" > "$T/stdout"
    name_eq_tooth_status=$?
    set -e
    if [ "$name_eq_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $name_eq_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
