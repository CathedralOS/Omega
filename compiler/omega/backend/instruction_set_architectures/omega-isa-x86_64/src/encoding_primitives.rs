//! Crate-internal x86-64 byte-building primitives shared by focused encoders.
//!
//! This module owns instruction mechanics only. It does not select operations,
//! assign ABI locations, derive relocations, or create semantic policy.

use omega_target_operations::{InstructionOperandLike, StateGuardOperator};
use psi_diagnostics::Diagnostic;

pub(crate) fn x86_gpr_number(register: omega_calling_conventions::MachineRegister) -> Option<u8> {
    Some(match register {
        omega_calling_conventions::MachineRegister::X86Rax => 0,
        omega_calling_conventions::MachineRegister::X86Rcx => 1,
        omega_calling_conventions::MachineRegister::X86Rdx => 2,
        omega_calling_conventions::MachineRegister::X86Rbx => 3,
        omega_calling_conventions::MachineRegister::X86Rsp => 4,
        omega_calling_conventions::MachineRegister::X86Rbp => 5,
        omega_calling_conventions::MachineRegister::X86Rsi => 6,
        omega_calling_conventions::MachineRegister::X86Rdi => 7,
        omega_calling_conventions::MachineRegister::X86R8 => 8,
        omega_calling_conventions::MachineRegister::X86R9 => 9,
        omega_calling_conventions::MachineRegister::X86R10 => 10,
        omega_calling_conventions::MachineRegister::X86R11 => 11,
        omega_calling_conventions::MachineRegister::X86R12 => 12,
        omega_calling_conventions::MachineRegister::X86R13 => 13,
        omega_calling_conventions::MachineRegister::X86R14 => 14,
        omega_calling_conventions::MachineRegister::X86R15 => 15,
        _ => return None,
    })
}

/// Compare the bits already in r10 (left) and r11 (right) as `byte_size`-wide IEEE
/// floats via the SSE unit. For an 8-byte operand: `movq` into xmm0/xmm1 + `ucomisd`
/// (double precision). For a 4-byte operand: `movd` the low dword + `ucomiss` (single
/// precision). `ucomis*` sets CF/ZF exactly like an unsigned integer `cmp` (and PF on
/// unordered/NaN, which the unsigned failure branches ignore — a documented first-cut
/// limitation), so the same unsigned/equal failure-jcc conditions apply.
pub(crate) fn append_float_compare_r10_r11(bytes: &mut Vec<u8>, byte_size: usize) {
    if byte_size == 4 {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xcb]); // movd xmm1, r11d
        bytes.extend([0x0f, 0x2e, 0xc1]); // ucomiss xmm0, xmm1
    } else {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xcb]); // movq xmm1, r11
        bytes.extend([0x66, 0x0f, 0x2e, 0xc1]); // ucomisd xmm0, xmm1
    }
}

pub(crate) fn append_input_delimiter_check(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    failure_branch_distance: isize,
) -> Result<(), Diagnostic> {
    append_load_al_from_r15(bytes, byte_offset)?;
    bytes.extend([0x3c, 10]); // cmp al, '\n'
    append_jcc_rel32(bytes, 0x84, 21)?; // je success
    bytes.extend([0x3c, 13]); // cmp al, '\r'
    append_jcc_rel32(bytes, 0x84, 13)?; // je success
    bytes.extend([0x3c, 0]); // cmp al, 0
    append_jcc_rel32(bytes, 0x84, 5)?; // je success
    append_jmp_rel32(bytes, failure_branch_distance)?;
    Ok(())
}

