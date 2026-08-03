use omega_calling_conventions::{
    IndirectPointerLocation, MachineRegister, MachineState, MachineStateSet, RegisterSet,
};
use omega_target_operations::StateGuardOperator;
use psi_diagnostics::Diagnostic;

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate_padded, append_unsigned_immediate_w_padded,
    encode_add_page_offset_placeholder, encode_add_x_immediate, encode_adrp_placeholder,
    encode_compare_w_immediate, encode_compare_w_register, encode_compare_x_register,
    encode_conditional_branch_equal, encode_conditional_branch_greater,
    encode_conditional_branch_greater_or_equal, encode_conditional_branch_higher,
    encode_conditional_branch_higher_or_same, encode_conditional_branch_less,
    encode_conditional_branch_less_or_equal, encode_conditional_branch_lower,
    encode_conditional_branch_lower_or_same, encode_conditional_branch_not_equal,
    encode_conditional_branch_plus, encode_float_compare, encode_float_move_from_gpr,
    encode_instruction, encode_load_w_from_x, encode_load_x_from_x, encode_move_x_register,
    encode_movz_w, encode_sign_extend_byte_to_w, encode_sign_extend_halfword_to_w,
    encode_store_w_to_x, encode_store_x_to_x, encode_unconditional_branch,
};
use super::widths::dispatch_guard_compare_static_width;

const DISPATCH_STATE_REGISTER: u8 = 28;

pub fn encode_dispatch_loop_enter_bytes(entry_dispatch_index: u32) -> Result<[u8; 4], Diagnostic> {
    let immediate = u16::try_from(entry_dispatch_index).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode dispatch index `{entry_dispatch_index}` yet"
        ))
    })?;
    Ok(encode_movz_w(DISPATCH_STATE_REGISTER, immediate))
}

pub fn encode_dispatch_case_enter_bytes(
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<[u8; 8], Diagnostic> {
    Ok(two_instructions(
        encode_compare_w_immediate(DISPATCH_STATE_REGISTER, dispatch_index)?,
        encode_conditional_branch_not_equal(skip_byte_distance)?,
    ))
}

pub fn encode_dispatch_state_write_bytes(
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<[u8; 8], Diagnostic> {
    let immediate = u16::try_from(dispatch_index).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode dispatch index `{dispatch_index}` yet"
        ))
    })?;
    Ok(two_instructions(
        encode_movz_w(DISPATCH_STATE_REGISTER, immediate),
        encode_unconditional_branch(case_leave_byte_distance)?,
    ))
}

pub fn encode_dispatch_case_leave_bytes(loop_byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_unconditional_branch(loop_byte_distance)
}

pub fn dispatch_loop_enter_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(DISPATCH_STATE_REGISTER)])
}

pub fn dispatch_case_enter_register_writes() -> RegisterSet {
    RegisterSet::default()
}

pub fn dispatch_case_enter_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn dispatch_state_write_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(DISPATCH_STATE_REGISTER)])
}

pub fn dispatch_case_leave_register_writes() -> RegisterSet {
    RegisterSet::default()
}

