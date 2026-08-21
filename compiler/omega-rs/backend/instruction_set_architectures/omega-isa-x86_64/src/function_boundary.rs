use super::{FUNCTION_FRAME_BYTES, append_mov_r15_imm64, disp32, x86_gpr_number};
use omega_calling_conventions::{IndirectPointerLocation, MachineRegister, RegisterSet};
use psi_diagnostics::Diagnostic;

pub const fn internal_function_call_width() -> usize {
    5
}

/// `call rel32` with a zero displacement owned by the object relocation.
pub fn encode_internal_function_call_bytes() -> [u8; 5] {
    [0xe8, 0, 0, 0, 0]
}

pub fn internal_function_call_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86Rsp])
}

pub fn internal_function_call_additional_machine_state()
-> omega_calling_conventions::MachineStateSet {
    omega_calling_conventions::MachineStateSet::new([
        omega_calling_conventions::MachineState::InstructionPointer,
        omega_calling_conventions::MachineState::StackPointer,
        omega_calling_conventions::MachineState::ControlState,
    ])
}

pub fn return_register_integer_write_width(
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
) -> usize {
    let high_register = x86_gpr_number(register).is_some_and(|number| number >= 8);
    if byte_size == 8 {
        10
    } else {
        5 + usize::from(high_register)
    }
}

pub fn runtime_storage_copy_to_return_register_width(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    // mov r15,imm64(region base, relocated) (10) + either a scalar XMM load
    // (9) or a GPR load (7; sign-extending 1/2-byte forms carry an 0F prefix,
    // 8).
    let _ = byte_offset;
    if matches!(
        register,
        omega_calling_conventions::MachineRegister::X86Xmm(_)
    ) {
        return 19;
    }
    let load_width = if matches!(byte_size, 1 | 2) { 8 } else { 7 };
    10 + load_width
}

/// Byte offset where the INDEX-region base's `mov r15, imm64` begins inside
/// `encode_runtime_machine_indexed_address_to_runtime_frame_write` (the
/// relocation machinery adds the +2 to reach its immediate).
pub const MACHINE_INDEXED_ADDRESS_INDEX_BASE_IMM_OFFSET: usize = 10;

pub fn entry_argument_register_write_width(
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
) -> usize {
    // mov r15,imm64(frame base, relocated at +2) (10), followed by either a
    // GPR store (7; 16-bit adds one prefix) or movss/movsd from XMM (9).
    match register {
        omega_calling_conventions::MachineRegister::X86Xmm(_) => 19,
        _ => 17 + usize::from(byte_size == 2),
    }
}

/// Registers overwritten by one inbound register-to-frame copy. Source
/// registers are reads, not clobbers, and therefore are deliberately absent.
pub fn entry_argument_register_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R15])
}

/// The ENTRY PROLOGUE's inbound unmarshal: store the exact GPR selected by the
/// normalized call plan into the entry parameter's runtime-frame slot. Runs
/// before anything else at the entry because argument registers are volatile.
pub fn encode_entry_argument_register_write_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if register == MachineRegister::X86R15 {
        return Err(Diagnostic::error(
            "x86-64 entry prologue cannot read an argument from its r15 frame-base scratch",
        ));
    }
    if let omega_calling_conventions::MachineRegister::X86Xmm(register_index) = register {
        if register_index > 15 || !matches!(byte_size, 4 | 8) {
            return Err(Diagnostic::error(format!(
                "x86-64 entry prologue cannot store {byte_size} bytes from XMM{register_index}"
            )));
        }
        let mut bytes =
            Vec::with_capacity(entry_argument_register_write_width(register, byte_size));
        append_mov_r15_imm64(&mut bytes, 0);
        // movss/movsd [r15+disp32], xmmN. The mandatory F3/F2 prefix precedes
        // REX; B names r15 and R extends XMM8..15.
        bytes.push(if byte_size == 4 { 0xf3 } else { 0xf2 });
        bytes.push(0x41 | if register_index >= 8 { 0x04 } else { 0 });
        bytes.extend([0x0f, 0x11, 0x87 | ((register_index & 7) << 3)]);
        bytes.extend(disp32(byte_offset)?.to_le_bytes());
        debug_assert_eq!(
            bytes.len(),
            entry_argument_register_write_width(register, byte_size)
        );
        return Ok(bytes);
    }
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "x86-64 entry prologue cannot store a {byte_size}-byte register value"
        )));
    }
    let mut bytes = Vec::with_capacity(entry_argument_register_write_width(register, byte_size));
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the runtime-frame region base
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "x86-64 entry prologue cannot store non-GPR plan location {register:?}"
        ))
    })?;
    if byte_size == 2 {
        bytes.push(0x66);
    }
    let rex =
        (if byte_size == 8 { 0x49 } else { 0x41 }) | if register_number >= 8 { 0x04 } else { 0 };
    let modrm = 0x87 | ((register_number & 7) << 3);
    bytes.extend([rex, if byte_size == 1 { 0x88 } else { 0x89 }, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        entry_argument_register_write_width(register, byte_size)
    );
    Ok(bytes)
}