pub(crate) fn append_failure_branch(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    failure_branch_distance: isize,
    is_float: bool,
) -> Result<(), Diagnostic> {
    // The guard jumps to the failure branch when the comparison is FALSE, so each
    // operator maps to its negation. Ordering uses signed (jl/jg/...) or unsigned
    // (jb/ja/...) conditions per the operand type.
    let opcode = match operator {
        StateGuardOperator::Equal => 0x85,                  // jne
        StateGuardOperator::NotEqual => 0x84,               // je
        StateGuardOperator::Greater => 0x8e,                // jle
        StateGuardOperator::GreaterOrEqual => 0x8c,         // jl
        StateGuardOperator::Less => 0x8d,                   // jge
        StateGuardOperator::LessOrEqual => 0x8f,            // jg
        StateGuardOperator::GreaterUnsigned => 0x86,        // jbe
        StateGuardOperator::GreaterOrEqualUnsigned => 0x82, // jb
        StateGuardOperator::LessUnsigned => 0x83,           // jae
        StateGuardOperator::LessOrEqualUnsigned => 0x87,    // ja
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime compare operator `{operator:?}` is not implemented yet"
            )));
        }
    };
    // IEEE semantics for a NaN operand: every comparison is FALSE except `!=` (true).
    // `ucomis*` reports an unordered/NaN operand by setting PF=1 (alongside ZF=CF=1),
    // which the ZF/CF-only failure jcc above misreads as "equal". Prepend a parity
    // branch so NaN is routed correctly. This 6-byte `jp` sits BEFORE the main jcc, so
    // the main jcc's own rel32 is unchanged (both it and its target shift down by 6);
    // the float width functions account for the extra 6 bytes.
    if is_float {
        if matches!(operator, StateGuardOperator::NotEqual) {
            // `!=` on NaN is TRUE (guard succeeds): jump PAST the 6-byte `je` so NaN
            // falls through to the success arm instead of taking the equal-failure jump.
            append_jcc_rel32(bytes, 0x8a, 6)?; // jp over the je
        } else {
            // Every other operator is FALSE on NaN (guard fails): jump to the same
            // failure arm as the main jcc, which now sits 6 bytes further along.
            append_jcc_rel32(bytes, 0x8a, failure_branch_distance + 6)?; // jp to failure
        }
    }
    append_jcc_rel32(bytes, opcode, failure_branch_distance)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reg64 {
    R10,
    R11,
}

pub(crate) fn append_mov_reg_imm64(bytes: &mut Vec<u8>, register: Reg64, value: u64) {
    match register {
        Reg64::R10 => append_mov_r10_imm64(bytes, value),
        Reg64::R11 => {
            bytes.extend([0x49, 0xbb]);
            bytes.extend(value.to_le_bytes());
        }
    }
}

pub(crate) fn append_mov_rax_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xb8]);
    bytes.extend(value.to_le_bytes());
}

pub(crate) fn append_mov_rdx_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xba]);
    bytes.extend(value.to_le_bytes());
}

pub(crate) fn append_mov_r10_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xba]);
    bytes.extend(value.to_le_bytes());
}

pub(crate) fn append_mov_r14_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbe]);
    bytes.extend(value.to_le_bytes());
}

/// The CROSS-REGION index base (place materializer): when a ScaledIndex
/// slot lives in a different region than the place's own base, r11 first
/// holds the INDEX region's base, then loads the index through itself --
/// no extra scratch register enters the discipline.
pub(crate) fn append_mov_r11_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbb]);
    bytes.extend(value.to_le_bytes());
}

/// Exact-width unsigned index load through r11's own value (the
/// cross-region index base pattern).
pub(crate) fn append_load_unsigned_r11_from_r11(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x45, 0x0f, 0xb6, 0x9b]),
        2 => bytes.extend([0x45, 0x0f, 0xb7, 0x9b]),
        4 => bytes.extend([0x45, 0x8b, 0x9b]),
        8 => bytes.extend([0x4d, 0x8b, 0x9b]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime indexes yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_mov_r15_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbf]);
    bytes.extend(value.to_le_bytes());
}

pub(crate) fn append_mov_reg_reg(bytes: &mut Vec<u8>, destination: Reg64, source: Reg64) {
    match (destination, source) {
        (Reg64::R10, Reg64::R10) => bytes.extend([0x4d, 0x89, 0xd2]),
        (Reg64::R10, Reg64::R11) => bytes.extend([0x4d, 0x89, 0xda]),
        (Reg64::R11, Reg64::R10) => bytes.extend([0x4d, 0x89, 0xd3]),
        (Reg64::R11, Reg64::R11) => bytes.extend([0x4d, 0x89, 0xdb]),
    }
}

pub(crate) fn append_push_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x41, 0x52]); // push r10
}

