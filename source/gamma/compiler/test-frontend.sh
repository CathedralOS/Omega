#!/usr/bin/env sh
# Adjacent gate for the canonical Gamma compiler's retained frontend and direct
# Alpha emitter/runtime substrate. No compiler artifact is published by this gate.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
T=$(mktemp -d); trap 'trash "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
runtime_emitter_source() {
  sed -n \
    '/^; ---- direct Alpha payload and fixup substrate/,/^; This is the eventual expression dispatcher/p' \
    gamma_compiler.beta
}
{
  sed -n '1,$p' gamma_compiler.beta
  printf '\nproc main() { return frontend_check_main() }\n'
} | "$T/bc.exe" > "$T/tc.tape" || {
  echo "bc(gamma_compiler.beta + frontend gate entry) failed"
  exit 1
}
stamp_seed "$T/tc.tape" "$SEED" "$T/tc.exe" >/dev/null 2>&1

{
  runtime_emitter_source
  printf '%s\n' \
    'proc main() {' \
    '    emit_reset()' \
    '    let start_label = new_label()' \
    '    let target_label = new_label()' \
    '    define_label(start_label)' \
    '    emit_imm(2, 72623859790382856)' \
    '    emit_jump(12, target_label)' \
    '    emit_rx(13, 3, start_label)' \
    '    define_label(target_label)' \
    '    emit_rr(3, 2, 3)' \
    '    emit_rrx(16, 2, 3, target_label)' \
    '    emit_r(0, 0)' \
    '    emit_ret()' \
    '    let payload_ok = validate_payload()' \
    '    state exact {' \
    '        to failed when (payload_ok != 1)' \
    '        to failed when (word[2097040] != 46)' \
    '        to failed when (word[2097024] != 3)' \
    '        to failed when (byte[33292288] != 1)' \
    '        to failed when (byte[33292289] != 2)' \
    '        to failed when (word[33292290] != 72623859790382856)' \
    '        to failed when (byte[33292298] != 12)' \
    '        to failed when (word[33292299] != 29)' \
    '        to failed when (byte[33292307] != 13)' \
    '        to failed when (byte[33292308] != 3)' \
	    '        to failed when (word[33292309] != 0)' \
	    '        to failed when (byte[33292317] != 3)' \
	    '        to failed when (byte[33292318] != 2)' \
	    '        to failed when (byte[33292319] != 3)' \
	    '        to failed when (byte[33292320] != 16)' \
	    '        to failed when (byte[33292321] != 2)' \
	    '        to failed when (byte[33292322] != 3)' \
	    '        to failed when (word[33292323] != 29)' \
	    '        to failed when (byte[33292331] != 0)' \
	    '        to failed when (byte[33292332] != 0)' \
	    '        to failed when (byte[33292333] != 20)' \
    '        to duplicate_setup' \
    '    }' \
    '    state duplicate_setup {' \
    '        emit_reset()' \
    '        let duplicate_label = new_label()' \
    '        define_label(duplicate_label)' \
    '        define_label(duplicate_label)' \
    '        let label_after_failure = new_label()' \
    '        to duplicate_check' \
    '    }' \
    '    state duplicate_check {' \
    '        to failed when (word[2097016] != 4)' \
    '        to failed when (label_after_failure != 0 - 1)' \
    '        to failed when (word[2097032] != 1)' \
    '        to missing_setup' \
    '    }' \
    '    state missing_setup {' \
    '        emit_reset()' \
    '        let missing_label = new_label()' \
    '        put_label_word(missing_label)' \
    '        let missing_valid = validate_payload()' \
    '        to missing_check' \
    '    }' \
    '    state missing_check {' \
    '        to failed when (missing_valid != 0)' \
    '        to failed when (word[2097016] != 8)' \
    '        to capacity_setup' \
    '    }' \
    '    state capacity_setup {' \
    '        emit_reset()' \
    '        word[2097040] = 262139' \
    '        let adjacent = put_byte(7)' \
    '        let overflow = put_byte(8)' \
    '        to capacity_check' \
    '    }' \
    '    state capacity_check {' \
    '        to failed when (adjacent != 1)' \
    '        to failed when (overflow != 0)' \
    '        to failed when (word[2097040] != 262140)' \
    '        to failed when (word[2097016] != 1)' \
    '        to fixup_capacity_setup' \
    '    }' \
    '    state fixup_capacity_setup {' \
    '        emit_reset()' \
    '        let fixup_label = new_label()' \
    '        put_u64(0)' \
    '        word[2097024] = 32768' \
    '        let fixup_result = add_fixup(0, fixup_label)' \
    '        to fixup_capacity_check' \
    '    }' \
    '    state fixup_capacity_check {' \
    '        to failed when (fixup_result != 0)' \
    '        to failed when (word[2097016] != 6)' \
    '        to frame_profile_setup' \
    '    }' \
    '    state frame_profile_setup {' \
    '        emit_reset()' \
    '        let frame_label = new_label()' \
    '        let invalid_frame_result = emit_gamma_call_frame(frame_label, frame_label, 8, 0)' \
    '        to frame_profile_check' \
    '    }' \
    '    state frame_profile_check {' \
    '        to failed when (invalid_frame_result != 0)' \
    '        to failed when (word[2097016] != 13)' \
    '        to failed when (word[2097040] != 0)' \
    '        to empty_setup' \
    '    }' \
    '    state empty_setup {' \
    '        emit_reset()' \
    '        let empty_valid = validate_payload()' \
    '        to empty_check' \
    '    }' \
    '    state empty_check {' \
    '        to failed when (empty_valid != 0)' \
    '        to failed when (word[2097016] != 12)' \
    '        return 1' \
    '    }' \
    '    state failed { return 0 }' \
    '}'
} | "$T/bc.exe" > "$T/emitter.tape" || {
  echo "bc(gamma_compiler.beta + emitter probe) failed"
  exit 1
}
stamp_seed "$T/emitter.tape" "$SEED" "$T/emitter.exe" >/dev/null 2>&1

{
  runtime_emitter_source
  printf '%s\n' \
    'proc main() {' \
    '    emit_reset()' \
    '    let entry_label = new_label()' \
    '    let heap_label = new_label()' \
    '    let stack_label = new_label()' \
    '    let failure_label = new_label()' \
    '    let unexpected_label = new_label()' \
    '    let heap_mode = new_label()' \
    '    let stack_mode = new_label()' \
    '    let negative_heap_mode = new_label()' \
    '    let negative_stack_mode = new_label()' \
    '    let overflow_heap_mode = new_label()' \
    '    let underflow_stack_mode = new_label()' \
    '    let heap_base_ok = new_label()' \
    '    let heap_first_ok = new_label()' \
    '    let heap_cap_ok = new_label()' \
    '    let stack_base_ok = new_label()' \
    '    let stack_first_ok = new_label()' \
    '    let stack_cap_ok = new_label()' \
    '    define_label(entry_label)' \
    '    emit_runtime_init()' \
    '    emit_r(17, 11)' \
    '    emit_imm(12, 104)' \
    '    emit_rrx(16, 11, 12, heap_mode)' \
    '    emit_imm(12, 115)' \
    '    emit_rrx(16, 11, 12, stack_mode)' \
    '    emit_imm(12, 72)' \
    '    emit_rrx(16, 11, 12, negative_heap_mode)' \
    '    emit_imm(12, 83)' \
    '    emit_rrx(16, 11, 12, negative_stack_mode)' \
    '    emit_imm(12, 111)' \
    '    emit_rrx(16, 11, 12, overflow_heap_mode)' \
    '    emit_imm(12, 117)' \
    '    emit_rrx(16, 11, 12, underflow_stack_mode)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(heap_mode)' \
    '    emit_imm(10, 0)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, heap_label)' \
    '    emit_imm(6, 16777248)' \
    '    emit_rrx(16, 0, 6, heap_base_ok)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(heap_base_ok)' \
    '    emit_imm(6, 16777264)' \
    '    emit_rrx(16, 254, 6, heap_first_ok)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(heap_first_ok)' \
    '    emit_imm(2, 33554384)' \
    '    emit_jump(19, heap_label)' \
    '    emit_imm(6, 50331648)' \
    '    emit_rrx(16, 254, 6, heap_cap_ok)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(heap_cap_ok)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 1)' \
    '    emit_jump(19, heap_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(stack_mode)' \
    '    emit_imm(10, 0)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_imm(6, 16777200)' \
    '    emit_rrx(16, 0, 6, stack_base_ok)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(stack_base_ok)' \
    '    emit_rrx(16, 252, 6, stack_first_ok)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(stack_first_ok)' \
    '    emit_imm(2, 16515056)' \
    '    emit_jump(19, stack_label)' \
    '    emit_imm(6, 262144)' \
    '    emit_rrx(16, 252, 6, stack_cap_ok)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(stack_cap_ok)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 1)' \
    '    emit_jump(19, stack_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(overflow_heap_mode)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775807)' \
    '    emit_jump(19, heap_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(underflow_stack_mode)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775807)' \
    '    emit_jump(19, stack_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(negative_heap_mode)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 0 - 1)' \
    '    emit_jump(19, heap_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(negative_stack_mode)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 0 - 1)' \
    '    emit_jump(19, stack_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(failure_label)' \
    '    emit_rx(13, 10, unexpected_label)' \
    '    emit_imm(0, 7)' \
    '    emit_r(0, 0)' \
    '    define_label(unexpected_label)' \
    '    emit_imm(0, 9)' \
    '    emit_r(0, 0)' \
    '    emit_heap_allocator(heap_label, failure_label)' \
    '    emit_stack_reserver(stack_label, failure_label)' \
    '    let payload_ok = validate_payload()' \
    '    state publish_setup {' \
    '        to failed when (payload_ok != 1)' \
    '        let i = 0' \
    '        to publish_loop' \
    '    }' \
    '    state publish_loop {' \
    '        to publish when (i < word[2097040])' \
    '        return 1' \
    '    }' \
    '    state publish {' \
    '        write_byte(byte[33292288 + i])' \
    '        i = i + 1' \
    '        to publish_loop' \
    '    }' \
    '    state failed { return 0 }' \
    '}'
} | "$T/bc.exe" > "$T/runtime-emitter.tape" || {
  echo "bc(gamma_compiler.beta + runtime containment probe) failed"
  exit 1
}
stamp_seed "$T/runtime-emitter.tape" "$SEED" "$T/runtime-emitter.exe" >/dev/null 2>&1
"$T/runtime-emitter.exe" > "$T/runtime-probe.tape"
runtime_emitter_status=$?
if [ "$runtime_emitter_status" != 1 ] || [ ! -s "$T/runtime-probe.tape" ]; then
  echo "gamma runtime probe emission failed: status $runtime_emitter_status" >&2
  exit 1