/// Narrow a guard's float `expected_value` (stored as f64 bits) to the operand's
/// width: for a 4-byte float operand the comparison runs in single precision, so
/// the materialized bits must be the f32 bit pattern. Exact for any value
/// representable in f32 (which a constant compared against an f32 field always
/// is). Mirrors the x86_64 backend's `float_compare_expected_bits`.
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
    let mut bytes = Vec::with_capacity(dispatch_guard_compare_static_width(
        byte_offset,
        byte_size,
        is_float,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if is_float {
        if !matches!(byte_size, 4 | 8) {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte float dispatch guards yet"
            )));
        }
        // Load the field's raw float bits, materialize the expected bits
        // (narrowed to f32 for 4-byte operands), move both into the FP bank and
        // FCMP. The NZCV flags then drive the same equal/unsigned skip branches
        // as the x86 `ucomis*` path (NaN handling is a documented first-cut
        // limitation there too).
        append_guard_load(&mut bytes, byte_offset, byte_size)?;
        let expected_bits = float_compare_expected_bits(expected_value, byte_size);
        if byte_size == 4 {
            append_unsigned_immediate_w_padded(&mut bytes, 26, expected_bits as u32);
        } else {
            append_unsigned_immediate_padded(&mut bytes, 26, expected_bits);
        }
        bytes.extend(encode_float_move_from_gpr(byte_size, 0, 17)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 1, 26)?);
        bytes.extend(encode_float_compare(byte_size, 0, 1)?);
    } else {
        // The expected value is materialized into a register (fixed-width, so
        // the width function stays magnitude-independent) and compared register
        // to register, which handles negative values and full-width patterns
        // that do not fit the 12-bit compare immediate. Narrow operands are
        // sign-extended on BOTH sides: exact for signed conditions and order-
        // preserving for unsigned ones (sign extension is monotone per width).
        match byte_size {
            1 => {
                append_guard_load(&mut bytes, byte_offset, byte_size)?;
                bytes.extend(encode_sign_extend_byte_to_w(17, 17));
                append_unsigned_immediate_w_padded(
                    &mut bytes,
                    26,
                    expected_value as i8 as i32 as u32,
                );
                bytes.extend(encode_compare_w_register(17, 26));
            }
            2 => {
                append_guard_load(&mut bytes, byte_offset, byte_size)?;
                bytes.extend(encode_sign_extend_halfword_to_w(17, 17));
                append_unsigned_immediate_w_padded(
                    &mut bytes,
                    26,
                    expected_value as i16 as i32 as u32,
                );
                bytes.extend(encode_compare_w_register(17, 26));
            }
            4 => {
                append_guard_load(&mut bytes, byte_offset, byte_size)?;
                append_unsigned_immediate_w_padded(&mut bytes, 26, expected_value as u32);
                bytes.extend(encode_compare_w_register(17, 26));
            }
            8 => {
                append_guard_load(&mut bytes, byte_offset, byte_size)?;
                append_unsigned_immediate_padded(&mut bytes, 26, expected_value as u64);
                bytes.extend(encode_compare_x_register(17, 26));
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
                )));
            }
        }
    }
    bytes.extend(match operator {
        StateGuardOperator::Equal => encode_conditional_branch_not_equal(skip_byte_distance)?,
        StateGuardOperator::NotEqual => encode_conditional_branch_equal(skip_byte_distance)?,
        StateGuardOperator::Greater => encode_conditional_branch_less_or_equal(skip_byte_distance)?,
        StateGuardOperator::GreaterOrEqual => encode_conditional_branch_less(skip_byte_distance)?,
        // Float `<`/`<=` skip on PL/HI, true on unordered (IEEE: comparisons
        // with NaN are false) -- see encode_conditional_branch_for_operator_bytes.
        StateGuardOperator::Less if is_float => encode_conditional_branch_plus(skip_byte_distance)?,
        StateGuardOperator::LessOrEqual if is_float => {
            encode_conditional_branch_higher(skip_byte_distance)?
        }
        StateGuardOperator::Less => encode_conditional_branch_greater_or_equal(skip_byte_distance)?,
        StateGuardOperator::LessOrEqual => encode_conditional_branch_greater(skip_byte_distance)?,
        // Unsigned comparisons skip on the negated UNSIGNED condition (cf. the x86
        // jcc skip-branches: LessUnsigned->jae, GreaterUnsigned->jbe, etc.).
        StateGuardOperator::LessUnsigned => {
            encode_conditional_branch_higher_or_same(skip_byte_distance)?
        }
        // Float `>`/`>=` skip on the SIGNED complements LE/LT: the unsigned
        // skips LS/LO are false on unordered (FCMP sets C+V on NaN).
        StateGuardOperator::GreaterUnsigned if is_float => {
            encode_conditional_branch_less_or_equal(skip_byte_distance)?
        }
        StateGuardOperator::GreaterOrEqualUnsigned if is_float => {
            encode_conditional_branch_less(skip_byte_distance)?
        }
        StateGuardOperator::GreaterUnsigned => {
            encode_conditional_branch_lower_or_same(skip_byte_distance)?
        }
        StateGuardOperator::LessOrEqualUnsigned => {
            encode_conditional_branch_higher(skip_byte_distance)?
        }
        StateGuardOperator::GreaterOrEqualUnsigned => {
            encode_conditional_branch_lower(skip_byte_distance)?
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot lower dispatch guard operator `{operator:?}` yet"
            )));
        }
    });
    debug_assert_eq!(
        bytes.len(),
        dispatch_guard_compare_static_width(byte_offset, byte_size, is_float)
    );
    Ok(bytes)
}