pub(crate) fn append_pop_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x41, 0x5a]); // pop r10
}

// --- Helpers for the runtime-length text-append memcpy (`rep movsb`) ---

pub(crate) fn append_mov_rcx_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xb9]); // mov rcx, imm64
    bytes.extend(value.to_le_bytes());
}

pub(crate) fn append_load_rax_from_rcx(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x48, 0x8b, 0x81]); // mov rax, [rcx + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_rcx_from_rcx(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x48, 0x8b, 0x89]); // mov rcx, [rcx + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_mov_r10_r14(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x89, 0xf2]); // mov r10, r14
}

pub(crate) fn append_add_r10_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xda]); // add r10, r11
}

pub(crate) fn append_add_r10_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!("X86_64 encoder cannot add offset `{value}` to r10"))
    })?;
    bytes.extend([0x49, 0x81, 0xc2]); // add r10, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

pub(crate) fn append_add_r11_rcx(bytes: &mut Vec<u8>) {
    bytes.extend([0x49, 0x01, 0xcb]); // add r11, rcx
}

pub(crate) fn append_add_r11_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!("X86_64 encoder cannot add offset `{value}` to r11"))
    })?;
    bytes.extend([0x49, 0x81, 0xc3]); // add r11, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

pub(crate) fn append_mov_r11_rcx(bytes: &mut Vec<u8>) {
    bytes.extend([0x49, 0x89, 0xcb]); // mov r11, rcx
}

pub(crate) fn append_load_r11_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x9f]); // mov r11, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_rax_from_r15_width(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x0f, 0xb6, 0x87]),
        2 => bytes.extend([0x41, 0x0f, 0xb7, 0x87]),
        4 => bytes.extend([0x41, 0x8b, 0x87]),
        8 => bytes.extend([0x49, 0x8b, 0x87]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte bit containers"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_store_r11_to_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0x9f]); // mov [r15 + disp32], r11
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_store_r11_to_r15_width(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x45, 0x88, 0x9f]),
        2 => bytes.extend([0x66, 0x45, 0x89, 0x9f]),
        4 => bytes.extend([0x45, 0x89, 0x9f]),
        8 => bytes.extend([0x4d, 0x89, 0x9f]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot store {byte_size}-byte bit containers"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_mov_rsi_rax(bytes: &mut Vec<u8>) {
    bytes.extend([0x48, 0x89, 0xc6]); // mov rsi, rax
}

pub(crate) fn append_mov_rdi_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x4c, 0x89, 0xd7]); // mov rdi, r10
}

pub(crate) fn append_rep_movsb(bytes: &mut Vec<u8>) {
    bytes.extend([0xf3, 0xa4]); // rep movsb (copy rcx bytes [rsi]->[rdi], DF=0)
}

pub(crate) fn append_mov_r12d_imm32(bytes: &mut Vec<u8>, value: u32) -> Result<(), Diagnostic> {
    bytes.extend([0x41, 0xbc]);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

pub(crate) fn append_cmp_r12d_imm32(bytes: &mut Vec<u8>, value: u32) -> Result<(), Diagnostic> {
    bytes.extend([0x41, 0x81, 0xfc]);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

pub(crate) fn append_add_r14_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xde]); // add r14, r11
}

pub(crate) fn append_load_al_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x41, 0x8a, 0x87]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_r15_from_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xbe]); // mov r15, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_r15_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xbf]); // mov r15, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_add_r15_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot add offset `{value}` to r15"
        ))
    })?;
    bytes.extend([0x49, 0x81, 0xc7]); // add r15, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

pub(crate) fn append_store_r15_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0xbe]); // mov [r14 + disp32], r15
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_r10_from_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x96]); // mov r10, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_mov_r14_r15(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x89, 0xfe]); // mov r14, r15
}

pub(crate) fn append_add_r15_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xdf]); // add r15, r11
}

pub(crate) fn append_load_rdx_from_r10(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x92]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_r10_from_r10(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x92]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_r8_from_r10(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x82]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_rax_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_rcx_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_r14_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xb7]); // mov r14, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_r14_from_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xb6]); // mov r14, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_imul_r11_imm32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend([0x4d, 0x69, 0xdb]); // imul r11, r11, imm32
    bytes.extend(value.to_le_bytes());
}