fi
stamp_seed "$T/runtime-probe.tape" "$SEED" "$T/runtime-probe.exe" >/dev/null 2>&1

{
  runtime_emitter_source
  printf '%s\n' \
    'proc emit_frame_eq(left, right, success_label, unexpected_label) {' \
    '    emit_rrx(16, left, right, success_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(success_label)' \
    '    return 0' \
    '}' \
    'proc main() {' \
    '    emit_reset()' \
    '    let entry_label = new_label()' \
    '    let stack_label = new_label()' \
    '    let loop_label = new_label()' \
    '    let loop_body = new_label()' \
    '    let loop_return = new_label()' \
    '    let loop_frame_ok = new_label()' \
    '    let large_label = new_label()' \
    '    let large_body = new_label()' \
    '    let large_return = new_label()' \
    '    let large_frame_ok = new_label()' \
    '    let large_marker_ok = new_label()' \
    '    let after_loop = new_label()' \
    '    let loop_result_ok = new_label()' \
    '    let loop_kind_ok = new_label()' \
    '    let spill_stack_ok = new_label()' \
    '    let spill_value_ok = new_label()' \
    '    let loop_stack_ok = new_label()' \
    '    let loop_base_ok = new_label()' \
    '    let wide_push_loop = new_label()' \
    '    let wide_push_body = new_label()' \
    '    let wide_push_done = new_label()' \
    '    let wide_label = new_label()' \
    '    let wide_frame_ok = new_label()' \
    '    let wide_first_kind_ok = new_label()' \
    '    let wide_first_ok = new_label()' \
    '    let wide_last_kind_ok = new_label()' \
    '    let wide_last_ok = new_label()' \
    '    let after_wide = new_label()' \
    '    let wide_kind_ok = new_label()' \
    '    let wide_result_ok = new_label()' \
    '    let final_stack_ok = new_label()' \
    '    let final_base_ok = new_label()' \
    '    let boundary_exact = new_label()' \
    '    let boundary_adjacent = new_label()' \
    '    let boundary_target = new_label()' \
    '    let boundary_stack_ok = new_label()' \
    '    let boundary_base_ok = new_label()' \
    '    let boundary_kind_ok = new_label()' \
    '    let boundary_payload_ok = new_label()' \
    '    let boundary_resource_expected = new_label()' \
    '    let boundary_header_base_ok = new_label()' \
    '    let boundary_header_cursor_ok = new_label()' \
    '    let resource_label = new_label()' \
    '    let unexpected_label = new_label()' \
    '    define_label(entry_label)' \
    '    emit_runtime_init()' \
    '    emit_imm(20, 55)' \
    '    emit_imm(2, 8)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 20)' \
    '    emit_imm(20, 4096)' \
    '    emit_imm(23, 0)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 22, 0)' \
    '    emit_imm(24, 8)' \
    '    emit_rr(3, 22, 24)' \
    '    emit_rr(11, 22, 20)' \
    '    emit_imm(20, 0)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 22, 0)' \
    '    emit_imm(24, 8)' \
    '    emit_rr(3, 22, 24)' \
    '    emit_rr(11, 22, 20)' \
    '    emit_gamma_call_frame(stack_label, loop_label, 16, 2)' \
    '    emit_jump(12, after_loop)' \
    '    define_label(loop_label)' \
    '    emit_frame_eq(252, 253, loop_frame_ok, unexpected_label)' \
    '    emit_rr(2, 20, 253)' \
    '    emit_imm(22, 40)' \
    '    emit_rr(3, 20, 22)' \
    '    emit_rr(10, 20, 20)' \
    '    emit_rr(2, 21, 253)' \
    '    emit_imm(22, 24)' \
    '    emit_rr(3, 21, 22)' \
    '    emit_rr(10, 21, 21)' \
    '    emit_rx(13, 20, loop_return)' \
    '    define_label(loop_body)' \
    '    emit_imm(22, 1)' \
    '    emit_rr(4, 20, 22)' \
    '    emit_rr(3, 21, 22)' \
    '    emit_imm(23, 0)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 24, 0)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(11, 24, 20)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 24, 0)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(11, 24, 21)' \
    '    emit_imm(20, 99)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 24, 0)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(11, 24, 20)' \
    '    emit_gamma_tail_frame(32, 3, large_label, resource_label)' \
    '    define_label(loop_return)' \
    '    emit_imm(0, 0)' \
    '    emit_rr(2, 1, 21)' \
    '    emit_gamma_return_frame()' \
    '    define_label(large_label)' \
    '    emit_frame_eq(252, 253, large_frame_ok, unexpected_label)' \
    '    emit_rr(2, 20, 253)' \
    '    emit_imm(22, 72)' \
    '    emit_rr(3, 20, 22)' \
    '    emit_rr(10, 20, 20)' \
    '    emit_rr(2, 21, 253)' \
    '    emit_imm(22, 56)' \
    '    emit_rr(3, 21, 22)' \
    '    emit_rr(10, 21, 21)' \
    '    emit_rr(2, 24, 253)' \
    '    emit_imm(22, 40)' \
    '    emit_rr(3, 24, 22)' \
    '    emit_rr(10, 24, 24)' \
    '    emit_imm(22, 99)' \
    '    emit_frame_eq(24, 22, large_marker_ok, unexpected_label)' \
    '    emit_rx(13, 20, large_return)' \
    '    define_label(large_body)' \
    '    emit_imm(22, 1)' \
    '    emit_rr(4, 20, 22)' \
    '    emit_rr(3, 21, 22)' \
    '    emit_imm(23, 0)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 24, 0)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(11, 24, 20)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 24, 0)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(11, 24, 21)' \
    '    emit_gamma_tail_frame(16, 2, loop_label, resource_label)' \
    '    define_label(large_return)' \
    '    emit_imm(0, 0)' \
    '    emit_rr(2, 1, 21)' \
    '    emit_gamma_return_frame()' \
    '    define_label(after_loop)' \
    '    emit_imm(22, 4096)' \
    '    emit_frame_eq(1, 22, loop_result_ok, unexpected_label)' \
    '    emit_imm(22, 0)' \
    '    emit_frame_eq(0, 22, loop_kind_ok, unexpected_label)' \
    '    emit_imm(22, 16777208)' \
    '    emit_frame_eq(252, 22, spill_stack_ok, unexpected_label)' \
    '    emit_rr(2, 20, 252)' \
    '    emit_rr(10, 20, 20)' \
    '    emit_imm(22, 55)' \
    '    emit_frame_eq(20, 22, spill_value_ok, unexpected_label)' \
    '    emit_imm(22, 8)' \
    '    emit_rr(3, 252, 22)' \
    '    emit_imm(22, 16777216)' \
    '    emit_frame_eq(252, 22, loop_stack_ok, unexpected_label)' \
    '    emit_frame_eq(253, 22, loop_base_ok, unexpected_label)' \
    '    emit_imm(20, 0)' \
    '    emit_imm(21, 600)' \
    '    emit_imm(23, 37)' \
    '    define_label(wide_push_loop)' \
    '    emit_rrx(15, 20, 21, wide_push_body)' \
    '    emit_jump(12, wide_push_done)' \
    '    define_label(wide_push_body)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 24, 0)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(11, 24, 20)' \
    '    emit_imm(22, 1)' \
    '    emit_rr(3, 20, 22)' \
    '    emit_jump(12, wide_push_loop)' \
    '    define_label(wide_push_done)' \
    '    emit_gamma_call_frame(stack_label, wide_label, 48, 600)' \
    '    emit_jump(12, after_wide)' \
    '    define_label(wide_label)' \
    '    emit_frame_eq(252, 253, wide_frame_ok, unexpected_label)' \
    '    emit_rr(2, 20, 253)' \
    '    emit_imm(22, 9632)' \
    '    emit_rr(3, 20, 22)' \
    '    emit_rr(10, 20, 20)' \
    '    emit_imm(22, 37)' \
    '    emit_frame_eq(20, 22, wide_first_kind_ok, unexpected_label)' \
    '    emit_rr(2, 20, 253)' \
    '    emit_imm(22, 9640)' \
    '    emit_rr(3, 20, 22)' \
    '    emit_rr(10, 20, 20)' \
    '    emit_imm(22, 0)' \
    '    emit_frame_eq(20, 22, wide_first_ok, unexpected_label)' \
    '    emit_rr(2, 20, 253)' \
    '    emit_imm(22, 48)' \
    '    emit_rr(3, 20, 22)' \
    '    emit_rr(10, 20, 20)' \
    '    emit_imm(22, 37)' \
    '    emit_frame_eq(20, 22, wide_last_kind_ok, unexpected_label)' \
    '    emit_rr(2, 20, 253)' \
    '    emit_imm(22, 56)' \
    '    emit_rr(3, 20, 22)' \
    '    emit_rr(10, 20, 20)' \
    '    emit_imm(22, 599)' \
    '    emit_frame_eq(20, 22, wide_last_ok, unexpected_label)' \
    '    emit_imm(0, 1)' \
    '    emit_imm(1, 77)' \
    '    emit_gamma_return_frame()' \
    '    define_label(after_wide)' \
    '    emit_imm(22, 1)' \
    '    emit_frame_eq(0, 22, wide_kind_ok, unexpected_label)' \
    '    emit_imm(22, 77)' \
    '    emit_frame_eq(1, 22, wide_result_ok, unexpected_label)' \
    '    emit_imm(22, 16777216)' \
    '    emit_frame_eq(252, 22, final_stack_ok, unexpected_label)' \
    '    emit_frame_eq(253, 22, final_base_ok, unexpected_label)' \
    '    emit_imm(6, 16777216)' \
    '    emit_imm(7, 16777216)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 6)' \
    '    emit_rr(2, 9, 0)' \
    '    emit_imm(10, 8)' \
    '    emit_rr(3, 9, 10)' \
    '    emit_rr(11, 9, 7)' \
    '    emit_rr(2, 253, 0)' \
    '    emit_imm(23, 37)' \
    '    emit_imm(20, 88)' \
    '    emit_imm(2, 16)' \
    '    emit_jump(19, stack_label)' \
    '    emit_rr(11, 0, 23)' \
    '    emit_rr(2, 24, 0)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(11, 24, 20)' \
    '    emit_imm(30, 0)' \
    '    emit_r(17, 11)' \
    '    emit_imm(12, 101)' \
    '    emit_rrx(16, 11, 12, boundary_exact)' \
    '    emit_imm(12, 102)' \
    '    emit_rrx(16, 11, 12, boundary_adjacent)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(boundary_exact)' \
    '    emit_gamma_tail_frame(16515056, 1, boundary_target, resource_label)' \
    '    define_label(boundary_adjacent)' \
    '    emit_imm(30, 1)' \
    '    emit_gamma_tail_frame(16515072, 1, boundary_target, resource_label)' \
    '    define_label(boundary_target)' \
    '    emit_rx(14, 30, unexpected_label)' \
    '    emit_imm(22, 262144)' \
    '    emit_frame_eq(252, 22, boundary_stack_ok, unexpected_label)' \
    '    emit_frame_eq(253, 22, boundary_base_ok, unexpected_label)' \
    '    emit_imm(24, 16777200)' \
    '    emit_rr(10, 20, 24)' \
    '    emit_imm(22, 37)' \
    '    emit_frame_eq(20, 22, boundary_kind_ok, unexpected_label)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(10, 20, 24)' \
    '    emit_imm(22, 88)' \
    '    emit_frame_eq(20, 22, boundary_payload_ok, unexpected_label)' \
    '    emit_imm(0, 7)' \
    '    emit_r(0, 0)' \
    '    define_label(resource_label)' \
    '    emit_rx(14, 30, boundary_resource_expected)' \
    '    emit_imm(0, 6)' \
    '    emit_r(0, 0)' \
    '    define_label(boundary_resource_expected)' \
    '    emit_imm(24, 16777200)' \
    '    emit_rr(10, 20, 24)' \
    '    emit_imm(22, 16777216)' \
    '    emit_frame_eq(20, 22, boundary_header_base_ok, unexpected_label)' \
    '    emit_imm(25, 8)' \
    '    emit_rr(3, 24, 25)' \
    '    emit_rr(10, 20, 24)' \
    '    emit_frame_eq(20, 22, boundary_header_cursor_ok, unexpected_label)' \
    '    emit_imm(0, 7)' \
    '    emit_r(0, 0)' \
    '    define_label(unexpected_label)' \
    '    emit_imm(0, 9)' \
    '    emit_r(0, 0)' \
    '    emit_stack_reserver(stack_label, resource_label)' \
    '    let payload_ok = validate_payload()' \
    '    state publish_setup {' \
    '        to failed when (payload_ok != 1)' \
    '        let i = 0' \
    '        to publish_loop' \
    '    }' \
    '    state publish_loop {' \
    '        to publish when (i < word[2097040])' \
    '        return 1' \
    '    }' \
    '    state publish {' \
    '        write_byte(byte[33292288 + i])' \
    '        i = i + 1' \
    '        to publish_loop' \
    '    }' \
    '    state failed { return 0 }' \
    '}'
} | "$T/bc.exe" > "$T/frame-emitter.tape" || {
  echo "bc(gamma_compiler.beta + Gamma frame ABI probe) failed"
  exit 1
}
stamp_seed "$T/frame-emitter.tape" "$SEED" "$T/frame-emitter.exe" >/dev/null 2>&1
"$T/frame-emitter.exe" > "$T/frame-probe.tape"
frame_emitter_status=$?
if [ "$frame_emitter_status" != 1 ] || [ ! -s "$T/frame-probe.tape" ]; then
  echo "Gamma frame ABI probe emission failed: status $frame_emitter_status" >&2
  exit 1