/// Width of one incoming stack-fragment copy. The runtime-frame base
/// materialization is 10 bytes; byte/dword/qword loads and stores total 15,
/// while the operand-size prefixes make the word form two bytes longer.
pub fn entry_stack_argument_write_width(byte_size: usize) -> usize {
    if byte_size == 2 { 27 } else { 25 }
}

/// Registers overwritten while copying one incoming stack fragment. `rsp` is
/// only used as an address base and is not clobbered by this fragment.
pub fn entry_stack_argument_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R15])
}

/// Copy an incoming x86-64 stack argument into runtime-frame storage. Calling
/// plans measure `stack_byte_offset` from the ABI stack-argument area, so the
/// source is beyond the fixed saved-register frame and return address.
pub fn encode_entry_stack_argument_write_bytes(
    stack_byte_offset: u32,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "x86-64 entry prologue cannot copy a {byte_size}-byte stack value"
        )));
    }
    let source_offset = stack_byte_offset
        .checked_add((FUNCTION_FRAME_BYTES + 8) as u32)
        .ok_or_else(|| Diagnostic::error("x86-64 incoming stack offset overflow"))?;
    let source_offset = i32::try_from(source_offset)
        .map_err(|_| Diagnostic::error("x86-64 incoming stack offset exceeds disp32"))?;
    let mut bytes = Vec::with_capacity(entry_stack_argument_write_width(byte_size));
    append_mov_r15_imm64(&mut bytes, 0); // runtime-frame base, relocated

    // mov r10{b,w,d,q}, [rsp + disp32]
    if byte_size == 2 {
        bytes.push(0x66);
    }
    bytes.extend([
        if byte_size == 8 { 0x4c } else { 0x44 },
        if byte_size == 1 { 0x8a } else { 0x8b },
        0x94,
        0x24,
    ]);
    bytes.extend(source_offset.to_le_bytes());

    // mov [r15 + disp32], r10{b,w,d,q}
    if byte_size == 2 {
        bytes.push(0x66);
    }
    bytes.extend([
        if byte_size == 8 { 0x4d } else { 0x45 },
        if byte_size == 1 { 0x88 } else { 0x89 },
        0x97,
    ]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(bytes.len(), entry_stack_argument_write_width(byte_size));
    Ok(bytes)
}

pub fn entry_indirect_argument_write_width(
    pointer: IndirectPointerLocation,
    byte_size: usize,
) -> usize {
    let pointer_setup = match pointer {
        IndirectPointerLocation::Register(_) => 3,
        IndirectPointerLocation::Stack { .. } => 8,
    };
    let mut width = pointer_setup + 10;
    let mut copied = 0usize;
    while copied < byte_size {
        let fragment = [8, 4, 2, 1]
            .into_iter()
            .find(|fragment| byte_size - copied >= *fragment)
            .expect("indirect entry copy has bytes remaining");
        width += 14 + usize::from(fragment == 2) * 2;
        copied += fragment;
    }
    width
}

