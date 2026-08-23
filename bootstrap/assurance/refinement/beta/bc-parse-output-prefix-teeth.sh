#!/usr/bin/env sh
# Phase-isolated canaries for the bounded parse_proc output-prefix theorem.

parse_output_prefix_build_tooth() { # name sed-expression
  parse_output_prefix_tooth_name=$1
  parse_output_prefix_tooth_sed=$2
  sed "$parse_output_prefix_tooth_sed" "$T/control-check.alpha" \
    > "$T/$parse_output_prefix_tooth_name.alpha"
  "$ASM" < "$T/$parse_output_prefix_tooth_name.alpha" \
    > "$T/$parse_output_prefix_tooth_name.tape"
  stamp_seed "$T/$parse_output_prefix_tooth_name.tape" "$SEED" \
    "$T/$parse_output_prefix_tooth_name" >/dev/null
}

parse_output_prefix_build_teeth() {
  parse_output_prefix_build_tooth parse-output-prefix-wrong-guard \
    's/imm r24, 50762              ; checked k<nparams guard/imm r24, 50763              ; checked k<nparams guard/'
  parse_output_prefix_build_tooth parse-output-prefix-wrong-exit \
    's/imm r24, 50781              ; checked loop exit/imm r24, 50782              ; checked loop exit/'
  parse_output_prefix_build_tooth parse-output-prefix-wrong-eids-continuation \
    's/imm r26, 50511              ; checked EIDS continuation/imm r26, 50512              ; checked EIDS continuation/'
  parse_output_prefix_build_tooth parse-output-prefix-wrong-epro-continuation \
    's/imm r26, 50611              ; checked EPRO continuation/imm r26, 50612              ; checked EPRO continuation/'
  parse_output_prefix_build_tooth parse-output-prefix-wrong-epar-continuation \
    's/imm r26, 50850              ; checked EPAR continuation/imm r26, 50851              ; checked EPAR continuation/'
  parse_output_prefix_build_tooth parse-output-prefix-wrong-name-push \
    's/imm r23, 50454/imm r23, 50455/'
  parse_output_prefix_build_tooth parse-output-prefix-event-undercount \
    's/imm r29, 600                 ; exclusive event row/imm r29, 599                 ; exclusive event row/'
  parse_output_prefix_build_tooth parse-output-prefix-primitive-undercount \
    's/imm r23, 805                 ; exclusive primitive row/imm r23, 804                 ; exclusive primitive row/'
  parse_output_prefix_build_tooth parse-output-prefix-store-undercount \
    's/imm r23, 8                   ; argument\/binary\/local stores/imm r23, 7                   ; argument\/binary\/local stores/'

  parse_output_prefix_build_tooth parse-output-prefix-omit-nparams-four \
    's/imm r1, 5                    ; checked nparams sweep includes 4/imm r1, 4                    ; checked nparams sweep includes 4/'
  parse_output_prefix_build_tooth parse-output-prefix-wrong-domain-count \
    's/imm r2, 15                   ; checked complete bounded loop domain/imm r2, 14                   ; checked complete bounded loop domain/'
  parse_output_prefix_build_tooth parse-output-prefix-drop-saved-name \
    's/store r1, r2                  ; slots0\/1 are the saved bounded name slice/store r1, r1                  ; slots0\/1 are the saved bounded name slice/'
  parse_output_prefix_build_tooth parse-output-prefix-wrong-colon-length \
    's/imm r2, 2                    ; checked literal is exactly ":\\n"/imm r2, 1                    ; checked literal is exactly ":\\n"/'
  parse_output_prefix_build_tooth parse-output-prefix-drop-epar-argument \
    's/imm r2, 1                    ; checked EPAR receives current k/imm r2, 0                    ; checked EPAR receives current k/'
  parse_output_prefix_build_tooth parse-output-prefix-drop-dead-result \
    's/store r1, r2                  ; EPAR result dead; row239 reloads k/store r1, r1                  ; EPAR result dead; row239 reloads k/'
  parse_output_prefix_build_tooth parse-output-prefix-drop-successor \
    's/store r1, r2                  ; checked successor and rank decrement/store r1, r1                  ; checked successor and rank decrement/'
  parse_output_prefix_build_tooth parse-output-prefix-wrong-exit-pc \
    's/imm r2, 50945                ; checked genbody entry PC/imm r2, 50946                ; checked genbody entry PC/'
  parse_output_prefix_build_tooth parse-output-prefix-drop-retained-frame \
    's/imm r2, 1                    ; active parse frame \/ quiet state retained/imm r2, 0                    ; active parse frame \/ quiet state retained/'
}

parse_output_prefix_reject_teeth() {
  for parse_output_prefix_tooth_name in \
    parse-output-prefix-wrong-guard \
    parse-output-prefix-wrong-exit \
    parse-output-prefix-wrong-eids-continuation \
    parse-output-prefix-wrong-epro-continuation \
    parse-output-prefix-wrong-epar-continuation \
    parse-output-prefix-wrong-name-push \
    parse-output-prefix-event-undercount \
    parse-output-prefix-primitive-undercount \
    parse-output-prefix-store-undercount \
    parse-output-prefix-omit-nparams-four \
    parse-output-prefix-wrong-domain-count \
    parse-output-prefix-drop-saved-name \
    parse-output-prefix-wrong-colon-length \
    parse-output-prefix-drop-epar-argument \
    parse-output-prefix-drop-dead-result \
    parse-output-prefix-drop-successor \
    parse-output-prefix-wrong-exit-pc \
    parse-output-prefix-drop-retained-frame
  do
    set +e
    "$T/$parse_output_prefix_tooth_name" < "$T/control.bundle" > "$T/stdout"
    parse_output_prefix_tooth_status=$?
    set -e
    if [ "$parse_output_prefix_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $parse_output_prefix_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