pub(crate) fn append_load_unsigned_r10_from_r10(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x45, 0x0f, 0xb6, 0x92]),
        2 => bytes.extend([0x45, 0x0f, 0xb7, 0x92]),
        4 => bytes.extend([0x45, 0x8b, 0x92]),
        8 => bytes.extend([0x4d, 0x8b, 0x92]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime indexes yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_imul_r10_imm32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend([0x4d, 0x69, 0xd2]); // imul r10, r10, imm32
    bytes.extend(value.to_le_bytes());
}

pub(crate) fn append_add_r14_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xd6]); // add r14, r10
}

pub(crate) fn append_add_r15_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xd7]); // add r15, r10
}

pub(crate) fn append_add_rax_r11(bytes: &mut Vec<u8>) {
    // add rax, r11 -- REX.W+REX.R (0x4c), opcode 0x01, ModRM 11 reg=r11(011) rm=rax(000) = 0xd8
    bytes.extend([0x4c, 0x01, 0xd8]);
}

pub(crate) fn append_store_r15_to_rax(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x89, 0xb8]); // mov [rax + disp32], r15
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_store_r11_to_rax(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x89, 0x98]); // mov [rax + disp32], r11
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_r11_from_rax(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x8b, 0x98]); // mov r11, [rax + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_mov_rax_r15(bytes: &mut Vec<u8>) {
    // mov rax, r15 -- REX.W+REX.R(no)+REX.B(r15 src as r/m): 0x4c 0x89 0xf8
    bytes.extend([0x4c, 0x89, 0xf8]);
}

pub(crate) fn append_mov_rax_r10(bytes: &mut Vec<u8>) {
    // mov rax, r10 -- 89 /r with r10 in the reg field (REX.R) and rax in r/m.
    bytes.extend([0x4c, 0x89, 0xd0]);
}

/// Byte count of [`append_mov_rax_r10`].
pub(crate) const MOV_RAX_R10_WIDTH: usize = 3;

pub(crate) fn element_scale(element_byte_size: usize) -> Result<i32, Diagnostic> {
    i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale runtime index by element size `{element_byte_size}`"
        ))
    })
}