pub fn entry_indirect_argument_frame_base_offset(pointer: IndirectPointerLocation) -> usize {
    match pointer {
        IndirectPointerLocation::Register(_) => 3,
        IndirectPointerLocation::Stack { .. } => 8,
    }
}

/// Registers overwritten while copying one indirectly passed aggregate.
pub fn entry_indirect_argument_write_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R15,
    ])
}

/// Copy one indirectly passed Microsoft x64 aggregate into its runtime-frame
/// slot. The ABI pointer occupies the argument's positional GPR or stack slot;
/// r11 preserves it while r10 moves one naturally sized fragment at a time.
pub fn encode_entry_indirect_argument_write_bytes(
    pointer: IndirectPointerLocation,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if matches!(byte_size, 0 | 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(
            "Microsoft x64 indirect entry aggregate must have a nondirect record width",
        ));
    }
    let mut bytes = Vec::with_capacity(entry_indirect_argument_write_width(pointer, byte_size));
    match pointer {
        IndirectPointerLocation::Register(
            register @ (MachineRegister::X86Rcx
            | MachineRegister::X86Rdx
            | MachineRegister::X86R8
            | MachineRegister::X86R9),
        ) => {
            let register_number = x86_gpr_number(register).expect("matched x86 GPR");
            bytes.extend([
                0x4c | u8::from(register_number >= 8),
                0x8b,
                0xd8 | (register_number & 7),
            ]); // mov r11, selected pointer register
        }
        IndirectPointerLocation::Register(register) => {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 indirect entry aggregate uses unsupported pointer register {register:?}"
            )));
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            let source_offset = stack_byte_offset
                .checked_add((FUNCTION_FRAME_BYTES + 8) as u32)
                .ok_or_else(|| Diagnostic::error("x86-64 incoming pointer offset overflow"))?;
            let source_offset = i32::try_from(source_offset)
                .map_err(|_| Diagnostic::error("x86-64 incoming pointer offset exceeds disp32"))?;
            bytes.extend([0x4c, 0x8b, 0x9c, 0x24]); // mov r11, [rsp+disp32]
            bytes.extend(source_offset.to_le_bytes());
        }
    }
    append_mov_r15_imm64(&mut bytes, 0); // runtime-frame base, relocated
    let mut copied = 0usize;
    while copied < byte_size {
        let fragment = [8, 4, 2, 1]
            .into_iter()
            .find(|fragment| byte_size - copied >= *fragment)
            .expect("indirect entry copy has bytes remaining");
        if fragment == 2 {
            bytes.push(0x66);
        }
        bytes.extend([
            if fragment == 8 { 0x4d } else { 0x45 },
            if fragment == 1 { 0x8a } else { 0x8b },
            0x93,
        ]); // mov r10{b,w,d,q}, [r11+disp32]
        bytes.extend(disp32(copied)?.to_le_bytes());
        if fragment == 2 {
            bytes.push(0x66);
        }
        bytes.extend([
            if fragment == 8 { 0x4d } else { 0x45 },
            if fragment == 1 { 0x88 } else { 0x89 },
            0x97,
        ]); // mov [r15+disp32], r10{b,w,d,q}
        bytes.extend(
            disp32(byte_offset.checked_add(copied).ok_or_else(|| {
                Diagnostic::error("x86-64 indirect entry destination offset overflow")
            })?)?
            .to_le_bytes(),
        );
        copied += fragment;
    }
    debug_assert_eq!(
        bytes.len(),
        entry_indirect_argument_write_width(pointer, byte_size)
    );
    Ok(bytes)
}