fi
stamp_seed "$T/frame-probe.tape" "$SEED" "$T/frame-probe.exe" >/dev/null 2>&1

{
  runtime_emitter_source
  printf '%s\n' \
    'proc emit_probe_eq(left, right, success_label, unexpected_label) {' \
    '    emit_rrx(16, left, right, success_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(success_label)' \
    '    return 0' \
    '}' \
    'proc main() {' \
    '    emit_reset()' \
    '    let entry_label = new_label()' \
    '    let heap_label = new_label()' \
    '    let add_label = new_label()' \
    '    let single_label = new_label()' \
    '    let length_label = new_label()' \
    '    let concat_label = new_label()' \
    '    let slice_label = new_label()' \
    '    let get_label = new_label()' \
    '    let resource_label = new_label()' \
    '    let trap_label = new_label()' \
    '    let internal_label = new_label()' \
    '    let unexpected_label = new_label()' \
    '    let empty_heap_ok = new_label()' \
    '    let empty_zero_ok = new_label()' \
    '    let empty_one_ok = new_label()' \
    '    let empty_two_ok = new_label()' \
    '    let empty_three_ok = new_label()' \
    '    let first_base_ok = new_label()' \
    '    let first_heap_ok = new_label()' \
    '    let first_length_ok = new_label()' \
    '    let first_byte_ok = new_label()' \
    '    let pair_length_ok = new_label()' \
    '    let pair_first_ok = new_label()' \
    '    let pair_second_ok = new_label()' \
    '    let four_length_ok = new_label()' \
    '    let cross_length_ok = new_label()' \
    '    let cross_first_ok = new_label()' \
    '    let cross_second_ok = new_label()' \
    '    let nested_byte_ok = new_label()' \
    '    let empty_concat_ok = new_label()' \
    '    let full_slice_ok = new_label()' \
    '    let zero_slice_ok = new_label()' \
    '    let deep_loop = new_label()' \
    '    let deep_body = new_label()' \
    '    let deep_done = new_label()' \
    '    let deep_byte_ok = new_label()' \
    '    define_label(entry_label)' \
    '    emit_runtime_init()' \
    '    emit_imm(30, 16777248)' \
    '    emit_probe_eq(254, 30, empty_heap_ok, unexpected_label)' \
    '    emit_imm(30, 0)' \
    '    emit_imm(31, 16777216)' \
    '    emit_rr(10, 29, 31)' \
    '    emit_probe_eq(29, 30, empty_zero_ok, unexpected_label)' \
    '    emit_imm(28, 8)' \
    '    emit_rr(3, 31, 28)' \
    '    emit_rr(10, 29, 31)' \
    '    emit_probe_eq(29, 30, empty_one_ok, unexpected_label)' \
    '    emit_rr(3, 31, 28)' \
    '    emit_rr(10, 29, 31)' \
    '    emit_probe_eq(29, 30, empty_two_ok, unexpected_label)' \
    '    emit_rr(3, 31, 28)' \
    '    emit_rr(10, 29, 31)' \
    '    emit_probe_eq(29, 30, empty_three_ok, unexpected_label)' \
    '    emit_imm(2, 0)' \
    '    emit_jump(19, single_label)' \
    '    emit_rr(2, 20, 0)' \
    '    emit_imm(30, 16777248)' \
    '    emit_probe_eq(20, 30, first_base_ok, unexpected_label)' \
    '    emit_imm(30, 16777280)' \
    '    emit_probe_eq(254, 30, first_heap_ok, unexpected_label)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_jump(19, length_label)' \
    '    emit_rr(2, 28, 0)' \
    '    emit_imm(29, 1)' \
    '    emit_probe_eq(28, 29, first_length_ok, unexpected_label)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_imm(3, 0)' \
    '    emit_jump(19, get_label)' \
    '    emit_imm(29, 0)' \
    '    emit_probe_eq(0, 29, first_byte_ok, unexpected_label)' \
    '    emit_imm(2, 255)' \
    '    emit_jump(19, single_label)' \
    '    emit_rr(2, 21, 0)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_rr(2, 3, 21)' \
    '    emit_jump(19, concat_label)' \
    '    emit_rr(2, 22, 0)' \
    '    emit_rr(2, 2, 22)' \
    '    emit_jump(19, length_label)' \
    '    emit_rr(2, 28, 0)' \
    '    emit_imm(29, 2)' \
    '    emit_probe_eq(28, 29, pair_length_ok, unexpected_label)' \
    '    emit_rr(2, 2, 22)' \
    '    emit_imm(3, 0)' \
    '    emit_jump(19, get_label)' \
    '    emit_imm(29, 0)' \
    '    emit_probe_eq(0, 29, pair_first_ok, unexpected_label)' \
    '    emit_rr(2, 2, 22)' \
    '    emit_imm(3, 1)' \
    '    emit_jump(19, get_label)' \
    '    emit_imm(29, 255)' \
    '    emit_probe_eq(0, 29, pair_second_ok, unexpected_label)' \
    '    emit_rr(2, 2, 22)' \
    '    emit_rr(2, 3, 22)' \
    '    emit_jump(19, concat_label)' \
    '    emit_rr(2, 23, 0)' \
    '    emit_rr(2, 2, 23)' \
    '    emit_jump(19, length_label)' \
    '    emit_rr(2, 28, 0)' \
    '    emit_imm(29, 4)' \
    '    emit_probe_eq(28, 29, four_length_ok, unexpected_label)' \
    '    emit_rr(2, 2, 23)' \
    '    emit_imm(3, 1)' \
    '    emit_imm(4, 2)' \
    '    emit_jump(19, slice_label)' \
    '    emit_rr(2, 24, 0)' \
    '    emit_rr(2, 2, 24)' \
    '    emit_jump(19, length_label)' \
    '    emit_rr(2, 28, 0)' \
    '    emit_imm(29, 2)' \
    '    emit_probe_eq(28, 29, cross_length_ok, unexpected_label)' \
    '    emit_rr(2, 2, 24)' \
    '    emit_imm(3, 0)' \
    '    emit_jump(19, get_label)' \
    '    emit_imm(29, 255)' \
    '    emit_probe_eq(0, 29, cross_first_ok, unexpected_label)' \
    '    emit_rr(2, 2, 24)' \
    '    emit_imm(3, 1)' \
    '    emit_jump(19, get_label)' \
    '    emit_imm(29, 0)' \
    '    emit_probe_eq(0, 29, cross_second_ok, unexpected_label)' \
    '    emit_rr(2, 2, 24)' \
    '    emit_imm(3, 1)' \
    '    emit_imm(4, 1)' \
    '    emit_jump(19, slice_label)' \
    '    emit_rr(2, 25, 0)' \
    '    emit_rr(2, 2, 25)' \
    '    emit_imm(3, 0)' \
    '    emit_jump(19, get_label)' \
    '    emit_imm(29, 0)' \
    '    emit_probe_eq(0, 29, nested_byte_ok, unexpected_label)' \
    '    emit_imm(2, 16777216)' \
    '    emit_rr(2, 3, 25)' \
    '    emit_jump(19, concat_label)' \
    '    emit_probe_eq(0, 25, empty_concat_ok, unexpected_label)' \
    '    emit_rr(2, 2, 22)' \
    '    emit_imm(3, 0)' \
    '    emit_imm(4, 2)' \
    '    emit_jump(19, slice_label)' \
    '    emit_probe_eq(0, 22, full_slice_ok, unexpected_label)' \
    '    emit_rr(2, 2, 22)' \
    '    emit_imm(3, 2)' \
    '    emit_imm(4, 0)' \
    '    emit_jump(19, slice_label)' \
    '    emit_imm(29, 16777216)' \
    '    emit_probe_eq(0, 29, zero_slice_ok, unexpected_label)' \
    '    emit_rr(2, 26, 20)' \
    '    emit_imm(27, 0)' \
    '    emit_imm(28, 1024)' \
    '    define_label(deep_loop)' \
    '    emit_rrx(15, 27, 28, deep_body)' \
    '    emit_jump(12, deep_done)' \
    '    define_label(deep_body)' \
    '    emit_rr(2, 2, 26)' \
    '    emit_rr(2, 3, 21)' \
    '    emit_jump(19, concat_label)' \
    '    emit_rr(2, 26, 0)' \
    '    emit_imm(29, 1)' \
    '    emit_rr(3, 27, 29)' \
    '    emit_jump(12, deep_loop)' \
    '    define_label(deep_done)' \
    '    emit_rr(2, 2, 26)' \
    '    emit_imm(3, 0)' \
    '    emit_jump(19, get_label)' \
    '    emit_imm(29, 0)' \
    '    emit_probe_eq(0, 29, deep_byte_ok, unexpected_label)' \
    '    emit_imm(0, 7)' \
    '    emit_r(0, 0)' \
    '    define_label(resource_label)' \
    '    emit_imm(0, 6)' \
    '    emit_r(0, 0)' \
    '    define_label(trap_label)' \
    '    emit_imm(0, 5)' \
    '    emit_r(0, 0)' \
    '    define_label(internal_label)' \
    '    emit_imm(0, 4)' \
    '    emit_r(0, 0)' \
    '    define_label(unexpected_label)' \
    '    emit_imm(0, 9)' \
    '    emit_r(0, 0)' \
    '    emit_heap_allocator(heap_label, resource_label)' \
    '    emit_checked_add(add_label, trap_label)' \
    '    emit_bytes_single(single_label, heap_label, trap_label)' \
    '    emit_bytes_length(length_label, internal_label)' \
    '    emit_bytes_concat(concat_label, heap_label, add_label, internal_label)' \
    '    emit_bytes_slice(slice_label, heap_label, trap_label, internal_label)' \
    '    emit_bytes_get(get_label, trap_label, internal_label)' \
    '    let payload_ok = validate_payload()' \
    '    state publish_setup {' \
    '        to failed when (payload_ok != 1)' \
    '        let i = 0' \
    '        to publish_loop' \
    '    }' \
    '    state publish_loop {' \
    '        to publish when (i < word[2097040])' \
    '        return 1' \
    '    }' \
    '    state publish {' \
    '        write_byte(byte[33292288 + i])' \
    '        i = i + 1' \
    '        to publish_loop' \
    '    }' \
    '    state failed { return 0 }' \
    '}'
} | "$T/bc.exe" > "$T/bytes-valid-emitter.tape" || {
  echo "bc(gamma_compiler.beta + Bytes valid runtime probe) failed"
  exit 1
}
stamp_seed "$T/bytes-valid-emitter.tape" "$SEED" "$T/bytes-valid-emitter.exe" >/dev/null 2>&1
"$T/bytes-valid-emitter.exe" > "$T/bytes-valid-probe.tape"
bytes_valid_emitter_status=$?
if [ "$bytes_valid_emitter_status" != 1 ] || [ ! -s "$T/bytes-valid-probe.tape" ]; then
  echo "Gamma Bytes valid probe emission failed: status $bytes_valid_emitter_status" >&2
  exit 1
