use super::{
    Reg64, append_cmp_r10_r11, append_cmp_r12d_imm32, append_failure_branch,
    append_float_compare_r10_r11, append_jcc_rel32, append_jmp_rel32, append_load_reg_from_r15,
    append_mov_r12d_imm32, append_mov_r15_imm64, append_mov_reg_imm64,
};
use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use omega_target_operations::StateGuardOperator;
use psi_diagnostics::Diagnostic;

pub fn dispatch_loop_enter_width() -> usize {
    6
}

pub fn dispatch_case_enter_width() -> usize {
    13
}

pub fn dispatch_state_write_width() -> usize {
    11
}

pub fn dispatch_case_leave_width() -> usize {
    5
}

pub fn encode_dispatch_loop_enter_bytes(entry_dispatch_index: u32) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_loop_enter_width());
    append_mov_r12d_imm32(&mut bytes, entry_dispatch_index)?;
    Ok(bytes)
}

pub fn encode_dispatch_case_enter_bytes(
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_case_enter_width());
    append_cmp_r12d_imm32(&mut bytes, dispatch_index)?;
    append_jcc_rel32(&mut bytes, 0x85, skip_byte_distance - 9)?; // jne
    Ok(bytes)
}

pub fn encode_dispatch_state_write_bytes(
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_state_write_width());
    append_mov_r12d_imm32(&mut bytes, dispatch_index)?;
    append_jmp_rel32(&mut bytes, case_leave_byte_distance - 7)?;
    Ok(bytes)
}

pub fn encode_dispatch_case_leave_bytes(loop_byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_case_leave_width());
    append_jmp_rel32(&mut bytes, loop_byte_distance - 5)?;
    Ok(bytes)
}

pub fn dispatch_loop_enter_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R12])
}

pub fn dispatch_case_enter_register_writes() -> RegisterSet {
    RegisterSet::default()
}

pub fn dispatch_case_enter_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn dispatch_state_write_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R12])
}

pub fn dispatch_case_leave_register_writes() -> RegisterSet {
    RegisterSet::default()
}

pub fn dispatch_guard_compare_static_width(is_float: bool, byte_size: usize) -> usize {
    // mov r15, imm64 (10) + load r10, [r15+disp32] (7; 8 for the 0x66-prefixed
    // 2-byte form) + mov r11, imm64 (10) + compare + jcc rel32 (6). Integer
    // compare is `cmp r10,r11` (3; 4 with the 0x66 prefix); float is
    // movq/movd + movq/movd + ucomisd/ucomiss.
    let load_width = if !is_float && byte_size == 2 { 8 } else { 7 };
    // Floats prepend a 6-byte `jp` parity branch before the failure jcc (NaN routing).
    let float_parity_branch = if is_float { 6 } else { 0 };
    10 + load_width
        + 10
        + runtime_float_or_integer_compare_width(is_float, byte_size)
        + 6
        + float_parity_branch
}

fn runtime_float_or_integer_compare_width(is_float: bool, byte_size: usize) -> usize {
    if is_float {
        // f64: movq(5)+movq(5)+ucomisd(4). f32: movd(5)+movd(5)+ucomiss(3) — the
        // single-precision SSE compare drops the 0x66 prefix, so it is 1 byte shorter.
        if byte_size == 4 { 13 } else { 14 }
    } else if byte_size == 2 {
        // 16-bit `cmp r10w,r11w` carries the 0x66 operand-size prefix.
        4
    } else {
        3
    }
}

/// Narrow a guard's float `expected_value` (stored as f64 bits) to the operand's
/// width: for a 4-byte float operand the comparison runs in single precision, so the
/// immediate must be the f32 bit pattern. Exact for any value representable in f32
/// (which a constant compared against an f32 field always is).
fn float_compare_expected_bits(expected_value: i64, byte_size: usize) -> u64 {
    if byte_size == 4 {
        u64::from((f64::from_bits(expected_value as u64) as f32).to_bits())
    } else {
        expected_value as u64
    }
}

pub fn encode_dispatch_guard_compare_static_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
        )));
    }
    let mut bytes = Vec::with_capacity(dispatch_guard_compare_static_width(is_float, byte_size));
    // Storage base; the imm64 (at instruction start + 2) is relocated to the
    // guard's storage-region data symbol by the relocation planner.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_reg_from_r15(&mut bytes, Reg64::R10, byte_offset, byte_size)?;
    let expected_bits = if is_float {
        float_compare_expected_bits(expected_value, byte_size)
    } else {
        expected_value as u64
    };
    append_mov_reg_imm64(&mut bytes, Reg64::R11, expected_bits);
    if is_float {
        append_float_compare_r10_r11(&mut bytes, byte_size);
    } else {
        append_cmp_r10_r11(&mut bytes, byte_size)?;
    }
    // `skip_byte_distance` is anchored at the instruction's rel32 field start
    // (`current.offset + byte_width - 4`, now architecture-aware in the branch-
    // distance helper). The jcc rel is measured from the field's end, 4 bytes
    // later, so the relative target is `skip_byte_distance - 4`.
    append_failure_branch(&mut bytes, operator, skip_byte_distance - 4, is_float)?;
    debug_assert_eq!(
        bytes.len(),
        dispatch_guard_compare_static_width(is_float, byte_size)
    );
    Ok(bytes)
}

/// Exact registers overwritten by a storage-backed static dispatch guard.
/// Integer guards stay in the GPR bank; float guards additionally stage the
/// operands through xmm0/xmm1 before `ucomis*` writes condition flags.
pub fn dispatch_guard_compare_static_register_writes(is_float: bool) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R15,
    ];
    if is_float {
        registers.extend([MachineRegister::X86Xmm(0), MachineRegister::X86Xmm(1)]);
    }
    RegisterSet::new(registers)
}

pub fn dispatch_guard_compare_static_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}