#[cfg(test)]
mod entry_argument_register_tests {
    use super::*;
    use crate::{
        encode_function_enter_bytes, encode_return_bytes, function_enter_additional_machine_state,
        function_enter_width, return_additional_machine_state, return_width,
    };
    use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet};

    #[test]
    fn ordinary_frame_preserves_generated_nonvolatile_gprs_and_alignment() {
        assert_eq!(FUNCTION_FRAME_BYTES, 80);
        assert_eq!(encode_function_enter_bytes().len(), function_enter_width());
        assert_eq!(encode_return_bytes().len(), return_width());
        assert_eq!(
            encode_function_enter_bytes(),
            [
                0x53, 0x55, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x83,
                0xec, 0x10, 0x0f, 0xae, 0x1c, 0x24, 0xc7, 0x44, 0x24, 0x04, 0x80, 0x1f, 0x00, 0x00,
                0x0f, 0xae, 0x54, 0x24, 0x04,
            ]
        );
        assert_eq!(
            encode_return_bytes(),
            [
                0x0f, 0xae, 0x14, 0x24, 0x48, 0x83, 0xc4, 0x10, 0x41, 0x5f, 0x41, 0x5e, 0x41, 0x5d,
                0x41, 0x5c, 0x5f, 0x5e, 0x5d, 0x5b, 0xc3,
            ]
        );
        assert!(
            function_enter_additional_machine_state()
                .contains_all(MachineStateSet::new([MachineState::ControlState]))
        );
        assert!(
            return_additional_machine_state()
                .contains_all(MachineStateSet::new([MachineState::ControlState]))
        );
    }

    #[test]
    fn scalar_float_entry_arguments_store_from_the_selected_xmm_register() {
        let bytes = encode_entry_argument_register_write_bytes(MachineRegister::X86Xmm(8), 8, 8)
            .expect("movsd entry store");
        assert_eq!(bytes.len(), 19);
        assert_eq!(&bytes[10..15], &[0xf2, 0x45, 0x0f, 0x11, 0x87]);
        assert_eq!(&bytes[15..19], &8i32.to_le_bytes());
    }

    #[test]
    fn scalar_float_entry_arguments_reject_non_scalar_widths() {
        let error = encode_entry_argument_register_write_bytes(MachineRegister::X86Xmm(0), 0, 16)
            .expect_err("unclassified vector argument must reject");
        assert!(error.message.contains("cannot store 16 bytes"));
    }

    #[test]
    fn entry_argument_cannot_alias_the_frame_base_scratch() {
        let error = encode_entry_argument_register_write_bytes(MachineRegister::X86R15, 0, 8)
            .expect_err("r15 input would be destroyed while materializing the frame base");
        assert!(error.message.contains("r15 frame-base scratch"));
    }

    #[test]
    fn ms_x64_fifth_argument_loads_after_return_address_and_shadow_space() {
        let bytes =
            encode_entry_stack_argument_write_bytes(32, 24, 8).expect("incoming stack copy");
        assert_eq!(bytes.len(), 25);
        assert_eq!(&bytes[10..18], &[0x4c, 0x8b, 0x94, 0x24, 120, 0, 0, 0]);
        assert_eq!(&bytes[18..25], &[0x4d, 0x89, 0x97, 24, 0, 0, 0]);
    }

    #[test]
    fn indirect_entry_aggregate_copies_from_a_pointer_register() {
        let pointer = IndirectPointerLocation::Register(MachineRegister::X86Rcx);
        let bytes = encode_entry_indirect_argument_write_bytes(pointer, 64, 16)
            .expect("two-fragment indirect entry copy");

        assert_eq!(
            bytes.len(),
            entry_indirect_argument_write_width(pointer, 16)
        );
        assert_eq!(entry_indirect_argument_frame_base_offset(pointer), 3);
        assert_eq!(&bytes[..3], &[0x4c, 0x8b, 0xd9]);
        assert_eq!(&bytes[13..20], &[0x4d, 0x8b, 0x93, 0, 0, 0, 0]);
        assert_eq!(&bytes[20..27], &[0x4d, 0x89, 0x97, 64, 0, 0, 0]);
        assert_eq!(&bytes[27..34], &[0x4d, 0x8b, 0x93, 8, 0, 0, 0]);
        assert_eq!(&bytes[34..41], &[0x4d, 0x89, 0x97, 72, 0, 0, 0]);
    }

    #[test]
    fn indirect_entry_aggregate_loads_a_stack_passed_pointer() {
        let pointer = IndirectPointerLocation::Stack {
            stack_byte_offset: 32,
            alignment: 8,
        };
        let bytes = encode_entry_indirect_argument_write_bytes(pointer, 64, 16)
            .expect("stack-pointer indirect entry copy");

        assert_eq!(
            bytes.len(),
            entry_indirect_argument_write_width(pointer, 16)
        );
        assert_eq!(entry_indirect_argument_frame_base_offset(pointer), 8);
        assert_eq!(&bytes[..8], &[0x4c, 0x8b, 0x9c, 0x24, 120, 0, 0, 0]);
        assert_eq!(&bytes[8..10], &[0x49, 0xbf]);
    }
}