fi
stamp_seed "$T/bytes-valid-probe.tape" "$SEED" "$T/bytes-valid-probe.exe" >/dev/null 2>&1

{
  runtime_emitter_source
  printf '%s\n' \
    'proc main() {' \
    '    emit_reset()' \
    '    let entry_label = new_label()' \
    '    let heap_label = new_label()' \
    '    let add_label = new_label()' \
    '    let single_label = new_label()' \
    '    let length_label = new_label()' \
    '    let concat_label = new_label()' \
    '    let slice_label = new_label()' \
    '    let get_label = new_label()' \
    '    let resource_label = new_label()' \
    '    let trap_label = new_label()' \
    '    let internal_label = new_label()' \
    '    let success_label = new_label()' \
    '    let unexpected_label = new_label()' \
    '    let dispatch_label = new_label()' \
    '    let single_negative = new_label()' \
    '    let single_large = new_label()' \
    '    let get_negative = new_label()' \
    '    let get_large = new_label()' \
    '    let slice_start_negative = new_label()' \
    '    let slice_length_negative = new_label()' \
    '    let slice_start_large = new_label()' \
    '    let slice_range_large = new_label()' \
    '    let misaligned_descriptor = new_label()' \
    '    let unallocated_descriptor = new_label()' \
    '    let unknown_kind = new_label()' \
    '    let malformed_child = new_label()' \
    '    let exact_resource = new_label()' \
    '    let exact_base_ok = new_label()' \
    '    let exact_cap_ok = new_label()' \
    '    define_label(entry_label)' \
    '    emit_runtime_init()' \
    '    emit_r(17, 11)' \
    '    emit_imm(2, 42)' \
    '    emit_jump(19, single_label)' \
    '    emit_rr(2, 20, 0)' \
    '    define_label(dispatch_label)' \
    '    emit_imm(12, 97)' \
    '    emit_rrx(16, 11, 12, single_negative)' \
    '    emit_imm(12, 98)' \
    '    emit_rrx(16, 11, 12, single_large)' \
    '    emit_imm(12, 99)' \
    '    emit_rrx(16, 11, 12, get_negative)' \
    '    emit_imm(12, 100)' \
    '    emit_rrx(16, 11, 12, get_large)' \
    '    emit_imm(12, 101)' \
    '    emit_rrx(16, 11, 12, slice_start_negative)' \
    '    emit_imm(12, 102)' \
    '    emit_rrx(16, 11, 12, slice_length_negative)' \
    '    emit_imm(12, 103)' \
    '    emit_rrx(16, 11, 12, slice_start_large)' \
    '    emit_imm(12, 104)' \
    '    emit_rrx(16, 11, 12, slice_range_large)' \
    '    emit_imm(12, 105)' \
    '    emit_rrx(16, 11, 12, misaligned_descriptor)' \
    '    emit_imm(12, 106)' \
    '    emit_rrx(16, 11, 12, unallocated_descriptor)' \
    '    emit_imm(12, 107)' \
    '    emit_rrx(16, 11, 12, unknown_kind)' \
    '    emit_imm(12, 108)' \
    '    emit_rrx(16, 11, 12, malformed_child)' \
    '    emit_imm(12, 109)' \
    '    emit_rrx(16, 11, 12, exact_resource)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(single_negative)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 0 - 1)' \
    '    emit_jump(19, single_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(single_large)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 256)' \
    '    emit_jump(19, single_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(get_negative)' \
    '    emit_imm(10, 1)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_imm(3, 0 - 1)' \
    '    emit_jump(19, get_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(get_large)' \
    '    emit_imm(10, 1)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_imm(3, 1)' \
    '    emit_jump(19, get_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(slice_start_negative)' \
    '    emit_imm(10, 1)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_imm(3, 0 - 1)' \
    '    emit_imm(4, 0)' \
    '    emit_jump(19, slice_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(slice_length_negative)' \
    '    emit_imm(10, 1)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_imm(3, 0)' \
    '    emit_imm(4, 0 - 1)' \
    '    emit_jump(19, slice_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(slice_start_large)' \
    '    emit_imm(10, 1)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_imm(3, 2)' \
    '    emit_imm(4, 0)' \
    '    emit_jump(19, slice_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(slice_range_large)' \
    '    emit_imm(10, 1)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_imm(3, 1)' \
    '    emit_imm(4, 1)' \
    '    emit_jump(19, slice_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(misaligned_descriptor)' \
    '    emit_imm(10, 2)' \
    '    emit_imm(2, 16777217)' \
    '    emit_jump(19, length_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(unallocated_descriptor)' \
    '    emit_imm(10, 2)' \
    '    emit_rr(2, 2, 254)' \
    '    emit_jump(19, length_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(unknown_kind)' \
    '    emit_imm(2, 32)' \
    '    emit_jump(19, heap_label)' \
    '    emit_rr(2, 21, 0)' \
    '    emit_imm(12, 99)' \
    '    emit_rr(11, 21, 12)' \
    '    emit_rr(2, 13, 21)' \
    '    emit_imm(12, 24)' \
    '    emit_rr(3, 13, 12)' \
    '    emit_imm(12, 1)' \
    '    emit_rr(11, 13, 12)' \
    '    emit_imm(10, 2)' \
    '    emit_rr(2, 2, 21)' \
    '    emit_imm(3, 0)' \
    '    emit_jump(19, get_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(malformed_child)' \
    '    emit_rr(2, 2, 20)' \
    '    emit_rr(2, 3, 20)' \
    '    emit_jump(19, concat_label)' \
    '    emit_rr(2, 21, 0)' \
    '    emit_rr(2, 13, 21)' \
    '    emit_imm(12, 16)' \
    '    emit_rr(3, 13, 12)' \
    '    emit_imm(12, 17)' \
    '    emit_rr(11, 13, 12)' \
    '    emit_imm(10, 2)' \
    '    emit_rr(2, 2, 21)' \
    '    emit_imm(3, 1)' \
    '    emit_jump(19, get_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(exact_resource)' \
    '    emit_imm(254, 50331616)' \
    '    emit_imm(2, 7)' \
    '    emit_jump(19, single_label)' \
    '    emit_imm(12, 50331616)' \
    '    emit_rrx(16, 0, 12, exact_base_ok)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(exact_base_ok)' \
    '    emit_rrx(16, 254, 255, exact_cap_ok)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(exact_cap_ok)' \
    '    emit_imm(10, 3)' \
    '    emit_imm(2, 8)' \
    '    emit_jump(19, single_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(resource_label)' \
    '    emit_imm(12, 3)' \
    '    emit_rrx(16, 10, 12, success_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(trap_label)' \
    '    emit_imm(12, 1)' \
    '    emit_rrx(16, 10, 12, success_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(internal_label)' \
    '    emit_imm(12, 2)' \
    '    emit_rrx(16, 10, 12, success_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(success_label)' \
    '    emit_imm(0, 7)' \
    '    emit_r(0, 0)' \
    '    define_label(unexpected_label)' \
    '    emit_imm(0, 9)' \
    '    emit_r(0, 0)' \
    '    emit_heap_allocator(heap_label, resource_label)' \
    '    emit_checked_add(add_label, trap_label)' \
    '    emit_bytes_single(single_label, heap_label, trap_label)' \
    '    emit_bytes_length(length_label, internal_label)' \
    '    emit_bytes_concat(concat_label, heap_label, add_label, internal_label)' \
    '    emit_bytes_slice(slice_label, heap_label, trap_label, internal_label)' \
    '    emit_bytes_get(get_label, trap_label, internal_label)' \
    '    let payload_ok = validate_payload()' \
    '    state publish_setup {' \
    '        to failed when (payload_ok != 1)' \
    '        let i = 0' \
    '        to publish_loop' \
    '    }' \
    '    state publish_loop {' \
    '        to publish when (i < word[2097040])' \
    '        return 1' \
    '    }' \
    '    state publish {' \
    '        write_byte(byte[33292288 + i])' \
    '        i = i + 1' \
    '        to publish_loop' \
    '    }' \
    '    state failed { return 0 }' \
    '}'
} | "$T/bc.exe" > "$T/bytes-invalid-emitter.tape" || {
  echo "bc(gamma_compiler.beta + Bytes failure-class probe) failed"
  exit 1
}
stamp_seed "$T/bytes-invalid-emitter.tape" "$SEED" "$T/bytes-invalid-emitter.exe" >/dev/null 2>&1
"$T/bytes-invalid-emitter.exe" > "$T/bytes-invalid-probe.tape"
bytes_invalid_emitter_status=$?
if [ "$bytes_invalid_emitter_status" != 1 ] || [ ! -s "$T/bytes-invalid-probe.tape" ]; then
  echo "Gamma Bytes failure-class probe emission failed: status $bytes_invalid_emitter_status" >&2
  exit 1