pub(crate) fn append_load_reg_from_rax(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        // mov r10{b,w,d,}, [rax + disp32] -- ModRM mod=10 reg=r10(010) rm=rax(000) = 0x90
        (Reg64::R10, 1) => bytes.extend([0x44, 0x8a, 0x90]),
        (Reg64::R10, 2) => bytes.extend([0x66, 0x44, 0x8b, 0x90]),
        (Reg64::R10, 4) => bytes.extend([0x44, 0x8b, 0x90]),
        (Reg64::R10, 8) => bytes.extend([0x4c, 0x8b, 0x90]),
        // mov r11{b,w,d,}, [rax + disp32] -- ModRM mod=10 reg=r11(011) rm=rax(000) = 0x98
        (Reg64::R11, 1) => bytes.extend([0x44, 0x8a, 0x98]),
        (Reg64::R11, 2) => bytes.extend([0x66, 0x44, 0x8b, 0x98]),
        (Reg64::R11, 4) => bytes.extend([0x44, 0x8b, 0x98]),
        (Reg64::R11, 8) => bytes.extend([0x4c, 0x8b, 0x98]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime operands yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Load an unsigned integer into a full address-calculation register.
///
/// Unlike ordinary narrow value loads, byte and word indexes must clear the
/// destination's upper bits before scaling.
pub(crate) fn append_load_unsigned_reg_from_rax(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        (Reg64::R10, 1) => bytes.extend([0x44, 0x0f, 0xb6, 0x90]),
        (Reg64::R10, 2) => bytes.extend([0x44, 0x0f, 0xb7, 0x90]),
        (Reg64::R11, 1) => bytes.extend([0x44, 0x0f, 0xb6, 0x98]),
        (Reg64::R11, 2) => bytes.extend([0x44, 0x0f, 0xb7, 0x98]),
        (_, 4 | 8) => {
            return append_load_reg_from_rax(bytes, destination, byte_offset, byte_size);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime indexes yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_rax_from_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x8a, 0x86]),
        2 => bytes.extend([0x66, 0x41, 0x8b, 0x86]),
        4 => bytes.extend([0x41, 0x8b, 0x86]),
        8 => bytes.extend([0x49, 0x8b, 0x86]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte storage values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn load_rax_from_r14_width(byte_size: usize) -> usize {
    match byte_size {
        2 => 8,
        1 | 4 | 8 => 7,
        _ => 7,
    }
}

pub(crate) fn append_load_reg_from_r15(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        (Reg64::R10, 1) => bytes.extend([0x45, 0x8a, 0x97]),
        (Reg64::R10, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x97]),
        (Reg64::R10, 4) => bytes.extend([0x45, 0x8b, 0x97]),
        (Reg64::R10, 8) => bytes.extend([0x4d, 0x8b, 0x97]),
        (Reg64::R11, 1) => bytes.extend([0x45, 0x8a, 0x9f]),
        (Reg64::R11, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x9f]),
        (Reg64::R11, 4) => bytes.extend([0x45, 0x8b, 0x9f]),
        (Reg64::R11, 8) => bytes.extend([0x4d, 0x8b, 0x9f]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime operands yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_unsigned_reg_from_r15(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        (Reg64::R10, 1) => bytes.extend([0x45, 0x0f, 0xb6, 0x97]),
        (Reg64::R10, 2) => bytes.extend([0x45, 0x0f, 0xb7, 0x97]),
        (Reg64::R11, 1) => bytes.extend([0x45, 0x0f, 0xb6, 0x9f]),
        (Reg64::R11, 2) => bytes.extend([0x45, 0x0f, 0xb7, 0x9f]),
        (_, 4 | 8) => {
            return append_load_reg_from_r15(bytes, destination, byte_offset, byte_size);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime indexes yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_load_unsigned_reg_from_r14(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        (Reg64::R10, 1) => bytes.extend([0x45, 0x0f, 0xb6, 0x96]),
        (Reg64::R10, 2) => bytes.extend([0x45, 0x0f, 0xb7, 0x96]),
        (Reg64::R11, 1) => bytes.extend([0x45, 0x0f, 0xb6, 0x9e]),
        (Reg64::R11, 2) => bytes.extend([0x45, 0x0f, 0xb7, 0x9e]),
        (_, 4 | 8) => {
            return append_load_reg_from_r14(bytes, destination, byte_offset, byte_size);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime indexes yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// The r14-base twin of `append_load_reg_from_r15` (ModRM r/m = r14): the
/// place-compare materializer walks its LEFT operand's address in r14 (the
/// CopyPlaces source discipline) and loads the operand through it.
pub(crate) fn append_load_reg_from_r14(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        (Reg64::R10, 1) => bytes.extend([0x45, 0x8a, 0x96]),
        (Reg64::R10, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x96]),
        (Reg64::R10, 4) => bytes.extend([0x45, 0x8b, 0x96]),
        (Reg64::R10, 8) => bytes.extend([0x4d, 0x8b, 0x96]),
        (Reg64::R11, 1) => bytes.extend([0x45, 0x8a, 0x9e]),
        (Reg64::R11, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x9e]),
        (Reg64::R11, 4) => bytes.extend([0x45, 0x8b, 0x9e]),
        (Reg64::R11, 8) => bytes.extend([0x4d, 0x8b, 0x9e]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime operands yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_store_rax_to_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x88, 0x87]),
        2 => bytes.extend([0x66, 0x41, 0x89, 0x87]),
        4 => bytes.extend([0x41, 0x89, 0x87]),
        8 => bytes.extend([0x49, 0x89, 0x87]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot store {byte_size}-byte runtime values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_store_r10_to_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = i32::try_from(byte_offset)
        .map_err(|_| Diagnostic::error("RFLAGS snapshot destination offset exceeds i32"))?;
    // mov qword ptr [r15+disp32], r10
    bytes.extend([0x4d, 0x89, 0x97]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_store_r14_to_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0xb7]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_store_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        // mov [r14+disp32], r10{b,w,d,} -- ModRM reg=r10, r/m=r14
        1 => bytes.extend([0x45, 0x88, 0x96]),
        2 => bytes.extend([0x66, 0x45, 0x89, 0x96]),
        4 => bytes.extend([0x45, 0x89, 0x96]),
        8 => bytes.extend([0x4d, 0x89, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot store {byte_size}-byte runtime values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_xchg_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        // XCHG with a memory operand is implicitly locked.
        1 => bytes.extend([0x45, 0x86, 0x96]),
        2 => bytes.extend([0x66, 0x45, 0x87, 0x96]),
        4 => bytes.extend([0x45, 0x87, 0x96]),
        8 => bytes.extend([0x4d, 0x87, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot atomically exchange a {byte_size}-byte runtime value"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_cmp_r10_r11(bytes: &mut Vec<u8>, byte_size: usize) -> Result<(), Diagnostic> {
    match byte_size {
        1 => bytes.extend([0x45, 0x38, 0xda]),
        2 => bytes.extend([0x66, 0x45, 0x39, 0xda]),
        4 => bytes.extend([0x45, 0x39, 0xda]),
        8 => bytes.extend([0x4d, 0x39, 0xda]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot compare {byte_size}-byte runtime values yet"
            )));
        }
    }
    Ok(())
}

pub(crate) fn append_jcc_rel32(
    bytes: &mut Vec<u8>,
    opcode: u8,
    byte_distance: isize,
) -> Result<(), Diagnostic> {
    let displacement = rel32(byte_distance)?;
    bytes.extend([0x0f, opcode]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn append_jmp_rel32(
    bytes: &mut Vec<u8>,
    byte_distance: isize,
) -> Result<(), Diagnostic> {
    let displacement = rel32(byte_distance)?;
    bytes.push(0xe9);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub(crate) fn for_each_runtime_copy_chunk(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
    mut visit: impl FnMut(usize, usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut remaining = byte_count;
    let mut offset = 0usize;

    while remaining > 0 {
        let source_offset = source_base_offset + offset;
        let target_offset = target_base_offset + offset;
        let chunk_size =
            if remaining >= 8 && source_offset.is_multiple_of(8) && target_offset.is_multiple_of(8)
            {
                8
            } else if remaining >= 4
                && source_offset.is_multiple_of(4)
                && target_offset.is_multiple_of(4)
            {
                4
            } else {
                1
            };

        visit(offset, chunk_size)?;
        offset += chunk_size;
        remaining -= chunk_size;
    }

    Ok(())
}

pub(crate) fn load_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 7,
        // The 2-byte form is the 4-byte form plus the 0x66 operand-size prefix.
        2 => 8,
        _ => 0,
    }
}

pub(crate) fn unsigned_load_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 2 => 8,
        4 | 8 => 7,
        _ => 0,
    }
}

pub(crate) fn store_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 7,
        // The 2-byte form is the 4-byte form plus the 0x66 operand-size prefix.
        2 => 8,
        _ => 0,
    }
}

pub(crate) fn immediate_i32<T: InstructionOperandLike>(
    operands: &[T],
    index: usize,
    label: &str,
) -> Result<i32, Diagnostic> {
    let Some(operand) = operands.get(index) else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: missing {label}"
        )));
    };
    let Some(value) = operand.immediate_integer() else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} is not an immediate integer"
        )));
    };
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} value {value} does not fit i32"
        ))
    })
}