/// Exact registers overwritten by a storage-backed static dispatch guard.
/// Large/unscaled offsets reuse x26 as the adjusted base before it is replaced
/// by the expected value, so the set is independent of offset magnitude.
pub fn dispatch_guard_compare_static_register_writes(is_float: bool) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(26),
    ];
    if is_float {
        registers.extend([MachineRegister::Aarch64V(0), MachineRegister::Aarch64V(1)]);
    }
    RegisterSet::new(registers)
}

pub fn dispatch_guard_compare_static_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

fn append_guard_load(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let direct = match byte_size {
        1 => byte_offset <= 4095,
        2 => byte_offset.is_multiple_of(2) && byte_offset / 2 <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    };

    let base_register = if direct {
        16
    } else {
        bytes.extend(encode_move_x_register(26, 16));
        append_add_x_constant(bytes, 26, 26, byte_offset, 19)?;
        26
    };

    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
            17,
            base_register,
            if direct { byte_offset } else { 0 },
            byte_size,
        )?),
        8 => bytes.extend(encode_load_x_from_x(
            17,
            base_register,
            if direct { byte_offset } else { 0 },
        )?),
        _ => unreachable!("dispatch guard byte_size was validated by caller"),
    }

    Ok(())
}

/// Load a runtime-storage scalar into the return register (w0/x0) so a
/// NON-CONSTANT terminal value (a local read, a field read-back) becomes the
/// process exit code. The leading adrp+add pair is relocated to the storage
/// region's data symbol by the relocation planner, exactly like a dispatch
/// guard's storage load. Narrow operands are sign-extended so a negative
/// i8/i16 terminal survives the widening read.
/// Entry prologue: store the process-entry argument register into its runtime
/// frame slot (`adrp`+`add` x16 = the frame base, relocated at instruction
/// start by the arch-agnostic record, then `str x<index>, [x16, offset]`).
/// AAPCS64 passes the first eight arguments in x0..x7; the prologue runs before
/// anything clobbers them (the function-enter stp prologue touches only
/// callee-saved pairs).
pub fn encode_entry_argument_register_write_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if register == MachineRegister::Aarch64X(16) {
        return Err(Diagnostic::error(
            "AArch64 entry prologue cannot read an argument from its x16 frame-base scratch",
        ));
    }
    let (register_index, is_vector) = match register {
        omega_calling_conventions::MachineRegister::Aarch64X(index) => (index, false),
        omega_calling_conventions::MachineRegister::Aarch64V(index) => (index, true),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 entry prologue cannot store foreign plan location {register:?}"
            )));
        }
    };
    if register_index > if is_vector { 31 } else { 30 } {
        return Err(Diagnostic::error(format!(
            "AArch64 entry prologue register {register:?} is outside the encodable set"
        )));
    }
    let mut bytes = Vec::with_capacity(super::widths::entry_argument_register_write_width());
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if is_vector {
        bytes.extend(encode_entry_vector_store(
            register_index,
            16,
            byte_offset,
            byte_size,
        )?);
    } else {
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_store_w_to_x(
                register_index,
                16,
                byte_offset,
                byte_size,
            )?),
            8 => bytes.extend(encode_store_x_to_x(register_index, 16, byte_offset)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 entry prologue cannot store a {byte_size}-byte register value"
                )));
            }
        }
    }
    debug_assert_eq!(
        bytes.len(),
        super::widths::entry_argument_register_write_width()
    );
    Ok(bytes)
}

/// Registers overwritten by one inbound register-to-frame copy. The selected
/// argument register is read rather than clobbered.
pub fn entry_argument_register_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16)])
}

/// Copy one AAPCS64 incoming stack fragment into runtime-frame storage. The
/// normalized offset is relative to the caller's SP; the ordinary function
/// prologue has already reserved `FUNCTION_FRAME_BYTES` beneath it.
pub fn encode_entry_stack_argument_write_bytes(
    stack_byte_offset: u32,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 entry prologue cannot copy a {byte_size}-byte stack value"
        )));
    }
    let source_offset = usize::try_from(stack_byte_offset)
        .ok()
        .and_then(|offset| offset.checked_add(super::FUNCTION_FRAME_BYTES))
        .ok_or_else(|| Diagnostic::error("AArch64 incoming stack offset overflow"))?;
    let mut bytes = Vec::with_capacity(super::widths::entry_stack_argument_write_width());
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_load_w_from_x(17, 31, source_offset, byte_size)?),
        8 => bytes.extend(encode_load_x_from_x(17, 31, source_offset)?),
        _ => unreachable!("stack-copy width validated above"),
    }
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(17, 16, byte_offset, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(17, 16, byte_offset)?),
        _ => unreachable!("stack-copy width validated above"),
    }
    debug_assert_eq!(
        bytes.len(),
        super::widths::entry_stack_argument_write_width()
    );
    Ok(bytes)
}