fi
stamp_seed "$T/bytes-invalid-probe.tape" "$SEED" "$T/bytes-invalid-probe.exe" >/dev/null 2>&1

{
  runtime_emitter_source
  printf '%s\n' \
    'proc main() {' \
    '    emit_reset()' \
    '    let entry_label = new_label()' \
    '    let add_label = new_label()' \
    '    let sub_label = new_label()' \
    '    let mul_label = new_label()' \
    '    let div_label = new_label()' \
    '    let mod_label = new_label()' \
    '    let failure_label = new_label()' \
    '    let check_label = new_label()' \
    '    let accepted_label = new_label()' \
    '    let unexpected_label = new_label()' \
    '    let add_ok = new_label()' \
    '    let add_positive_overflow = new_label()' \
    '    let add_negative_overflow = new_label()' \
    '    let sub_ok = new_label()' \
    '    let sub_positive_overflow = new_label()' \
    '    let sub_negative_overflow = new_label()' \
    '    let mul_ok = new_label()' \
    '    let mul_positive_overflow = new_label()' \
    '    let mul_minimum_ok = new_label()' \
    '    let mul_minimum_overflow = new_label()' \
    '    let div_ok = new_label()' \
    '    let div_zero = new_label()' \
    '    let div_minimum_overflow = new_label()' \
    '    let mod_ok = new_label()' \
    '    let mod_zero = new_label()' \
    '    let mod_minimum_overflow = new_label()' \
    '    define_label(entry_label)' \
    '    emit_r(17, 11)' \
    '    emit_imm(12, 97)' \
    '    emit_rrx(16, 11, 12, add_ok)' \
    '    emit_imm(12, 65)' \
    '    emit_rrx(16, 11, 12, add_positive_overflow)' \
    '    emit_imm(12, 66)' \
    '    emit_rrx(16, 11, 12, add_negative_overflow)' \
    '    emit_imm(12, 115)' \
    '    emit_rrx(16, 11, 12, sub_ok)' \
    '    emit_imm(12, 83)' \
    '    emit_rrx(16, 11, 12, sub_positive_overflow)' \
    '    emit_imm(12, 84)' \
    '    emit_rrx(16, 11, 12, sub_negative_overflow)' \
    '    emit_imm(12, 109)' \
    '    emit_rrx(16, 11, 12, mul_ok)' \
    '    emit_imm(12, 77)' \
    '    emit_rrx(16, 11, 12, mul_positive_overflow)' \
    '    emit_imm(12, 78)' \
    '    emit_rrx(16, 11, 12, mul_minimum_ok)' \
    '    emit_imm(12, 79)' \
    '    emit_rrx(16, 11, 12, mul_minimum_overflow)' \
    '    emit_imm(12, 100)' \
    '    emit_rrx(16, 11, 12, div_ok)' \
    '    emit_imm(12, 68)' \
    '    emit_rrx(16, 11, 12, div_zero)' \
    '    emit_imm(12, 69)' \
    '    emit_rrx(16, 11, 12, div_minimum_overflow)' \
    '    emit_imm(12, 114)' \
    '    emit_rrx(16, 11, 12, mod_ok)' \
    '    emit_imm(12, 82)' \
    '    emit_rrx(16, 11, 12, mod_zero)' \
    '    emit_imm(12, 70)' \
    '    emit_rrx(16, 11, 12, mod_minimum_overflow)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(add_ok)' \
    '    emit_imm(10, 0)' \
    '    emit_imm(2, 2)' \
    '    emit_imm(3, 3)' \
    '    emit_imm(9, 5)' \
    '    emit_jump(19, add_label)' \
    '    emit_jump(12, check_label)' \
    '    define_label(add_positive_overflow)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775807)' \
    '    emit_imm(3, 1)' \
    '    emit_jump(19, add_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(add_negative_overflow)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775808)' \
    '    emit_imm(3, 18446744073709551615)' \
    '    emit_jump(19, add_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(sub_ok)' \
    '    emit_imm(10, 0)' \
    '    emit_imm(2, 5)' \
    '    emit_imm(3, 3)' \
    '    emit_imm(9, 2)' \
    '    emit_jump(19, sub_label)' \
    '    emit_jump(12, check_label)' \
    '    define_label(sub_positive_overflow)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775807)' \
    '    emit_imm(3, 18446744073709551615)' \
    '    emit_jump(19, sub_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(sub_negative_overflow)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775808)' \
    '    emit_imm(3, 1)' \
    '    emit_jump(19, sub_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(mul_ok)' \
    '    emit_imm(10, 0)' \
    '    emit_imm(2, 6)' \
    '    emit_imm(3, 7)' \
    '    emit_imm(9, 42)' \
    '    emit_jump(19, mul_label)' \
    '    emit_jump(12, check_label)' \
    '    define_label(mul_positive_overflow)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775807)' \
    '    emit_imm(3, 2)' \
    '    emit_jump(19, mul_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(mul_minimum_ok)' \
    '    emit_imm(10, 0)' \
    '    emit_imm(2, 9223372036854775808)' \
    '    emit_imm(3, 1)' \
    '    emit_imm(9, 9223372036854775808)' \
    '    emit_jump(19, mul_label)' \
    '    emit_jump(12, check_label)' \
    '    define_label(mul_minimum_overflow)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775808)' \
    '    emit_imm(3, 18446744073709551615)' \
    '    emit_jump(19, mul_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(div_ok)' \
    '    emit_imm(10, 0)' \
    '    emit_imm(2, 18446744073709551609)' \
    '    emit_imm(3, 2)' \
    '    emit_imm(9, 18446744073709551613)' \
    '    emit_jump(19, div_label)' \
    '    emit_jump(12, check_label)' \
    '    define_label(div_zero)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 7)' \
    '    emit_imm(3, 0)' \
    '    emit_jump(19, div_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(div_minimum_overflow)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775808)' \
    '    emit_imm(3, 18446744073709551615)' \
    '    emit_jump(19, div_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(mod_ok)' \
    '    emit_imm(10, 0)' \
    '    emit_imm(2, 18446744073709551609)' \
    '    emit_imm(3, 2)' \
    '    emit_imm(9, 18446744073709551615)' \
    '    emit_jump(19, mod_label)' \
    '    emit_jump(12, check_label)' \
    '    define_label(mod_zero)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 7)' \
    '    emit_imm(3, 0)' \
    '    emit_jump(19, mod_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(mod_minimum_overflow)' \
    '    emit_imm(10, 1)' \
    '    emit_imm(2, 9223372036854775808)' \
    '    emit_imm(3, 18446744073709551615)' \
    '    emit_jump(19, mod_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(failure_label)' \
    '    emit_rx(13, 10, unexpected_label)' \
    '    emit_jump(12, accepted_label)' \
    '    define_label(check_label)' \
    '    emit_rrx(16, 0, 9, accepted_label)' \
    '    emit_jump(12, unexpected_label)' \
    '    define_label(accepted_label)' \
    '    emit_imm(0, 7)' \
    '    emit_r(0, 0)' \
    '    define_label(unexpected_label)' \
    '    emit_imm(0, 9)' \
    '    emit_r(0, 0)' \
    '    emit_checked_add(add_label, failure_label)' \
    '    emit_checked_sub(sub_label, failure_label)' \
    '    emit_checked_mul(mul_label, failure_label)' \
    '    emit_checked_div(div_label, failure_label)' \
    '    emit_checked_mod(mod_label, failure_label)' \
    '    let payload_ok = validate_payload()' \
    '    state publish_setup {' \
    '        to failed when (payload_ok != 1)' \
    '        let i = 0' \
    '        to publish_loop' \
    '    }' \
    '    state publish_loop {' \
    '        to publish when (i < word[2097040])' \
    '        return 1' \
    '    }' \
    '    state publish {' \
    '        write_byte(byte[33292288 + i])' \
    '        i = i + 1' \
    '        to publish_loop' \
    '    }' \
    '    state failed { return 0 }' \
    '}'
} | "$T/bc.exe" > "$T/int-emitter.tape" || {
  echo "bc(gamma_compiler.beta + checked Int probe) failed"
  exit 1
}
stamp_seed "$T/int-emitter.tape" "$SEED" "$T/int-emitter.exe" >/dev/null 2>&1
"$T/int-emitter.exe" > "$T/int-probe.tape"
int_emitter_status=$?
if [ "$int_emitter_status" != 1 ] || [ ! -s "$T/int-probe.tape" ]; then
  echo "Gamma checked Int probe emission failed: status $int_emitter_status" >&2
  exit 1