pub fn entry_arguments_slice_descriptor_write_width() -> usize {
    // mov r15,imm64(frame base, relocated at +2) (10) + lea rax,[r15+spill] (7)
    // + mov [r15+desc],rax (7) + mov qword [r15+desc+8],imm32(len) (11).
    35
}

/// Exact register footprint of the bytes-handoff descriptor encoder below.
/// Keep this beside the implementation so certificate derivation cannot drift
/// from its fixed frame-base and descriptor-address scratch choices.
pub fn entry_arguments_slice_descriptor_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86R15])
}

/// The bytes-handoff half of the entry prologue: bind `args: &[u8]` as a view
/// over the entry-argument spill -- write the slice descriptor
/// {ptr @ desc+0 = frame+spill_offset, len @ desc+8 = byte_length}.
pub fn encode_entry_arguments_slice_descriptor_write_bytes(
    descriptor_offset: usize,
    spill_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(entry_arguments_slice_descriptor_write_width());
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the runtime-frame region base
    bytes.extend([0x49, 0x8d, 0x87]); // lea rax, [r15 + disp32]
    bytes.extend(disp32(spill_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x89, 0x87]); // mov [r15 + disp32], rax
    bytes.extend(disp32(descriptor_offset)?.to_le_bytes());
    bytes.extend([0x49, 0xc7, 0x87]); // mov qword [r15 + disp32], imm32
    bytes.extend(disp32(descriptor_offset + 8)?.to_le_bytes());
    let length = i32::try_from(byte_length)
        .map_err(|_| Diagnostic::error("entry-argument slice length exceeds an imm32"))?;
    bytes.extend(length.to_le_bytes());
    debug_assert_eq!(bytes.len(), entry_arguments_slice_descriptor_write_width());
    Ok(bytes)
}

pub fn encode_return_register_integer_write_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot write {byte_size}-byte return integers yet"
        )));
    }
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot use {register:?} as an integer result register"
        ))
    })?;
    let mut bytes = Vec::with_capacity(return_register_integer_write_width(register, byte_size));
    if byte_size == 8 {
        bytes.extend([
            0x48 | u8::from(register_number >= 8),
            0xb8 + (register_number & 7),
        ]);
        bytes.extend(value.to_le_bytes());
    } else {
        let value = i32::try_from(value).map_err(|_| {
            Diagnostic::error(format!(
                "X86_64 MVP encoder cannot write return integer `{value}` yet"
            ))
        })?;
        if register_number >= 8 {
            bytes.push(0x41);
        }
        bytes.push(0xb8 + (register_number & 7));
        bytes.extend(value.to_le_bytes());
    }
    debug_assert_eq!(
        bytes.len(),
        return_register_integer_write_width(register, byte_size)
    );
    Ok(bytes)
}

/// Exact register footprint of immediate result materialization.
pub fn return_register_integer_write_clobbers(register: MachineRegister) -> RegisterSet {
    RegisterSet::new([register])
}