/// Registers overwritten while copying one incoming stack fragment. The
/// architectural stack pointer is only read as an address base.
pub fn entry_stack_argument_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)])
}

/// Copy one indirectly passed AAPCS64 aggregate into its runtime-frame slot.
/// The pointer may arrive in x0..x7 or in the incoming stack-argument area.
pub fn encode_entry_indirect_argument_write_bytes(
    pointer: IndirectPointerLocation,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if byte_size <= 16 {
        return Err(Diagnostic::error(
            "AArch64 indirect entry aggregate must be larger than 16 bytes",
        ));
    }
    let mut bytes = Vec::with_capacity(super::widths::entry_indirect_argument_write_width(
        pointer,
        byte_offset,
        byte_size,
    ));
    let source_register = match pointer {
        IndirectPointerLocation::Register(MachineRegister::Aarch64X(register @ 0..=7)) => register,
        IndirectPointerLocation::Register(register) => {
            return Err(Diagnostic::error(format!(
                "AArch64 indirect entry aggregate uses unsupported pointer register {register:?}"
            )));
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            let source_offset = usize::try_from(stack_byte_offset)
                .ok()
                .and_then(|offset| offset.checked_add(super::FUNCTION_FRAME_BYTES))
                .ok_or_else(|| Diagnostic::error("AArch64 incoming pointer offset overflow"))?;
            super::append_load_data_from_x_offset(&mut bytes, 17, 31, source_offset, 8, 9)?;
            17
        }
    };
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    let data_register = if source_register == 17 { 10 } else { 17 };
    let mut copied = 0usize;
    while copied < byte_size {
        let fragment = [8, 4, 2, 1]
            .into_iter()
            .find(|fragment| byte_size - copied >= *fragment)
            .expect("indirect entry copy has bytes remaining");
        super::append_load_data_from_x_offset(
            &mut bytes,
            data_register,
            source_register,
            copied,
            fragment,
            9,
        )?;
        super::append_store_data_to_x_offset(
            &mut bytes,
            data_register,
            16,
            byte_offset
                .checked_add(copied)
                .ok_or_else(|| Diagnostic::error("AArch64 entry destination offset overflow"))?,
            fragment,
            9,
        )?;
        copied += fragment;
    }
    debug_assert_eq!(
        bytes.len(),
        super::widths::entry_indirect_argument_write_width(pointer, byte_offset, byte_size)
    );
    Ok(bytes)
}

/// Registers overwritten while copying one indirectly passed aggregate. A
/// stack-carried pointer occupies x17, so x10 becomes the fragment scratch;
/// a register-carried pointer leaves x17 available for fragment data.
pub fn entry_indirect_argument_write_clobbers(pointer: IndirectPointerLocation) -> RegisterSet {
    match pointer {
        IndirectPointerLocation::Register(_) => {
            RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)])
        }
        IndirectPointerLocation::Stack { .. } => RegisterSet::new([
            MachineRegister::Aarch64X(10),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
        ]),
    }
}