fi
stamp_seed "$T/int-probe.tape" "$SEED" "$T/int-probe.exe" >/dev/null 2>&1

{
  sed -n '1,$p' gamma_compiler.beta
  printf '%s\n' \
    'proc main() {' \
    '    let frontend_status = frontend_check_main()' \
    '    state checked {' \
    '        to failed when (frontend_status != 1)' \
    '        emit_reset()' \
    '        let entry_label = new_label()' \
    '        let stack_label = new_label()' \
    '        let heap_label = new_label()' \
    '        let add_label = new_label()' \
    '        let sub_label = new_label()' \
    '        let mul_label = new_label()' \
    '        let div_label = new_label()' \
    '        let mod_label = new_label()' \
    '        let single_label = new_label()' \
    '        let length_label = new_label()' \
    '        let get_label = new_label()' \
    '        let slice_label = new_label()' \
    '        let concat_label = new_label()' \
    '        let failure_label = new_label()' \
    '        let kind_ok_label = new_label()' \
    '        let stack_ok_label = new_label()' \
    '        let unexpected_label = new_label()' \
    '        word[2097000] = stack_label' \
    '        word[2096992] = add_label' \
    '        word[2096984] = sub_label' \
    '        word[2096976] = mul_label' \
    '        word[2096968] = div_label' \
    '        word[2096960] = mod_label' \
    '        word[2096952] = heap_label' \
    '        word[2096944] = single_label' \
    '        word[2096936] = length_label' \
    '        word[2096928] = get_label' \
    '        word[2096920] = slice_label' \
    '        word[2096912] = concat_label' \
    '        define_label(entry_label)' \
    '        emit_runtime_init()' \
    '        let body = word[8388608 + 24]' \
    '        let lower_status = lower_expr(body, 1)' \
    '        to lowered' \
    '    }' \
    '    state lowered {' \
    '        to failed when (lower_status != 1)' \
    '        emit_imm(8, 0)' \
    '        emit_rrx(16, 0, 8, kind_ok_label)' \
    '        emit_jump(12, unexpected_label)' \
    '        define_label(kind_ok_label)' \
    '        emit_imm(8, 16777216)' \
    '        emit_rrx(16, 252, 8, stack_ok_label)' \
    '        emit_jump(12, unexpected_label)' \
    '        define_label(stack_ok_label)' \
    '        emit_r(0, 1)' \
    '        define_label(failure_label)' \
    '        emit_imm(0, 253)' \
    '        emit_r(0, 0)' \
    '        define_label(unexpected_label)' \
    '        emit_imm(0, 254)' \
    '        emit_r(0, 0)' \
    '        emit_stack_reserver(stack_label, failure_label)' \
    '        emit_heap_allocator(heap_label, failure_label)' \
    '        emit_checked_add(add_label, failure_label)' \
    '        emit_checked_sub(sub_label, failure_label)' \
    '        emit_checked_mul(mul_label, failure_label)' \
    '        emit_checked_div(div_label, failure_label)' \
    '        emit_checked_mod(mod_label, failure_label)' \
    '        emit_bytes_single(single_label, heap_label, failure_label)' \
    '        emit_bytes_length(length_label, unexpected_label)' \
    '        emit_bytes_concat(concat_label, heap_label, add_label, unexpected_label)' \
    '        emit_bytes_slice(slice_label, heap_label, failure_label, unexpected_label)' \
    '        emit_bytes_get(get_label, failure_label, unexpected_label)' \
    '        let payload_ok = validate_payload()' \
    '        to publish_setup' \
    '    }' \
    '    state publish_setup {' \
    '        to failed when (payload_ok != 1)' \
    '        let i = 0' \
    '        to publish_loop' \
    '    }' \
    '    state publish_loop {' \
    '        to publish when (i < word[2097040])' \
    '        return 1' \
    '    }' \
    '    state publish {' \
    '        write_byte(byte[33292288 + i])' \
    '        i = i + 1' \
    '        to publish_loop' \
    '    }' \
    '    state failed { return 0 }' \
    '}'
} | "$T/bc.exe" > "$T/lowering-emitter.tape" || {
  echo "bc(gamma_compiler.beta + closed Int lowering gate) failed"
  exit 1
}
stamp_seed "$T/lowering-emitter.tape" "$SEED" "$T/lowering-emitter.exe" >/dev/null 2>&1

PASS=0; FAIL=0
for frame_mode in e f; do
  printf '%s' "$frame_mode" | "$T/frame-probe.exe" > "$T/frame-$frame_mode.out"
  frame_status=$?
  if [ "$frame_status" = 7 ] && [ ! -s "$T/frame-$frame_mode.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL Gamma frame ABI $frame_mode: status $frame_status, output $(wc -c < "$T/frame-$frame_mode.out" | tr -d ' ') bytes"
  fi
done
"$T/bytes-valid-probe.exe" > "$T/bytes-valid.out"
bytes_valid_status=$?
if [ "$bytes_valid_status" = 7 ] && [ ! -s "$T/bytes-valid.out" ]; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1))
  echo "  FAIL Bytes valid/deep probe: status $bytes_valid_status, output $(wc -c < "$T/bytes-valid.out" | tr -d ' ') bytes"
