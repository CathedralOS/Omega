#!/usr/bin/env sh
# Phase-isolated negative canaries for parse_proc's pre-output slot guard.
# Sourced by bc-block-control.sh after T/ASM/SEED are established.

parse_capacity_build_tooth() { # name sed-expression
  parse_capacity_tooth_name=$1
  parse_capacity_tooth_sed=$2
  sed "$parse_capacity_tooth_sed" "$T/control-check.alpha" \
    > "$T/$parse_capacity_tooth_name.alpha"
  "$ASM" < "$T/$parse_capacity_tooth_name.alpha" \
    > "$T/$parse_capacity_tooth_name.tape"
  stamp_seed "$T/$parse_capacity_tooth_name.tape" "$SEED" \
    "$T/$parse_capacity_tooth_name" >/dev/null
}

parse_capacity_build_teeth() {
  # Exact pdone addition, held local, comparison, transition, and status store.
  parse_capacity_build_tooth parse-capacity-wrong-count-ambient \
    's/imm r24, 1                  ; checked nparams ambient local/imm r24, 0                  ; checked nparams ambient local/'
  parse_capacity_build_tooth parse-capacity-wrong-add-opcode \
    's/imm r23, 3                   ; nparams + count_lets/imm r23, 4                   ; nparams + count_lets/'
  parse_capacity_build_tooth parse-capacity-wrong-limit \
    's/imm r23, 1024                ; checked total slot limit/imm r23, 1023                ; checked total slot limit/'
  parse_capacity_build_tooth parse-capacity-wrong-guard \
    's/imm r24, 50296              ; checked nslots<=1024 guard/imm r24, 50297              ; checked nslots<=1024 guard/'
  parse_capacity_build_tooth parse-capacity-wrong-status-site \
    's/imm r24, 50367              ; slot-capacity status 252/imm r24, 50368              ; slot-capacity status 252/'

  # Conditional composition and both exhaustive outcomes.
  parse_capacity_build_tooth parse-capacity-drop-close-clause \
    's/imm r21, 1                    ; PLOP close clause completed/imm r21, 0                    ; PLOP close clause completed/'
  parse_capacity_build_tooth parse-capacity-wrong-close-tag \
    's/imm r2, 1                    ; checked post-ADVE close relation tag/imm r2, 2                    ; checked post-ADVE close relation tag/'
  parse_capacity_build_tooth parse-capacity-wrong-expect-tag \
    's/imm r2, 1                    ; checked post-EXPS cursor relation tag/imm r2, 2                    ; checked post-EXPS cursor relation tag/'
  parse_capacity_build_tooth parse-capacity-drop-count-restore \
    's/imm r21, 1                    ; count_lets CUR restoration/imm r21, 0                    ; count_lets CUR restoration/'
  parse_capacity_build_tooth parse-capacity-wrong-nparams \
    's/store r1, r2                  ; nparams=NLOC in \[0,4\]/store r1, r1                  ; nparams=NLOC in [0,4]/'
  parse_capacity_build_tooth parse-capacity-wrong-count-bound \
    's/store r1, r2                  ; 0<=count<=LEN<=1048576/store r1, r1                  ; 0<=count<=LEN<=1048576/'
  parse_capacity_build_tooth parse-capacity-wrapping-sum \
    's/store r1, r2                  ; nslots=nparams+count, no wrap/store r1, r1                  ; nslots=nparams+count, no wrap/'
  parse_capacity_build_tooth parse-capacity-drop-prefix-status \
    's/imm r21, 1                    ; checked prefix status remains zero/imm r21, 0                    ; checked prefix status remains zero/'
  parse_capacity_build_tooth parse-capacity-reverse-room \
    's/imm r2, 1                    ; checked nslots<=1024 edge 281/imm r2, 2                    ; checked nslots<=1024 edge 281/'
  parse_capacity_build_tooth parse-capacity-reverse-overflow \
    's/imm r2, 2                    ; complementary nslots>1024/imm r2, 1                    ; complementary nslots>1024/'
  parse_capacity_build_tooth parse-capacity-wrong-overflow-status \
    's/store r1, r2                  ; numeric RESOURCE_FAIL=252/store r1, r1                  ; numeric RESOURCE_FAIL=252/'
  parse_capacity_build_tooth parse-capacity-wrong-failure-frame \
    's/store r1, r2                  ; return zero \/ restored caller frame/store r1, r1                  ; return zero \/ restored caller frame/'
}

parse_capacity_reject_teeth() {
  for parse_capacity_tooth_name in \
    parse-capacity-wrong-count-ambient \
    parse-capacity-wrong-add-opcode \
    parse-capacity-wrong-limit \
    parse-capacity-wrong-guard \
    parse-capacity-wrong-status-site \
    parse-capacity-drop-close-clause \
    parse-capacity-wrong-close-tag \
    parse-capacity-wrong-expect-tag \
    parse-capacity-drop-count-restore \
    parse-capacity-wrong-nparams \
    parse-capacity-wrong-count-bound \
    parse-capacity-wrapping-sum \
    parse-capacity-drop-prefix-status \
    parse-capacity-reverse-room \
    parse-capacity-reverse-overflow \
    parse-capacity-wrong-overflow-status \
    parse-capacity-wrong-failure-frame
  do
    set +e
    "$T/$parse_capacity_tooth_name" < "$T/control.bundle" > "$T/stdout"
    parse_capacity_tooth_status=$?
    set -e
    if [ "$parse_capacity_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $parse_capacity_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