pub(crate) fn disp32(value: usize) -> Result<i32, Diagnostic> {
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot address displacement `{value}`"
        ))
    })
}

pub(crate) fn rel32(value: isize) -> Result<i32, Diagnostic> {
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 branch target is out of rel32 range: {value} byte(s)"
        ))
    })
}

/// Emit `lock xadd [r14 + disp32], r10` at the given operand width. XADD swaps
/// then adds: it loads the prior `[mem]` into the source register (r10) and
/// stores `[mem] + r10` back, all as ONE atomic read-modify-write under the
/// LOCK prefix -- exactly `fetch_add`'s contract (r10 ends with the OLD value).
/// Caller sets r10 = the delta and r14 = the atomic field's base BEFORE this.
/// Used by `encode_atomic_fetch_add`; byte-verified by `atomic_tests` below.
pub(crate) fn append_lock_xadd_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    // F0 = LOCK. REX picks operand width (W) + r10 (R) + r14 (B). XADD is
    // `0F C1 /r` (or `0F C0 /r` for 8-bit). ModRM 0x96 = mod=10 (disp32),
    // reg=r10&7=2, r/m=r14&7=6.
    match byte_size {
        1 => bytes.extend([0xf0, 0x45, 0x0f, 0xc0, 0x96]),
        2 => bytes.extend([0xf0, 0x66, 0x45, 0x0f, 0xc1, 0x96]),
        4 => bytes.extend([0xf0, 0x45, 0x0f, 0xc1, 0x96]),
        8 => bytes.extend([0xf0, 0x4d, 0x0f, 0xc1, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 encoder cannot LOCK xadd {byte_size}-byte atomics yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Emitted byte count of [`append_lock_xadd_r10_to_r14`] (opcode block + disp32).
pub(crate) fn lock_xadd_r10_to_r14_width(byte_size: usize) -> usize {
    let opcode = match byte_size {
        1 | 4 => 5,
        2 => 6,
        8 => 5,
        _ => 5,
    };
    opcode + 4
}

/// Negate r10 at the atomic operand width. XADD then adds this truncated
/// two's-complement value, implementing wrapping fetch_sub while leaving the
/// prior memory value in r10.
pub(crate) fn append_negate_r10(bytes: &mut Vec<u8>, byte_size: usize) -> Result<(), Diagnostic> {
    match byte_size {
        1 => bytes.extend([0x41, 0xf6, 0xda]),
        2 => bytes.extend([0x66, 0x41, 0xf7, 0xda]),
        4 => bytes.extend([0x41, 0xf7, 0xda]),
        8 => bytes.extend([0x49, 0xf7, 0xda]),
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 encoder cannot negate a {other}-byte atomic operand"
            )));
        }
    }
    Ok(())
}

