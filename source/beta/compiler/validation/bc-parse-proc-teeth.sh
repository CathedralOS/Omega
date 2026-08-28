#!/usr/bin/env sh
# Phase-isolated seam canaries for the complete parse_proc implication.

parse_proc_build_tooth() { # name exact-old exact-new
  parse_proc_tooth_name=$1
  parse_proc_tooth_old=$2
  parse_proc_tooth_new=$3
  parse_proc_tooth_count=$(grep -F -c -- "$parse_proc_tooth_old" \
    "$T/parse-proc.alpha" || true)
  if [ "$parse_proc_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $parse_proc_tooth_name anchor count $parse_proc_tooth_count" >&2
    exit 1
  fi
  awk -v old="$parse_proc_tooth_old" -v new="$parse_proc_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/parse-proc.alpha" > "$T/$parse_proc_tooth_name.alpha"
  "$ASM" < "$T/$parse_proc_tooth_name.alpha" > \
    "$T/$parse_proc_tooth_name.tape"
  stamp_seed "$T/$parse_proc_tooth_name.tape" "$SEED" \
    "$T/$parse_proc_tooth_name" >/dev/null
}

parse_proc_build_teeth() {
  parse_proc_build_tooth parse-proc-wrong-entry-block \
    'imm r21, 48993                 ; exact p68 entry PC' \
    'imm r21, 48994                 ; exact p68 entry PC'
  parse_proc_build_tooth parse-proc-wrong-nloc-reset-store \
    'imm r24, 49077' \
    'imm r24, 49078'
  parse_proc_build_tooth parse-proc-reorder-first-ident \
    'imm r23, 49098                 ; first RIDS continuation' \
    'imm r23, 49099                 ; first RIDS continuation'
  parse_proc_build_tooth parse-proc-wrong-expect-arity \
    'imm r24, 1                     ; expect arity1' \
    'imm r24, 0                     ; expect arity1'
  parse_proc_build_tooth parse-proc-drop-saved-name \
    'call parse_proc_entry_store_cell ; saved slice copied to CURPROC' \
    'call reject ; saved slice copied to CURPROC'
  parse_proc_build_tooth parse-proc-invent-keyword-check \
    'imm r21, 1                     ; no keyword equality premise' \
    'imm r21, 2                     ; no keyword equality premise'
  parse_proc_build_tooth parse-proc-wrong-plop-schema \
    'imm r21, 1347178320             ; PLOP' \
    'imm r21, 1347178319             ; PLOP'
  parse_proc_build_tooth parse-proc-wrong-pcap-partition \
    'imm r21, 2                      ; early7 / room' \
    'imm r21, 1                      ; early7 / room'
  parse_proc_build_tooth parse-proc-float-pbod-depth \
    'imm r21, 0                      ; exact PBOD entry D' \
    'imm r21, 1                      ; exact PBOD entry D'
  parse_proc_build_tooth parse-proc-wrong-drem \
    'imm r2, 1296388676             ; local hypothetical DREM marker' \
    'imm r2, 1296388675             ; local hypothetical DREM marker'
  parse_proc_build_tooth parse-proc-admit-origin3 \
    'store r20, r2                  ; origin3 excluded by DREM' \
    'store r20, r12                 ; origin3 excluded by DREM'
  parse_proc_build_tooth parse-proc-wrong-outcome-count \
    'imm r21, 7                      ; complete root outcome rows' \
    'imm r21, 8                      ; complete root outcome rows'
  parse_proc_build_tooth parse-proc-truncate-div-trace \
    'imm r15, 2                    ; maximal PFXS||child, no epilogue' \
    'imm r15, 1                    ; maximal PFXS||child, no epilogue'
  parse_proc_build_tooth parse-proc-drop-finite-cursor-bound \
    'store r20, r18                 ; finite CUR<=LEN+2 category' \
    'store r20, r12                 ; finite CUR<=LEN+2 category'
  parse_proc_build_tooth parse-proc-drop-provenance \
    'store r20, r2                  ; provenance retained exactly' \
    'store r20, r12                 ; provenance retained exactly'
  parse_proc_build_tooth parse-proc-wrong-publication \
    'imm r2, 1129467984             ; PPRC' \
    'imm r2, 1129467983             ; PPRC'
}

parse_proc_reject_teeth() {
  for parse_proc_tooth_name in \
    parse-proc-wrong-entry-block \
    parse-proc-wrong-nloc-reset-store \
    parse-proc-reorder-first-ident \
    parse-proc-wrong-expect-arity \
    parse-proc-drop-saved-name \
    parse-proc-invent-keyword-check \
    parse-proc-wrong-plop-schema \
    parse-proc-wrong-pcap-partition \
    parse-proc-float-pbod-depth \
    parse-proc-wrong-drem \
    parse-proc-admit-origin3 \
    parse-proc-wrong-outcome-count \
    parse-proc-truncate-div-trace \
    parse-proc-drop-finite-cursor-bound \
    parse-proc-drop-provenance \
    parse-proc-wrong-publication
  do
    set +e
    "$T/$parse_proc_tooth_name" < "$T/control.bundle" > "$T/stdout"
    parse_proc_tooth_status=$?
    set -e
    if [ "$parse_proc_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $parse_proc_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