fn encode_entry_vector_store(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<[u8; 4], Diagnostic> {
    let (opcode, scale) = match byte_size {
        4 => (0xbd00_0000, 4), // str sN, [xBase, #offset]
        8 => (0xfd00_0000, 8), // str dN, [xBase, #offset]
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 entry prologue cannot store {byte_size} bytes from v{source_register}"
            )));
        }
    };
    if !byte_offset.is_multiple_of(scale) || byte_offset / scale > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 entry prologue cannot store v{source_register} at offset `{byte_offset}`"
        )));
    }
    Ok(encode_instruction(
        opcode
            | (((byte_offset / scale) as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

#[cfg(test)]
mod entry_argument_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn scalar_float_entry_arguments_store_from_the_selected_v_register() {
        let bytes = encode_entry_argument_register_write_bytes(MachineRegister::Aarch64V(3), 8, 8)
            .expect("str d3 entry store");
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[8..12], &0xfd00_0603u32.to_le_bytes());
    }

    #[test]
    fn scalar_float_entry_arguments_reject_non_scalar_widths() {
        let error = encode_entry_argument_register_write_bytes(MachineRegister::Aarch64V(0), 0, 16)
            .expect_err("unclassified vector argument must reject");
        assert!(error.message.contains("cannot store 16 bytes"));
    }

    #[test]
    fn entry_argument_cannot_alias_the_frame_base_scratch() {
        let error = encode_entry_argument_register_write_bytes(MachineRegister::Aarch64X(16), 0, 8)
            .expect_err("x16 input would be destroyed while materializing the frame base");
        assert!(error.message.contains("x16 frame-base scratch"));
    }

    #[test]
    fn ninth_aapcs64_argument_loads_above_the_function_frame() {
        let bytes = encode_entry_stack_argument_write_bytes(0, 0, 8).expect("incoming stack copy");
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[8..12], &0xf940_3bf1u32.to_le_bytes());
        assert_eq!(&bytes[12..16], &0xf900_0211u32.to_le_bytes());
    }

    #[test]
    fn indirect_entry_aggregate_copies_from_a_pointer_register() {
        let pointer = IndirectPointerLocation::Register(MachineRegister::Aarch64X(3));
        let bytes = encode_entry_indirect_argument_write_bytes(pointer, 64, 24)
            .expect("three-fragment indirect entry copy");

        assert_eq!(
            bytes.len(),
            super::super::widths::entry_indirect_argument_write_width(pointer, 64, 24)
        );
        assert_eq!(
            super::super::widths::entry_indirect_argument_frame_base_offset(pointer),
            0
        );
        assert_eq!(&bytes[8..12], &encode_load_x_from_x(17, 3, 0).unwrap());
        assert_eq!(&bytes[12..16], &encode_store_x_to_x(17, 16, 64).unwrap());
        assert_eq!(&bytes[24..28], &encode_load_x_from_x(17, 3, 16).unwrap());
        assert_eq!(&bytes[28..32], &encode_store_x_to_x(17, 16, 80).unwrap());
    }

    #[test]
    fn indirect_entry_aggregate_loads_a_stack_passed_pointer_before_frame_base() {
        let pointer = IndirectPointerLocation::Stack {
            stack_byte_offset: 0,
            alignment: 8,
        };
        let bytes = encode_entry_indirect_argument_write_bytes(pointer, 64, 24)
            .expect("stack-pointer indirect entry copy");

        assert_eq!(
            bytes.len(),
            super::super::widths::entry_indirect_argument_write_width(pointer, 64, 24)
        );
        assert_eq!(
            super::super::widths::entry_indirect_argument_frame_base_offset(pointer),
            4
        );
        assert_eq!(
            &bytes[..4],
            &encode_load_x_from_x(17, 31, super::super::FUNCTION_FRAME_BYTES).unwrap()
        );
        assert_eq!(&bytes[12..16], &encode_load_x_from_x(10, 17, 0).unwrap());
        assert_eq!(&bytes[16..20], &encode_store_x_to_x(10, 16, 64).unwrap());
    }
}

/// The bytes-handoff half of the entry prologue: bind `args: &[u8]` as a view
/// over the entry-argument spill -- write the slice descriptor
/// `{ptr @ desc+0 = frame+spill_offset, len @ desc+8 = byte_length}`. One
/// relocation (the frame pair at instruction start, shared with the
/// register-write record). Fixed 28-byte shape.
pub fn entry_arguments_slice_descriptor_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)])
}

pub fn encode_entry_arguments_slice_descriptor_write_bytes(
    descriptor_offset: usize,
    spill_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let length = u32::try_from(byte_length)
        .map_err(|_| Diagnostic::error("entry-argument slice length exceeds a 32-bit immediate"))?;
    let mut bytes =
        Vec::with_capacity(super::widths::entry_arguments_slice_descriptor_write_width());
    bytes.extend(encode_adrp_placeholder(16)); // frame base [reloc @ start]
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_add_x_immediate(17, 16, spill_offset)?); // ptr = frame + spill
    bytes.extend(encode_store_x_to_x(17, 16, descriptor_offset)?);
    append_unsigned_immediate_w_padded(&mut bytes, 17, length); // fixed 8 bytes
    bytes.extend(encode_store_x_to_x(17, 16, descriptor_offset + 8)?);
    debug_assert_eq!(
        bytes.len(),
        super::widths::entry_arguments_slice_descriptor_write_width()
    );
    Ok(bytes)
}