pub(crate) fn negate_r10_width(byte_size: usize) -> usize {
    match byte_size {
        2 => 4,
        1 | 4 | 8 => 3,
        _ => 3,
    }
}

/// `LOCK CMPXCHG [r14+disp32], r10`: compare rax with the place; if equal store
/// r10 (ZF=1), else load the place into rax (ZF=0). Identical layout to
/// `append_lock_xadd_r10_to_r14` but with the CMPXCHG opcode (`0F B1`, or
/// `0F B0` for 8-bit). Used by `encode_atomic_compare_exchange`; byte-verified
/// by `atomic_tests`.
pub(crate) fn append_lock_cmpxchg_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0xf0, 0x45, 0x0f, 0xb0, 0x96]),
        2 => bytes.extend([0xf0, 0x66, 0x45, 0x0f, 0xb1, 0x96]),
        4 => bytes.extend([0xf0, 0x45, 0x0f, 0xb1, 0x96]),
        8 => bytes.extend([0xf0, 0x4d, 0x0f, 0xb1, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 encoder cannot LOCK cmpxchg {byte_size}-byte atomics yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Emitted byte count of [`append_lock_cmpxchg_r10_to_r14`] (opcode + disp32).
/// Same layout as `lock_xadd_r10_to_r14_width` (only the opcode byte differs).
pub(crate) fn lock_cmpxchg_r10_to_r14_width(byte_size: usize) -> usize {
    lock_xadd_r10_to_r14_width(byte_size)
}

#[cfg(test)]
mod runtime_operand_load_tests {
    use super::*;

    #[test]
    fn rax_based_u16_operands_use_word_loads_in_width_lockstep() {
        for (register, prefix) in [
            (Reg64::R10, [0x66, 0x44, 0x8b, 0x90]),
            (Reg64::R11, [0x66, 0x44, 0x8b, 0x98]),
        ] {
            let mut bytes = Vec::new();
            append_load_reg_from_rax(&mut bytes, register, 24, 2)
                .expect("u16 pointee load should encode");
            assert_eq!(&bytes[..4], &prefix);
            assert_eq!(&bytes[4..], &24i32.to_le_bytes());
            assert_eq!(bytes.len(), load_width(2));
        }
    }
}