/// Load a runtime-storage scalar into the plan-selected integer result register so a
/// NON-CONSTANT terminal value (a local read, a field read-back) becomes the
/// process exit code. The `mov r15, imm64=0` (imm at instruction start + 2) is
/// relocated to the storage region's data symbol by the relocation planner,
/// exactly like a dispatch guard's storage load. Narrow operands use the
/// sign-extending movsx forms so a negative i8/i16 terminal survives the
/// widening read.
pub fn encode_runtime_storage_copy_to_return_register_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_to_return_register_width(
        register,
        byte_offset,
        byte_size,
    ));
    append_mov_r15_imm64(&mut bytes, 0);
    let displacement = disp32(byte_offset)?;
    if let omega_calling_conventions::MachineRegister::X86Xmm(register_index) = register {
        if register_index > 15 || !matches!(byte_size, 4 | 8) {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot copy {byte_size} bytes into XMM{register_index}"
            )));
        }
        bytes.push(if byte_size == 4 { 0xf3 } else { 0xf2 });
        bytes.push(0x41 | if register_index >= 8 { 0x04 } else { 0 });
        bytes.extend([0x0f, 0x10, 0x87 | ((register_index & 7) << 3)]);
        bytes.extend(displacement.to_le_bytes());
        debug_assert_eq!(
            bytes.len(),
            runtime_storage_copy_to_return_register_width(register, byte_offset, byte_size)
        );
        return Ok(bytes);
    }
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot use {register:?} as an integer result register"
        ))
    })?;
    let modrm = 0x87 | ((register_number & 7) << 3);
    let rex_r = if register_number >= 8 { 0x04 } else { 0 };
    match byte_size {
        1 => bytes.extend([0x41 | rex_r, 0x0f, 0xbe, modrm]),
        2 => bytes.extend([0x41 | rex_r, 0x0f, 0xbf, modrm]),
        4 => bytes.extend([0x41 | rex_r, 0x8b, modrm]),
        8 => bytes.extend([0x49 | rex_r, 0x8b, modrm]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot copy {byte_size}-byte terminal values to the return register yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_to_return_register_width(register, byte_offset, byte_size)
    );
    Ok(bytes)
}

/// Exact register footprint of the runtime-frame result load above.
pub fn runtime_storage_copy_to_return_register_clobbers(register: MachineRegister) -> RegisterSet {
    RegisterSet::new([register, MachineRegister::X86R15])
}

#[cfg(test)]
mod result_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn constant_result_uses_the_plan_selected_high_gpr() {
        let bytes = encode_return_register_integer_write_bytes(MachineRegister::X86R9, 4, 7)
            .expect("r9d result write");
        assert_eq!(bytes, [0x41, 0xb9, 7, 0, 0, 0]);
    }

    #[test]
    fn constant_result_accepts_the_normalized_u16_width() {
        let bytes = encode_return_register_integer_write_bytes(MachineRegister::X86Rax, 2, 7)
            .expect("ax result represented through eax");
        assert_eq!(bytes, [0xb8, 7, 0, 0, 0]);
    }

    #[test]
    fn runtime_result_load_uses_the_plan_selected_high_gpr() {
        let bytes =
            encode_runtime_storage_copy_to_return_register_bytes(MachineRegister::X86R10, 16, 4)
                .expect("r10d result load");
        assert_eq!(&bytes[10..17], &[0x45, 0x8b, 0x97, 16, 0, 0, 0]);
    }

    #[test]
    fn runtime_result_load_uses_the_plan_selected_xmm_register() {
        let bytes =
            encode_runtime_storage_copy_to_return_register_bytes(MachineRegister::X86Xmm(2), 24, 8)
                .expect("xmm2 result load");
        assert_eq!(&bytes[10..19], &[0xf2, 0x41, 0x0f, 0x10, 0x97, 24, 0, 0, 0]);
    }
}