fi
for bytes_invalid_mode in a b c d e f g h i j k l m; do
  printf '%s' "$bytes_invalid_mode" | "$T/bytes-invalid-probe.exe" > "$T/bytes-invalid-$bytes_invalid_mode.out"
  bytes_invalid_status=$?
  if [ "$bytes_invalid_status" = 7 ] && [ ! -s "$T/bytes-invalid-$bytes_invalid_mode.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL Bytes failure class $bytes_invalid_mode: status $bytes_invalid_status, output $(wc -c < "$T/bytes-invalid-$bytes_invalid_mode.out" | tr -d ' ') bytes"
  fi
done
"$T/emitter.exe" > "$T/emitter.out"
emitter_status=$?
if [ "$emitter_status" = 1 ] && [ ! -s "$T/emitter.out" ]; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1))
  echo "  FAIL emitter probe: status $emitter_status, output $(wc -c < "$T/emitter.out" | tr -d ' ') bytes"
fi
for runtime_mode in h s H S o u; do
  printf '%s' "$runtime_mode" | "$T/runtime-probe.exe" > "$T/runtime-$runtime_mode.out"
  runtime_status=$?
  if [ "$runtime_status" = 7 ] && [ ! -s "$T/runtime-$runtime_mode.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL runtime $runtime_mode: status $runtime_status, output $(wc -c < "$T/runtime-$runtime_mode.out" | tr -d ' ') bytes"
  fi
done
for int_mode in a A B s S T m M N O d D E r R F; do
  printf '%s' "$int_mode" | "$T/int-probe.exe" > "$T/int-$int_mode.out"
  int_status=$?
  if [ "$int_status" = 7 ] && [ ! -s "$T/int-$int_mode.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL checked Int $int_mode: status $int_status, output $(wc -c < "$T/int-$int_mode.out" | tr -d ' ') bytes"
  fi
done
LOWER_CASE=0
lower_int() { # Gamma program  expected generated status  description
  LOWER_CASE=$((LOWER_CASE+1))
  printf '%s' "$1" | "$T/lowering-emitter.exe" > "$T/lower-$LOWER_CASE.tape"
  lower_compile_status=$?
  if [ "$lower_compile_status" != 1 ] || [ ! -s "$T/lower-$LOWER_CASE.tape" ]; then
    FAIL=$((FAIL+1))
    echo "  FAIL lower $3: compiler status $lower_compile_status"
    return
  fi
  stamp_seed "$T/lower-$LOWER_CASE.tape" "$SEED" "$T/lower-$LOWER_CASE.exe" >/dev/null 2>&1 || {
    FAIL=$((FAIL+1))
    echo "  FAIL lower $3: emitted tape could not be stamped"
    return
  }
  "$T/lower-$LOWER_CASE.exe" > "$T/lower-$LOWER_CASE.out"
  lower_runtime_status=$?
  if [ "$lower_runtime_status" = "$2" ] && [ ! -s "$T/lower-$LOWER_CASE.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL lower $3: status $lower_runtime_status, output $(wc -c < "$T/lower-$LOWER_CASE.out" | tr -d ' ') bytes"
  fi
}
lower_int '(def main () Int 7)' 7 'literal'
lower_int '(def main () Int (+ 2 (* 3 4)))' 14 'nested left-to-right arithmetic'
lower_int '(def main () Int (- 9 4))' 5 'subtraction'
lower_int '(def main () Int (* 6 7))' 42 'multiplication'
lower_int '(def main () Int (/ 7 2))' 3 'division'
lower_int '(def main () Int (% 7 4))' 3 'remainder'
lower_int '(def main () Int (eq 5 5))' 1 'equal true'
lower_int '(def main () Int (eq 5 6))' 0 'equal false'
lower_int '(def main () Int (lt -1 0))' 1 'less true'
lower_int '(def main () Int (lt 2 1))' 0 'less false'
lower_int '(def main () Int (if 0 (/ 1 0) 7))' 7 'zero selects else lazily'
lower_int '(def main () Int (if -2 9 (/ 1 0)))' 9 'nonzero selects then lazily'
lower_int '(def main () Int (if (lt 1 2) (+ 2 3) 0))' 5 'nested conditional control flow'
lower_int '(def main () Int (if (/ 1 0) 7 8))' 253 'condition trap precedes branch selection'
lower_int '(def main () Int (if 1 (/ 1 0) 7))' 253 'selected then trap is contained'
lower_int '(def main () Int (if 0 7 (/ 1 0)))' 253 'selected else trap is contained'
lower_int '(def main () Int (if (lt 1 2) (if 0 3 4) 5))' 4 'nested conditional joins'
lower_int '(def main () Int (+ 10 (if (eq 1 2) (/ 1 0) (* 3 4))))' 22 'outer spill survives conditional'
lower_int '(def main () Int (+ 9223372036854775807 1))' 253 'lowered addition overflow'
lower_int '(def main () Int (/ -9223372036854775808 -1))' 253 'lowered division overflow'
lower_int '(def main () Int (bytes_length (bytes_empty)))' 0 'empty Bytes length'
lower_int '(def main () Int (bytes_get (bytes_single 255) 0))' 255 'single Bytes access'
lower_int '(def main () Int (bytes_get (bytes_concat (bytes_single 7) (bytes_single 9)) 1))' 9 'concatenated Bytes access'
lower_int '(def main () Int (bytes_length (bytes_concat (bytes_single 1) (bytes_concat (bytes_single 2) (bytes_single 3)))))' 3 'nested Bytes concatenation length'
lower_int '(def main () Int (bytes_get (bytes_slice (bytes_concat (bytes_concat (bytes_single 10) (bytes_single 20)) (bytes_single 30)) 1 2) 1))' 30 'cross-rope Bytes slice'
lower_int '(def main () Int (bytes_length (bytes_slice (bytes_single 8) 1 0)))' 0 'zero Bytes slice at exact end'
lower_int '(def main () Int (bytes_get (if 0 (bytes_single (/ 1 0)) (bytes_single 44)) 0))' 44 'conditional selects one Bytes branch lazily'
lower_int '(def main () Int (+ 1 (bytes_get (bytes_slice (bytes_concat (bytes_single 8) (bytes_single 9)) 1 1) 0)))' 10 'outer Int spill survives Bytes lowering'
lower_int '(def main () Int (bytes_length (bytes_single 256)))' 253 'invalid constructed byte traps'
lower_int '(def main () Int (bytes_get (bytes_single 1) 1))' 253 'invalid Bytes index traps'
lower_int '(def main () Int (bytes_length (bytes_slice (bytes_single 1) 1 1)))' 253 'invalid Bytes range traps'
deterministic_source='(def main () Int (+ 10 (if (eq 1 2) (/ 1 0) (* 3 4))))'
printf '%s' "$deterministic_source" | "$T/lowering-emitter.exe" > "$T/lower-deterministic-a.tape"
deterministic_a_status=$?
printf '%s' "$deterministic_source" | "$T/lowering-emitter.exe" > "$T/lower-deterministic-b.tape"
deterministic_b_status=$?
if [ "$deterministic_a_status" = 1 ] && [ "$deterministic_b_status" = 1 ] &&
   cmp -s "$T/lower-deterministic-a.tape" "$T/lower-deterministic-b.tape"; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1))
  echo "  FAIL lower deterministic reconstruction: statuses $deterministic_a_status/$deterministic_b_status"
fi
unset deterministic_source deterministic_a_status deterministic_b_status
bytes_deterministic_source='(def main () Int (bytes_get (bytes_slice (bytes_concat (bytes_single 8) (bytes_single 9)) 1 1) 0))'
printf '%s' "$bytes_deterministic_source" | "$T/lowering-emitter.exe" > "$T/lower-bytes-deterministic-a.tape"
bytes_deterministic_a_status=$?
printf '%s' "$bytes_deterministic_source" | "$T/lowering-emitter.exe" > "$T/lower-bytes-deterministic-b.tape"
bytes_deterministic_b_status=$?
if [ "$bytes_deterministic_a_status" = 1 ] && [ "$bytes_deterministic_b_status" = 1 ] &&
   cmp -s "$T/lower-bytes-deterministic-a.tape" "$T/lower-bytes-deterministic-b.tape"; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1))
  echo "  FAIL lower Bytes deterministic reconstruction: statuses $bytes_deterministic_a_status/$bytes_deterministic_b_status"