pub fn encode_runtime_storage_copy_to_return_register_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot copy {byte_size}-byte terminal values to the return register yet"
        )));
    }
    let (register_index, is_float) = match register {
        omega_calling_conventions::MachineRegister::Aarch64X(register_index) => {
            (register_index, false)
        }
        omega_calling_conventions::MachineRegister::Aarch64V(register_index)
            if matches!(byte_size, 4 | 8) =>
        {
            (register_index, true)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot use {register:?} as a {byte_size}-byte result register"
            )));
        }
    };
    if register_index > 30 {
        return Err(Diagnostic::error(format!(
            "AArch64 integer result register x{register_index} is outside the encodable set"
        )));
    }
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_to_return_register_width(byte_offset, byte_size),
    );
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));

    // Mirrors `append_guard_load`'s direct/indirect offset handling so the
    // width helper's `load_data_offset_width` stays exact.
    let direct = runtime_storage_result_offset_is_direct(byte_offset, byte_size);
    let base_register = if direct {
        16
    } else {
        bytes.extend(encode_move_x_register(26, 16));
        append_add_x_constant(&mut bytes, 26, 26, byte_offset, 19)?;
        26
    };
    let load_offset = if direct { byte_offset } else { 0 };
    if is_float {
        let scale = byte_size;
        let immediate = load_offset / scale;
        if !load_offset.is_multiple_of(scale) || immediate > 4095 {
            return Err(Diagnostic::error(
                "AArch64 floating result load offset is not encodable",
            ));
        }
        let base = if byte_size == 4 {
            0xbd40_0000
        } else {
            0xfd40_0000
        };
        bytes.extend(encode_instruction(
            base | ((immediate as u32) << 10)
                | (u32::from(base_register) << 5)
                | u32::from(register_index),
        ));
    } else {
        match byte_size {
            8 => bytes.extend(encode_load_x_from_x(
                register_index,
                base_register,
                load_offset,
            )?),
            _ => bytes.extend(encode_load_w_from_x(
                register_index,
                base_register,
                load_offset,
                byte_size,
            )?),
        }
    }
    match (is_float, byte_size) {
        (false, 1) => bytes.extend(encode_sign_extend_byte_to_w(register_index, register_index)),
        (false, 2) => bytes.extend(encode_sign_extend_halfword_to_w(
            register_index,
            register_index,
        )),
        _ => {}
    }
    debug_assert_eq!(
        bytes.len(),
        super::widths::runtime_storage_copy_to_return_register_width(byte_offset, byte_size)
    );
    Ok(bytes)
}

fn runtime_storage_result_offset_is_direct(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        2 => byte_offset.is_multiple_of(2) && byte_offset / 2 <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}

/// Exact register footprint of the runtime-frame result load above. Large or
/// unscaled offsets use x26 as the adjusted base and x19 while materializing
/// that offset; direct loads need only the relocated x16 base.
pub fn runtime_storage_copy_to_return_register_clobbers(
    register: MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> RegisterSet {
    let mut registers = vec![register, MachineRegister::Aarch64X(16)];
    if !runtime_storage_result_offset_is_direct(byte_offset, byte_size) {
        registers.extend([MachineRegister::Aarch64X(19), MachineRegister::Aarch64X(26)]);
    }
    RegisterSet::new(registers)
}

#[cfg(test)]
mod result_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn runtime_result_load_uses_the_plan_selected_x_register() {
        let bytes = encode_runtime_storage_copy_to_return_register_bytes(
            MachineRegister::Aarch64X(5),
            0,
            4,
        )
        .expect("w5 result load");
        assert_eq!(&bytes[8..12], &0xb940_0205u32.to_le_bytes());
    }

    #[test]
    fn runtime_result_load_uses_the_plan_selected_v_register() {
        let bytes = encode_runtime_storage_copy_to_return_register_bytes(
            MachineRegister::Aarch64V(3),
            8,
            8,
        )
        .expect("d3 result load");
        assert_eq!(&bytes[8..12], &0xfd40_0603u32.to_le_bytes());
    }
}

fn two_instructions(first: [u8; 4], second: [u8; 4]) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&first);
    bytes[4..].copy_from_slice(&second);
    bytes
}
