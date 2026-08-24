#!/usr/bin/env sh
# Phase-isolated canaries for conditional procedure36 lookup.

lookup_build_tooth() { # name exact-old exact-new
  lookup_tooth_name=$1
  lookup_tooth_old=$2
  lookup_tooth_new=$3
  lookup_tooth_count=$(grep -F -c -- "$lookup_tooth_old" "$T/lookup-check.alpha" || true)
  if [ "$lookup_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $lookup_tooth_name anchor count $lookup_tooth_count" >&2
    exit 1
  fi
  sed "s|$lookup_tooth_old|$lookup_tooth_new|" \
    "$T/lookup-check.alpha" > "$T/$lookup_tooth_name.alpha"
  "$ASM" < "$T/$lookup_tooth_name.alpha" > "$T/$lookup_tooth_name.tape"
  stamp_seed "$T/$lookup_tooth_name.tape" "$SEED" "$T/$lookup_tooth_name" >/dev/null
}

lookup_build_teeth() {
  lookup_build_tooth lookup-wrong-continuation \
    'imm r2, 1112887120             ; LUPB' \
    'imm r2, 1112887119             ; LUPB'
  lookup_build_tooth lookup-wrong-frame \
    'imm r1, 24                   ; exact proc36 frame bytes' \
    'imm r1, 16                   ; exact proc36 frame bytes'
  lookup_build_tooth lookup-wrong-entry-pc \
    'imm r21, 26114                ; exact proc36 entry block' \
    'imm r21, 26115                ; exact proc36 entry block'
  lookup_build_tooth lookup-wrong-loop-transition \
    'imm r24, 26320                ; i<n branch root' \
    'imm r24, 26321                ; i<n branch root'
  lookup_build_tooth lookup-wrong-call-event \
    'imm r23, 26420                ; name_eq call' \
    'imm r23, 26421                ; name_eq call'
  lookup_build_tooth lookup-wrong-call-target \
    'imm r21, 24914                ; exact name_eq target' \
    'imm r21, 24915                ; exact name_eq target'
  lookup_build_tooth lookup-wrong-call-continuation \
    'imm r22, 26429                ; exact call continuation' \
    'imm r22, 26428                ; exact call continuation'
  lookup_build_tooth lookup-wrong-arity \
    'imm r21, 1                    ; one argument' \
    'imm r21, 0                    ; one argument'
  lookup_build_tooth lookup-wrong-ambient \
    'imm r21, 0                    ; zero ambient expression words' \
    'imm r21, 1                    ; zero ambient expression words'
  lookup_build_tooth lookup-wrong-epilogue \
    'imm r20, 26349' 'imm r20, 26350'
  lookup_build_tooth lookup-wrong-snapshot-local \
    'imm r24, 26159                ; n snapshot store' \
    'imm r24, 26160                ; n snapshot store'
  lookup_build_tooth lookup-wrong-memory \
    'imm r24, 26156                ; exact NLOC snapshot load' \
    'imm r24, 26157                ; exact NLOC snapshot load'
  lookup_build_tooth lookup-wrong-less-than \
    'imm r23, 8                    ; signed i<n' \
    'imm r23, 10                   ; signed i<n'
  lookup_build_tooth lookup-wrong-argument-push \
    'imm r23, 26388                ; exact name_eq argument push' \
    'imm r23, 26389                ; exact name_eq argument push'
  lookup_build_tooth lookup-wrong-primitive-end \
    'imm r23, 485' 'imm r23, 484'
  lookup_build_tooth lookup-wrong-call-census \
    'imm r22, 1                    ; name_eq call' \
    'imm r22, 0                    ; name_eq call'
  lookup_build_tooth lookup-wrong-store-census \
    'imm r24, 8                    ; prologue/local/temporary stores' \
    'imm r24, 7                    ; prologue/local/temporary stores'
  lookup_build_tooth lookup-drop-neqs-import \
    'imm r21, 1397839182           ; NEQS' \
    'imm r21, 1397839181           ; NEQS'
  lookup_build_tooth lookup-drop-snapshot \
    'store r1, r2                  ; immutable snapshot n0=NLOC, n0<=1024' \
    'store r1, r1                  ; immutable snapshot n0=NLOC, n0<=1024'
  lookup_build_tooth lookup-drop-prior-prefix \
    'store r1, r2                  ; exact forall j<i NEQS(j)=0, initially empty' \
    'store r1, r1                  ; exact forall j<i NEQS(j)=0, initially empty'
  lookup_build_tooth lookup-reread-live-bound \
    'store r1, r2                  ; n local written once and never reread NLOC' \
    'store r1, r1                  ; n local written once and never reread NLOC'
  lookup_build_tooth lookup-detach-prefix-snapshot \
    'store r1, r2                  ; NTDM prefix n is this same snapshot n0' \
    'store r1, r1                  ; NTDM prefix n is this same snapshot n0'
  lookup_build_tooth lookup-drop-current-argument \
    'store r1, r2                  ; local i -> arg313 -> call26420/cont26429' \
    'store r1, r1                  ; local i -> arg313 -> call26420/cont26429'
  lookup_build_tooth lookup-drop-current-selection \
    'store r1, r2                  ; selected index reinstantiated from current local i' \
    'store r1, r1                  ; selected index reinstantiated from current local i'
  lookup_build_tooth lookup-drop-neqs-total \
    'store r1, r2                  ; NEQS total zero/one and quiet/frame-restoring' \
    'store r1, r1                  ; NEQS total zero/one and quiet/frame-restoring'
  lookup_build_tooth lookup-drop-hit-zero-provenance \
    'store r1, r2                  ; provenance hit-slot-zero' \
    'store r1, r1                  ; provenance hit-slot-zero'
  lookup_build_tooth lookup-wrong-positive-result \
    'store r1, r2                  ; exact result equals positive i' \
    'store r1, r1                  ; exact result equals positive i'
  lookup_build_tooth lookup-drop-least-prefix \
    'imm r21, 1                    ; exact prior-false-prefix tag' \
    'imm r21, 2                    ; exact prior-false-prefix tag'
  lookup_build_tooth lookup-wrong-miss-result \
    'store r1, r2                  ; exact NEQS(i)=0 result' \
    'store r1, r1                  ; exact NEQS(i)=0 result'
  lookup_build_tooth lookup-zero-rank-step \
    'store r1, r2                  ; exact successor rank=rank-1' \
    'store r1, r1                  ; exact successor rank=rank-1'
  lookup_build_tooth lookup-drop-prefix-extension \
    'store r1, r2                  ; successor exact prior-index nonmatch prefix' \
    'store r1, r1                  ; successor exact prior-index nonmatch prefix'
  lookup_build_tooth lookup-drop-backedge-rename \
    'store r1, r2                  ; checked capture-avoiding rename' \
    'store r1, r1                  ; checked capture-avoiding rename'
  lookup_build_tooth lookup-drop-exhausted-no-call \
    'store r1, r2                  ; exact no-call exhausted path' \
    'store r1, r1                  ; exact no-call exhausted path'
  lookup_build_tooth lookup-drop-no-match-provenance \
    'store r1, r2                  ; provenance exhausted no-match' \
    'store r1, r1                  ; provenance exhausted no-match'
  lookup_build_tooth lookup-wrong-publication \
    'imm r2, 1263488844            ; LOOK' \
    'imm r2, 1263488843            ; LOOK'
}

lookup_reject_teeth() {
  for lookup_tooth_name in \
    lookup-wrong-continuation lookup-wrong-frame lookup-wrong-entry-pc \
    lookup-wrong-loop-transition lookup-wrong-call-event \
    lookup-wrong-call-target lookup-wrong-call-continuation \
    lookup-wrong-arity lookup-wrong-ambient lookup-wrong-epilogue \
    lookup-wrong-snapshot-local lookup-wrong-memory lookup-wrong-less-than \
    lookup-wrong-argument-push lookup-wrong-primitive-end \
    lookup-wrong-call-census lookup-wrong-store-census \
    lookup-drop-neqs-import lookup-drop-snapshot lookup-drop-prior-prefix \
    lookup-reread-live-bound lookup-detach-prefix-snapshot \
    lookup-drop-current-argument lookup-drop-current-selection \
    lookup-drop-neqs-total \
    lookup-drop-hit-zero-provenance lookup-wrong-positive-result \
    lookup-drop-least-prefix lookup-wrong-miss-result lookup-zero-rank-step \
    lookup-drop-prefix-extension lookup-drop-backedge-rename \
    lookup-drop-exhausted-no-call lookup-drop-no-match-provenance \
    lookup-wrong-publication
  do
    set +e
    "$T/$lookup_tooth_name" < "$T/control.bundle" > "$T/stdout"
    lookup_tooth_status=$?
    set -e
    if [ "$lookup_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $lookup_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