fi
unset bytes_deterministic_source bytes_deterministic_a_status bytes_deterministic_b_status
tc() { # program  expect(1 ok / 0 type-error)  desc
  printf '%s' "$1" | "$T/tc.exe"; got=$?
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL want $2 got $got : $3"; fi
}
reject_source() { # name source-file
  name=$1
  source_file=$2
  set +e
  "$T/tc.exe" < "$source_file" > "$T/$name.out"
  got=$?
  if [ "$got" = 0 ] && [ ! -s "$T/$name.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL $name: source envelope returned $got with $(wc -c < "$T/$name.out" | tr -d ' ') output bytes"
  fi
}
accept_source() { # name source-file
  name=$1
  source_file=$2
  set +e
  "$T/tc.exe" < "$source_file" > "$T/$name.out"
  got=$?
  if [ "$got" = 1 ] && [ ! -s "$T/$name.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL $name: valid source returned $got with $(wc -c < "$T/$name.out" | tr -d ' ') output bytes"
  fi
}
# phase 1 — Int + typed functions
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 2 3))' 1 'well-typed'
cr_comment_program=$(printf '; before\r(def id ((x Int)) Int x)')
tc "$cr_comment_program" 1 'CR-terminated comment'
unset cr_comment_program
printf '; hidden\000\n(def id ((x Int)) Int x)' > "$T/comment-nul.gamma"
reject_source comment-nul "$T/comment-nul.gamma"
printf '(def\013id ((x Int)) Int x)' > "$T/vertical-tab.gamma"
reject_source vertical-tab "$T/vertical-tab.gamma"
printf '; hidden\177\n(def id ((x Int)) Int x)' > "$T/comment-del.gamma"
reject_source comment-del "$T/comment-del.gamma"
printf '; hidden\303\251\n(def id ((x Int)) Int x)' > "$T/comment-high.gamma"
reject_source comment-high "$T/comment-high.gamma"
# Place the second declaration exactly at source offset 2 MiB. The former
# declaration tables began at raw address 4 MiB and therefore aliased this byte
# because the source buffer begins at raw address 2 MiB.
printf '(def first () Int (later))\n;' > "$T/table-disjoint.gamma"
table_prefix_size=$(wc -c < "$T/table-disjoint.gamma" | tr -d ' ')
table_pad_size=$((2097151 - table_prefix_size))
dd if=/dev/zero bs="$table_pad_size" count=1 2>/dev/null | tr '\000' 'x' >> "$T/table-disjoint.gamma"
printf '\n(def later () Int 7)' >> "$T/table-disjoint.gamma"
accept_source table-disjoint "$T/table-disjoint.gamma"
unset table_prefix_size table_pad_size
# Cross the retired interpreter's 512-value scratch bound. Gamma arity is a
# language property, so frontend parsing/checking must not recurse once per row
# or inherit that oracle-private ceiling.
awk 'BEGIN {
  printf "(def wide ("
  for (i = 0; i < 600; i++) printf "(p%d Int)", i
  printf ") Int p599) (def main () Int (wide"
  for (i = 0; i < 600; i++) printf " %d", i
  print "))"
}' > "$T/wide-call.gamma"
accept_source wide-call "$T/wide-call.gamma"
awk 'BEGIN {
  printf "(data Wide (Mk"
  for (i = 0; i < 600; i++) printf " Int"
  printf ")) (def make () Wide (Mk"
  for (i = 0; i < 600; i++) printf " %d", i
  printf ")) (def last ((w Wide)) Int (match w ((Mk"
  for (i = 0; i < 600; i++) printf " x%d", i
  print ") x599)))"
}' > "$T/wide-constructor.gamma"
accept_source wide-constructor "$T/wide-constructor.gamma"
accept_source delta-compiler-foundation "$OMEGA_REPO_ROOT/source/delta/compiler/delta_compiler.gamma"
awk 'BEGIN {
  for (i = 0; i <= 32768; i++) printf "(def f%d () Int 0)\n", i
}' > "$T/function-capacity.gamma"
reject_source function-capacity "$T/function-capacity.gamma"
awk 'BEGIN {
  printf "(def f ((x Int)) Int x) (def main () Int (f"
  for (i = 0; i < 300000; i++) printf " 0"
  print "))"
}' > "$T/arena-capacity.gamma"
reject_source arena-capacity "$T/arena-capacity.gamma"
awk 'BEGIN {
  printf "(def main () Int "
  for (i = 0; i < 900; i++) printf "(let x%d 0 ", i
  printf "0"
  for (i = 0; i < 900; i++) printf ")"
  print ")"
}' > "$T/nesting-within-profile.gamma"
accept_source nesting-within-profile "$T/nesting-within-profile.gamma"
awk 'BEGIN {
  printf "(def main () Int "
  for (i = 0; i < 1100; i++) printf "(let x%d 0 ", i
  printf "0"
  for (i = 0; i < 1100; i++) printf ")"
  print ")"
}' > "$T/nesting-exhausted.gamma"
reject_source nesting-exhausted "$T/nesting-exhausted.gamma"
# fixed D16 program/declaration grammar and exact source exhaustion
tc '' 0 'empty program'
tc '; comment only' 0 'comment-only program'
tc '(data Nat (Z))' 0 'data-only program'
tc 'junk' 0 'junk-only program'
tc '(def main () Int 0) junk' 0 'trailing token'
tc '(def main () Int 0) (data Nat (Z))' 0 'data after function'
tc '(def main () Int 0))' 0 'stray closing delimiter'
tc '(def main () Int 0' 0 'missing closing delimiter'
tc '(fun main () Int 0)' 0 'unknown top-level declaration'
tc '(def main () Int (if 1 2 3 4))' 0 'if has exact arity'
tc '(def main () Int (+ 1 2 3))' 0 'operator has exact arity'
tc '(data Nat (Z)) (def main ((n Nat)) Int (match n (Z 0 1)))' 0 'match arm has exact arity'
tc '(data nat (Z)) (def main () Int 0)' 0 'declared type begins uppercase'
tc '(data Nat (z)) (def main () Int 0)' 0 'constructor begins uppercase'
tc '(def Main () Int 0)' 0 'function begins lowercase or underscore'
tc '(def main ((X Int)) Int X)' 0 'parameter begins lowercase or underscore'
tc '(def main () Int (let X 1 X))' 0 'local begins lowercase or underscore'
tc '(def if () Int 0)' 0 'keyword cannot be a declaration name'
tc '(def bytes_empty () Int 0)' 0 'Bytes builtin cannot be a declaration name'
tc '(def main () Int (let match 1 match))' 0 'keyword cannot be a binder'
tc '(data Bytes (B)) (def main () Int 0)' 0 'builtin type cannot be redeclared'
tc '(data Nat) (def main () Int 0)' 0 'data requires a constructor'
tc '(def main () Bytes bytes_empty)' 0 'Bytes builtin is not a bare variable'
tc '(data A garbage) (def main () Int 0)' 0 'invalid constructor punctuation rejects without nonprogress'
tc '(def id ((x Int)) Int x)' 1 'identity'
tc '(def f ((a Int) (b Int)) Int (if (lt a b) a b))' 1 'if/branches'
tc '(def f ((a Int)) Int (let y (+ a 1) (* y y)))' 1 'let'
tc '(def f ((a Int)) Int (g a)) (def g ((x Int)) Int x)' 1 'forward call'
tc '(def min () Int -9223372036854775808) (def max () Int 9223372036854775807)' 1 'signed Int literal bounds'
tc '(def bad () Int 9223372036854775808)' 0 'positive Int literal overflow'
tc '(def bad () Int -9223372036854775809)' 0 'negative Int literal overflow'
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 2))' 0 'arity too few'
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 1 2 3))' 0 'arity too many'
tc '(def main () Int (nope 1))' 0 'unknown function'
tc '(def f ((x Nope)) Nope x)' 0 'unknown declared type'
# D16 compact Bytes type and closed builtin signatures
tc '(def one () Bytes (bytes_single 255)) (def main ((b Bytes)) Bytes (bytes_concat (bytes_empty) (bytes_slice b 0 (bytes_length b))))' 1 'Bytes constructors and views'
tc '(def main ((b Bytes)) Int (bytes_get b 0))' 1 'Bytes indexed read'
tc '(def bad () Bytes (bytes_single (bytes_empty)))' 0 'bytes_single requires Int'
tc '(def bad ((b Bytes)) Int (bytes_length 1))' 0 'bytes_length requires Bytes'
tc '(def bad ((b Bytes)) Int (bytes_get b))' 0 'bytes_get arity'
tc '(def bad ((b Bytes)) Bytes (bytes_slice b 0))' 0 'bytes_slice arity'
tc '(def bad ((b Bytes)) Bytes (bytes_concat b 0))' 0 'bytes_concat argument type'
tc '(def bad ((b Bytes)) Int (match b (rest 0)))' 0 'Bytes is not algebraic'
# phase 2 — data declarations (ADTs) + match, well-typed
tc '(data Nat (Z) (S Nat)) (def pred ((n Nat)) Nat (match n (Z Z) ((S m) m))) (def main () Nat (pred (S (S Z))))' 1 'Nat pred'
tc '(data List (Nil) (Cons Int List)) (def len ((xs List)) Int (match xs (Nil 0) ((Cons h t) (+ 1 (len t)))))' 1 'list length'
tc '(data Nat (Z) (S Nat)) (def plus ((a Nat) (b Nat)) Nat (match a (Z b) ((S m) (S (plus m b)))))' 1 'Nat plus'
tc '(data A (MkA B)) (data B (MkB A)) (def keep ((a A)) A a)' 1 'forward and mutual nominal type references'
tc '(data Nat (Z) (S Nat)) (def classify ((n Nat)) Int (match n (Z 0) (rest 1)))' 1 'final catch-all is exhaustive'
# phase 2 — TYPE ERRORS
tc '(data List (Nil) (Cons Int List)) (def bad ((xs List)) Int (+ xs 1))' 0 'Int op on a List'
tc '(data List (Nil) (Cons Int List)) (def bad () List (Cons Nil Nil))' 0 'Cons wants Int got List'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n (Z 0) ((S m) m)))' 0 'match arms differ'
tc '(data Nat (Z) (S Nat)) (data List (Nil) (Cons Int List)) (def bad ((n Nat)) Int (match n (Nil 0) (x 1)))' 0 'Nil pattern on a Nat'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (+ n 1))' 0 'return Nat but body Int'
# phase 2 — CONSTRUCTOR application and pattern arity (distinct from call arity above)
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (S (S n)))' 1 'control: nested constructor ok'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (S Z Z))' 0 'constructor too many args'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (S))' 0 'constructor too few args'
tc '(data Nat (Z) (S Nat)) (def bad ((n Int)) Nat (S n))' 0 'constructor arg wrong type (S on Int)'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (Nope n))' 0 'unknown constructor'
tc '(data Pair (Mk Int Int)) (def bad ((p Pair)) Int (match p ((Mk a) a)))' 0 'pattern arity wrong (1 of 2)'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n (Z 0)))' 0 'missing constructor arm'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n (Z 0) (Z 1) ((S m) 2)))' 0 'duplicate constructor arm'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n (rest 0) (Z 1)))' 0 'arm after catch-all'
tc '(def bad ((n Int)) Int (match n (rest 0)))' 0 'match requires algebraic scrutinee'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n))' 0 'match requires an arm'
echo "gamma compiler substrate: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
