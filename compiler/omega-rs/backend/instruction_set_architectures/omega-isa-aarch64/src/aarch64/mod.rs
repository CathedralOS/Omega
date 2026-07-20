use crate::Aarch64CallOperand;
use crate::Aarch64CallOperand::*;
use omega_calling_conventions::{MachineRegister, ValueLocation, ValuePlacement};
use omega_core::diagnostics::Diagnostic;

mod dispatch;
mod primitives;
mod runtime_storage;
mod runtime_text;
mod widths;
mod wire_decode;
mod wire_encode;

pub use dispatch::*;
use primitives::*;
pub use runtime_storage::*;
pub use runtime_text::*;
pub use widths::*;
pub use wire_decode::*;
pub use wire_encode::*;

pub fn encode_host_call_sequence(
    operands: &[Aarch64CallOperand],
    argument_placements: &[ValuePlacement],
) -> Result<Vec<u8>, Diagnostic> {
    encode_host_call_sequence_from_operands(operands.iter().copied(), argument_placements)
}

pub fn encode_host_call_sequence_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(host_call_sequence_width_from_operands(operands.clone()));
    let stack_bytes = append_call_operands(&mut bytes, operands, argument_placements)?;
    bytes.extend(encode_branch_link_placeholder());
    append_call_stack_restore(&mut bytes, stack_bytes)?;
    Ok(bytes)
}

/// AAPCS64 per-object vtable dispatch. Arguments are materialized from the
/// normalized placements exactly like an import call; the first planned
/// argument must be the receiver in x0. The callee is loaded from its slot into
/// caller-saved x16 and invoked with `blr x16`, so there is no import fixup.
pub fn encode_vtable_call_sequence_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    index: i64,
) -> Result<Vec<u8>, Diagnostic> {
    encode_vtable_call_sequence_at_offset_from_operands(
        operands,
        argument_placements,
        vtable_slot_byte_offset(index)?,
    )
}

/// The field-model form of a result-free AAPCS64 vtable call. Unlike the
/// legacy slot form, `byte_offset` is already resolved from the table layout.
pub fn encode_vtable_call_sequence_at_offset_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    validate_vtable_receiver(argument_placements)?;
    let mut bytes = Vec::with_capacity(
        host_call_sequence_width_from_operands(operands.clone())
            + host_call_stack_total_width_for_placements(argument_placements)
            + 4,
    );
    let stack_bytes = append_call_operands(&mut bytes, operands, argument_placements)?;
    append_vtable_dispatch(&mut bytes, byte_offset)?;
    append_call_stack_restore(&mut bytes, stack_bytes)?;
    Ok(bytes)
}

/// The field-model form with a leading scalar result place. Arguments still
/// consume their normalized AAPCS64 placements; after indirect dispatch the
/// plan-selected GPR result is stored through the same relocated tail as a
/// direct import.
pub fn encode_vtable_call_sequence_at_offset_value_returning_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    result_register: MachineRegister,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    validate_vtable_receiver(argument_placements)?;
    let all = operands.collect::<Vec<_>>();
    let Some((result, arguments)) = all.split_first() else {
        return Err(Diagnostic::error(
            "AArch64 value-returning vtable call has no result storage operand",
        ));
    };
    let RuntimeScalarInteger {
        byte_offset: result_byte_offset,
        byte_count,
    } = *result
    else {
        return Err(Diagnostic::error(
            "AArch64 value-returning vtable result place did not lower to a runtime scalar",
        ));
    };
    let MachineRegister::Aarch64X(result_register) = result_register else {
        return Err(Diagnostic::error(format!(
            "AArch64 integer-returning vtable plan selected non-GPR result register {result_register:?}"
        )));
    };
    let mut bytes = Vec::with_capacity(
        host_call_sequence_width_from_operands(all.iter().copied())
            + host_call_stack_total_width_for_placements(argument_placements)
            + 4,
    );
    let stack_bytes =
        append_call_operands(&mut bytes, arguments.iter().copied(), argument_placements)?;
    append_vtable_dispatch(&mut bytes, byte_offset)?;
    append_call_stack_restore(&mut bytes, stack_bytes)?;
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_store_data_to_x_offset(
        &mut bytes,
        result_register,
        16,
        result_byte_offset,
        byte_count,
        17,
    )?;
    Ok(bytes)
}

/// AAPCS64 service-table dispatch. The table pointer is a storage operand used
/// only to find the callee; it is excluded from `argument_placements`, so the
/// first declared function argument still consumes x0/v0. Operand roles are
/// `[result?][table][arguments...]`.
pub fn encode_table_function_call_sequence_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    result_register: Option<MachineRegister>,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let all = operands.collect::<Vec<_>>();
    let table_index = usize::from(result_register.is_some());
    let Some(table) = all.get(table_index) else {
        return Err(Diagnostic::error(
            "AArch64 table-function call has no dispatch table operand",
        ));
    };
    let RuntimeScalarInteger {
        byte_offset: table_byte_offset,
        byte_count: 8,
    } = *table
    else {
        return Err(Diagnostic::error(
            "AArch64 table-function dispatch table did not lower to an eight-byte runtime scalar",
        ));
    };
    let arguments = &all[table_index + 1..];
    let mut bytes = Vec::with_capacity(
        host_call_sequence_width_from_operands(all.iter().copied())
            + host_call_stack_total_width_for_placements(argument_placements)
            + 4,
    );
    let stack_bytes =
        append_call_operands(&mut bytes, arguments.iter().copied(), argument_placements)?;

    // Materialize the storage region, read its table pointer, then read and
    // call the layout-selected function pointer. x16/x17 are caller-saved and
    // already required by the normalized compatibility-plan ceiling.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_load_data_from_x_offset(&mut bytes, 16, 16, table_byte_offset, 8, 17)?;
    append_vtable_dispatch_from_register(&mut bytes, 16, byte_offset)?;
    append_call_stack_restore(&mut bytes, stack_bytes)?;

    if let Some(result_register) = result_register {
        let Some(RuntimeScalarInteger {
            byte_offset: result_byte_offset,
            byte_count,
        }) = all.first().copied()
        else {
            return Err(Diagnostic::error(
                "AArch64 value-returning table-function result place did not lower to a runtime scalar",
            ));
        };
        let MachineRegister::Aarch64X(result_register) = result_register else {
            return Err(Diagnostic::error(format!(
                "AArch64 integer-returning table-function plan selected non-GPR result register {result_register:?}"
            )));
        };
        bytes.extend(encode_adrp_placeholder(16));
        bytes.extend(encode_add_page_offset_placeholder(16));
        append_store_data_to_x_offset(
            &mut bytes,
            result_register,
            16,
            result_byte_offset,
            byte_count,
            17,
        )?;
    }
    Ok(bytes)
}

fn validate_vtable_receiver(argument_placements: &[ValuePlacement]) -> Result<(), Diagnostic> {
    let Some(receiver) = argument_placements.first() else {
        return Err(Diagnostic::error(
            "AArch64 vtable call has no receiver placement",
        ));
    };
    if !matches!(
        receiver.locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            value_byte_offset: 0,
            byte_size: 8,
        }]
    ) {
        return Err(Diagnostic::error(format!(
            "AAPCS64 vtable receiver requires x0, got {:?}",
            receiver.locations
        )));
    }
    Ok(())
}

fn append_vtable_dispatch(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    append_vtable_dispatch_from_register(bytes, 0, byte_offset)
}

fn append_vtable_dispatch_from_register(
    bytes: &mut Vec<u8>,
    table_register: u8,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_load_x_from_x(16, table_register, byte_offset)?);
    bytes.extend(encode_instruction(0xd63f_0000 | (16 << 5))); // blr x16
    Ok(())
}

pub fn vtable_call_dispatch_width(index: i64) -> Option<usize> {
    vtable_slot_byte_offset(index)
        .ok()
        .and_then(vtable_call_dispatch_width_at_offset)
}

pub fn vtable_call_dispatch_width_at_offset(byte_offset: usize) -> Option<usize> {
    encode_load_x_from_x(16, 0, byte_offset).ok().map(|_| 8)
}

fn vtable_slot_byte_offset(index: i64) -> Result<usize, Diagnostic> {
    let index = usize::try_from(index)
        .map_err(|_| Diagnostic::error("AArch64 vtable slot index cannot be negative"))?;
    index
        .checked_mul(8)
        .ok_or_else(|| Diagnostic::error("AArch64 vtable slot offset overflows usize"))
}

/// A VALUE-RETURNING host call: `operands[0]` is the result storage place, the
/// rest are the call arguments. Marshal the args into x0.., branch-link to the
/// callee (relocated to the import symbol), then store the return register into
/// the result place — `str w0` for a 4-byte result (an `i32` fd/rc; the sign is
/// preserved so a negative `-errno` reads back correctly), `str x0` for 8 bytes
/// (an `i64` byte count). Mirrors x86_64's `encode_win64_import_call(.., true)`.
/// The total width equals the non-returning form: the result operand's scalar
/// width (adrp+add+ldr = 12) is the same as its store (adrp+add+str = 12).
pub fn encode_host_call_sequence_value_returning_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    result_register: MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let all: Vec<Aarch64CallOperand> = operands.collect();
    let Some((result, args)) = all.split_first() else {
        return Err(Diagnostic::error(
            "AArch64 value-returning host call has no result storage operand",
        ));
    };
    let RuntimeScalarInteger {
        byte_offset,
        byte_count,
    } = *result
    else {
        return Err(Diagnostic::error(
            "AArch64 value-returning host call result place did not lower to a runtime scalar",
        ));
    };
    let mut bytes = Vec::with_capacity(host_call_sequence_width_from_operands(all.iter().copied()));
    let stack_bytes = append_call_operands(&mut bytes, args.iter().copied(), argument_placements)?;
    bytes.extend(encode_branch_link_placeholder());
    append_call_stack_restore(&mut bytes, stack_bytes)?;
    let MachineRegister::Aarch64X(result_register) = result_register else {
        return Err(Diagnostic::error(format!(
            "AArch64 integer-returning call plan selected non-GPR result register {result_register:?}"
        )));
    };
    // Result store: x16 <- result region base (adrp/add relocated), then store
    // the return register at the field's offset.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize a large result-field offset (a scalar after a big array) via scratch
    // x17 (caller-saved; the planned register holds the result, x16 the region base).
    // Kept in lockstep
    // with operand_width's store_data_offset_width accounting + the relocation planner.
    append_store_data_to_x_offset(&mut bytes, result_register, 16, byte_offset, byte_count, 17)?;
    Ok(bytes)
}

/// An authored import returning a flat homogeneous floating-point aggregate.
/// The normalized AAPCS64 result placement names every returned `v` register;
/// spill those member fragments into the single selected aggregate result
/// place after the call. One relocated x16 base serves every member store.
pub fn encode_host_call_sequence_hfa_returning_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    result_placement: &ValuePlacement,
) -> Result<Vec<u8>, Diagnostic> {
    let all = operands.collect::<Vec<_>>();
    let Some((result, arguments)) = all.split_first() else {
        return Err(Diagnostic::error(
            "AArch64 HFA-returning host call has no result storage operand",
        ));
    };
    let RuntimeHomogeneousFloatAggregate {
        byte_offset,
        member_byte_count,
        members,
    } = *result
    else {
        return Err(Diagnostic::error(
            "AArch64 HFA-returning host call result place is not a homogeneous float aggregate",
        ));
    };
    if !matches!(member_byte_count, 4 | 8)
        || !matches!(
            result_placement.shape.class,
            omega_calling_conventions::ValueClass::HomogeneousFloatAggregate {
                members: planned_members
            } if planned_members == members
        )
        || usize::from(result_placement.shape.byte_size) != member_byte_count * usize::from(members)
        || result_placement.locations.len() != usize::from(members)
    {
        return Err(Diagnostic::error(
            "AAPCS64 HFA result placement disagrees with its selected storage shape",
        ));
    }

    let mut bytes = Vec::with_capacity(host_call_sequence_width_from_operands(all.iter().copied()));
    let stack_bytes =
        append_call_operands(&mut bytes, arguments.iter().copied(), argument_placements)?;
    bytes.extend(encode_branch_link_placeholder());
    append_call_stack_restore(&mut bytes, stack_bytes)?;
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));

    for (member, location) in result_placement.locations.iter().copied().enumerate() {
        let member_offset = member * member_byte_count;
        let ValueLocation::Register {
            register: MachineRegister::Aarch64V(register),
            value_byte_offset,
            byte_size,
        } = location
        else {
            return Err(Diagnostic::error(format!(
                "AAPCS64 HFA result member {member} is not in a vector register: {location:?}"
            )));
        };
        if usize::from(value_byte_offset) != member_offset
            || usize::from(byte_size) != member_byte_count
        {
            return Err(Diagnostic::error(format!(
                "AAPCS64 HFA result member {member} disagrees with its normalized byte range"
            )));
        }
        bytes.extend(encode_float_move_to_gpr(member_byte_count, 17, register)?);
        append_store_data_to_x_offset(
            &mut bytes,
            17,
            16,
            byte_offset.checked_add(member_offset).ok_or_else(|| {
                Diagnostic::error("AAPCS64 HFA result storage offset overflows usize")
            })?,
            member_byte_count,
            9,
        )?;
    }
    Ok(bytes)
}

/// A CONSTANT-RESULT host op (`PlatformCallData::ConstantResult`; std::time's
/// wall-clock calibration constants): NO call at all. operands[0] is the
/// result place, operands[1] the constant. Materialize the imm64 into x0 with
/// a FIXED-width movz+movk*3 (16 bytes -- padded so the width and the
/// relocation offset are layout-constant), then the standard result store
/// tail (adrp/add x16 relocated to the result region + store). Total width =
/// 16 + 8 + store_data_offset_width; the result data-address site sits at 16.
pub fn encode_host_call_sequence_constant_result_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
) -> Result<Vec<u8>, Diagnostic> {
    let all: Vec<Aarch64CallOperand> = operands.collect();
    let Some(RuntimeScalarInteger {
        byte_offset,
        byte_count,
    }) = all.first().copied()
    else {
        return Err(Diagnostic::error(
            "AArch64 constant-result host op's result place did not lower to a runtime scalar",
        ));
    };
    let Some(ImmediateInteger(value)) = all.get(1).copied() else {
        return Err(Diagnostic::error(
            "AArch64 constant-result host op did not lower its constant to an immediate operand",
        ));
    };
    let mut bytes = Vec::with_capacity(constant_result_sequence_width(byte_offset, byte_count));
    append_unsigned_immediate_padded(&mut bytes, 0, value as u64);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_store_data_to_x_offset(&mut bytes, 0, 16, byte_offset, byte_count, 17)?;
    debug_assert_eq!(
        bytes.len(),
        constant_result_sequence_width(byte_offset, byte_count)
    );
    Ok(bytes)
}

/// A value-returning host call whose callee returns a POINTER to the real
/// result (darwin `___error()` -> `&errno`). Identical to
/// `encode_host_call_sequence_value_returning_from_operands` except that, right
/// after the `BL`, it derefs the return register once with `ldr w0,[x0]`
/// (0xB9400000) so the stored value is `*x0` (the errno int), not the pointer.
/// The single extra 4-byte load is why `dereferences_result` adds 4 to both the
/// call-sequence width and the result-store data-address relocation offset — the
/// store now sits 4 bytes later. `read_errno` takes no args, so the `BL`
/// relocation (which precedes the load) is unaffected.
pub fn encode_host_call_sequence_value_returning_deref_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    result_register: MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let all: Vec<Aarch64CallOperand> = operands.collect();
    let Some((result, args)) = all.split_first() else {
        return Err(Diagnostic::error(
            "AArch64 deref host call has no result storage operand",
        ));
    };
    let RuntimeScalarInteger {
        byte_offset,
        byte_count,
    } = *result
    else {
        return Err(Diagnostic::error(
            "AArch64 deref host call result place did not lower to a runtime scalar",
        ));
    };
    let mut bytes =
        Vec::with_capacity(host_call_sequence_width_from_operands(all.iter().copied()) + 4);
    let stack_bytes = append_call_operands(&mut bytes, args.iter().copied(), argument_placements)?;
    bytes.extend(encode_branch_link_placeholder());
    append_call_stack_restore(&mut bytes, stack_bytes)?;
    let MachineRegister::Aarch64X(result_register) = result_register else {
        return Err(Diagnostic::error(format!(
            "AArch64 pointer-returning call plan selected non-GPR result register {result_register:?}"
        )));
    };
    // Deref the returned pointer through the plan-selected result register.
    bytes.extend(encode_load_w_from_x(
        result_register,
        result_register,
        0,
        4,
    )?);
    // Result store: x16 <- result region base (adrp/add relocated), then store.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize a large result-field offset (a scalar after a big array) via scratch
    // x17 (caller-saved; the planned register holds the result, x16 the region base).
    // Kept in lockstep
    // with operand_width's store_data_offset_width accounting + the relocation planner.
    append_store_data_to_x_offset(&mut bytes, result_register, 16, byte_offset, byte_count, 17)?;
    Ok(bytes)
}

/// A value-returning host call whose callee returns its result in the FLOAT
/// return register `d0`/`s0` (libm `sqrt`, `hypot`; Core Graphics `double`
/// getters). Identical to `encode_host_call_sequence_value_returning_from_operands`
/// except that, right after the `BL`, it moves the raw float bits back into the
/// GPR bank with `fmov x0, d0` (`encode_float_move_to_gpr`) so the normal
/// integer result-store can spill the 8 bytes into the field. The result place is
/// an `f64` slot but the store is bit-identical to an i64 store, so the result
/// operand still destructures as `RuntimeScalarInteger`. The single extra 4-byte
/// `fmov` is why `returns_float` adds 4 to both the call-sequence width and the
/// result-store data-address relocation offset (the store sits 4 bytes later) —
/// MUST stay in lockstep with those sites. Float args precede the `BL`, so the
/// `BL` relocation is unaffected.
pub fn encode_host_call_sequence_value_returning_float_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    result_register: MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let all: Vec<Aarch64CallOperand> = operands.collect();
    let Some((result, args)) = all.split_first() else {
        return Err(Diagnostic::error(
            "AArch64 float-returning host call has no result storage operand",
        ));
    };
    let RuntimeScalarInteger {
        byte_offset,
        byte_count,
    } = *result
    else {
        return Err(Diagnostic::error(
            "AArch64 float-returning host call result place did not lower to a runtime scalar",
        ));
    };
    let mut bytes =
        Vec::with_capacity(host_call_sequence_width_from_operands(all.iter().copied()) + 4);
    let stack_bytes = append_call_operands(&mut bytes, args.iter().copied(), argument_placements)?;
    bytes.extend(encode_branch_link_placeholder());
    append_call_stack_restore(&mut bytes, stack_bytes)?;
    let MachineRegister::Aarch64V(result_register) = result_register else {
        return Err(Diagnostic::error(format!(
            "AArch64 float-returning call plan selected non-vector result register {result_register:?}"
        )));
    };
    // Move the float return from the plan-selected vector register into `x0` so
    // the integer result-store can spill the raw bits.
    bytes.extend(encode_float_move_to_gpr(
        byte_count.max(4),
        0,
        result_register,
    )?);
    // Result store: x16 <- result region base (adrp/add relocated), then store.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize a large result-field offset (a scalar after a big array) via scratch
    // x17 (caller-saved; the planned vector register was copied to x0, x16 holds
    // the region base). Kept in lockstep
    // with operand_width's store_data_offset_width accounting + the relocation planner.
    append_store_data_to_x_offset(&mut bytes, 0, 16, byte_offset, byte_count, 17)?;
    Ok(bytes)
}

/// A value-returning host call whose TRAILING argument (a `mode`) is passed on the
/// STACK, not a register — darwin `open(path, flags, ...)` reads the create `mode`
/// via `va_arg`, and Apple arm64 places variadic args at `[sp,#0]`. The register
/// args (`path` -> x0, `flags` -> x1) marshal normally; then the call is bracketed
/// by `sub sp,sp,#16` … `str w9,[sp]` … `bl` … `add sp,sp,#16`. The `mode` must be
/// a compile-time immediate (materialized into the caller-saved w9, no relocation
/// of its own). The +12 (sub+str+add) is why `passes_trailing_mode_on_stack` adds
/// 12 to the width + result-store relocation and 8 to the `BL` relocation (the add
/// sits AFTER the BL) — MUST stay in lockstep with those sites.
pub fn encode_host_call_sequence_value_returning_open_create_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    argument_placements: &[ValuePlacement],
    result_register: MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let all: Vec<Aarch64CallOperand> = operands.collect();
    let Some((result, args)) = all.split_first() else {
        return Err(Diagnostic::error(
            "AArch64 open_create host call has no result storage operand",
        ));
    };
    let RuntimeScalarInteger {
        byte_offset,
        byte_count,
    } = *result
    else {
        return Err(Diagnostic::error(
            "AArch64 open_create result place did not lower to a runtime scalar",
        ));
    };
    // args = [path, flags, mode]; the trailing `mode` is the stack-passed variadic.
    let Some((mode_operand, register_args)) = args.split_last() else {
        return Err(Diagnostic::error(
            "AArch64 open_create host call is missing its mode argument",
        ));
    };
    let ImmediateInteger(mode) = *mode_operand else {
        return Err(Diagnostic::error(
            "AArch64 open_create mode must be a compile-time immediate (variadic stack marshalling)",
        ));
    };
    let mut bytes =
        Vec::with_capacity(host_call_sequence_width_from_operands(all.iter().copied()) + 12);
    // `path` -> x0, `flags` -> x1 (the named register args).
    let named_stack_bytes = append_call_operands(
        &mut bytes,
        register_args.iter().copied(),
        argument_placements,
    )?;
    if named_stack_bytes != 0 {
        return Err(Diagnostic::error(
            "AArch64 open_create cannot combine named stack arguments with its variadic stack seam",
        ));
    }
    // `mode` -> [sp,#0]: reserve a 16-byte-aligned slot, materialize into w9, store.
    bytes.extend(encode_instruction(0xD100_43FF)); // sub sp, sp, #16
    append_immediate(&mut bytes, 9, mode)?; // movz w9, #mode (+ movk if wide)
    bytes.extend(encode_instruction(0xB900_03E9)); // str w9, [sp]
    bytes.extend(encode_branch_link_placeholder());
    bytes.extend(encode_instruction(0x9100_43FF)); // add sp, sp, #16
    let MachineRegister::Aarch64X(result_register) = result_register else {
        return Err(Diagnostic::error(format!(
            "AArch64 open_create call plan selected non-GPR result register {result_register:?}"
        )));
    };
    // Result store: x16 <- result region base (adrp/add relocated), then store.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize a large result-field offset (a scalar after a big array) via scratch
    // x17 (caller-saved; the planned register holds the result, x16 the region base).
    // Kept in lockstep
    // with operand_width's store_data_offset_width accounting + the relocation planner.
    append_store_data_to_x_offset(&mut bytes, result_register, 16, byte_offset, byte_count, 17)?;
    Ok(bytes)
}

pub fn encode_syscall_sequence(
    operands: &[Aarch64CallOperand],
    syscall_number: u32,
    argument_registers: &[omega_calling_conventions::MachineRegister],
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    encode_syscall_sequence_from_operands(
        operands.iter().copied(),
        syscall_number,
        argument_registers,
        number_register,
        supervisor_call,
    )
}

pub fn encode_syscall_sequence_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    syscall_number: u32,
    argument_registers: &[omega_calling_conventions::MachineRegister],
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(syscall_sequence_width_from_operands(
        operands.clone(),
        syscall_number,
    ));
    let omega_calling_conventions::MachineRegister::Aarch64X(number_register) = number_register
    else {
        return Err(Diagnostic::error(format!(
            "AArch64 syscall plan selected non-GPR number register {number_register:?}"
        )));
    };
    append_syscall_operands(&mut bytes, operands, argument_registers, number_register)?;
    append_unsigned_immediate(&mut bytes, number_register, u64::from(syscall_number));
    bytes.extend(encode_svc(supervisor_call));
    Ok(bytes)
}

fn append_syscall_operands(
    bytes: &mut Vec<u8>,
    operands: impl Iterator<Item = Aarch64CallOperand>,
    argument_registers: &[omega_calling_conventions::MachineRegister],
    number_register: u8,
) -> Result<(), Diagnostic> {
    let operands = operands.collect::<Vec<_>>();
    if operands.len() != argument_registers.len() {
        return Err(Diagnostic::error(format!(
            "AArch64 syscall plan supplied {} argument registers for {} operands",
            argument_registers.len(),
            operands.len()
        )));
    }

    for (operand, register) in operands.into_iter().zip(argument_registers.iter().copied()) {
        let omega_calling_conventions::MachineRegister::Aarch64X(register) = register else {
            return Err(Diagnostic::error(format!(
                "AArch64 syscall plan selected non-GPR argument register {register:?}"
            )));
        };
        match operand {
            ImmediateInteger(value) => append_immediate(bytes, register, value)?,
            DataAddress { .. } => {
                bytes.extend(encode_adrp_placeholder(register));
                bytes.extend(encode_add_page_offset_placeholder(register));
            }
            RuntimeStringPointer {
                byte_offset,
                is_bounded_buffer,
            } => {
                bytes.extend(encode_adrp_placeholder(register));
                bytes.extend(encode_add_page_offset_placeholder(register));
                if is_bounded_buffer {
                    bytes.extend(encode_add_x_immediate(register, register, byte_offset + 8)?);
                } else {
                    bytes.extend(encode_load_x_from_x(register, register, byte_offset)?);
                }
            }
            RuntimeStringLength {
                byte_offset,
                is_bounded_buffer,
            } => {
                bytes.extend(encode_adrp_placeholder(register));
                bytes.extend(encode_add_page_offset_placeholder(register));
                bytes.extend(encode_load_x_from_x(
                    register,
                    register,
                    if is_bounded_buffer {
                        byte_offset
                    } else {
                        byte_offset + 8
                    },
                )?);
            }
            RuntimePointeeStringPointer { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(register));
                bytes.extend(encode_add_page_offset_placeholder(register));
                bytes.extend(encode_load_x_from_x(register, register, byte_offset)?);
                bytes.extend(encode_load_x_from_x(register, register, 0)?);
            }
            RuntimePointeeStringLength { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(register));
                bytes.extend(encode_add_page_offset_placeholder(register));
                bytes.extend(encode_load_x_from_x(register, register, byte_offset)?);
                bytes.extend(encode_load_x_from_x(register, register, 8)?);
            }
            RuntimeScalarInteger {
                byte_offset,
                byte_count,
            } => {
                bytes.extend(encode_adrp_placeholder(register));
                bytes.extend(encode_add_page_offset_placeholder(register));
                append_load_data_from_x_offset(
                    bytes,
                    register,
                    register,
                    byte_offset,
                    byte_count,
                    number_register,
                )?;
            }
            RuntimeStorageAddress { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(register));
                bytes.extend(encode_add_page_offset_placeholder(register));
                append_add_x_constant(bytes, register, register, byte_offset, number_register)?;
            }
            ByteLength(value) => append_unsigned_immediate(bytes, register, value as u64),
            RuntimeScalarFloat { .. } => {
                return Err(Diagnostic::error(
                    "AArch64 Linux syscall plans do not admit float operands",
                ));
            }
            RuntimeHomogeneousFloatAggregate { .. } => {
                return Err(Diagnostic::error(
                    "AArch64 Linux syscall plans do not admit HFA operands",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod syscall_plan_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn syscall_arguments_and_control_use_the_plan_selected_registers() {
        let bytes = encode_syscall_sequence(
            &[Aarch64CallOperand::ImmediateInteger(7)],
            64,
            &[MachineRegister::Aarch64X(3)],
            MachineRegister::Aarch64X(12),
            5,
        )
        .expect("noncanonical syscall registers should encode");

        assert_eq!(&bytes[0..4], &0xd280_00e3u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &0xd280_080cu32.to_le_bytes());
        assert_eq!(&bytes[8..12], &0xd400_00a1u32.to_le_bytes());
    }

    #[test]
    fn large_syscall_operand_offsets_reuse_the_number_register_as_scratch() {
        let bytes = encode_syscall_sequence(
            &[Aarch64CallOperand::RuntimeScalarInteger {
                byte_offset: 0x1_0000,
                byte_count: 8,
            }],
            64,
            &[MachineRegister::Aarch64X(0)],
            MachineRegister::Aarch64X(12),
            0,
        )
        .expect("large-offset syscall argument");

        assert!(
            bytes
                .windows(4)
                .any(|window| window == 0xf2a0_002cu32.to_le_bytes()),
            "the large offset must materialize in plan-selected x12"
        );
        assert!(
            bytes
                .windows(4)
                .any(|window| window == 0x8b0c_000cu32.to_le_bytes()),
            "the address add must remain in plan-selected x12"
        );
    }
}

/// Bytes reserved by the ordinary AArch64 function-enter prologue. Incoming
/// stack arguments remain relative to the caller's pre-prologue SP.
pub const FUNCTION_FRAME_BYTES: usize = 96;

pub fn encode_function_enter_bytes() -> [u8; 28] {
    let mut bytes = [0; 28];
    bytes[0..4].copy_from_slice(&encode_instruction(0xA9BA7BFD));
    bytes[4..8].copy_from_slice(&encode_instruction(0x910003FD));
    bytes[8..12].copy_from_slice(&encode_instruction(0xA90153F3));
    bytes[12..16].copy_from_slice(&encode_instruction(0xA9025BF5));
    bytes[16..20].copy_from_slice(&encode_instruction(0xA90363F7));
    bytes[20..24].copy_from_slice(&encode_instruction(0xA9046BF9));
    bytes[24..28].copy_from_slice(&encode_instruction(0xA90573FB));
    bytes
}

pub fn encode_return_bytes() -> [u8; 28] {
    let mut bytes = [0; 28];
    bytes[0..4].copy_from_slice(&encode_instruction(0xA94153F3));
    bytes[4..8].copy_from_slice(&encode_instruction(0xA9425BF5));
    bytes[8..12].copy_from_slice(&encode_instruction(0xA94363F7));
    bytes[12..16].copy_from_slice(&encode_instruction(0xA9446BF9));
    bytes[16..20].copy_from_slice(&encode_instruction(0xA94573FB));
    bytes[20..24].copy_from_slice(&encode_instruction(0xA8C67BFD));
    bytes[24..28].copy_from_slice(&encode_instruction(0xD65F03C0));
    bytes
}

/// The AArch64 idle instruction `wfi` (wait for interrupt, 0xD503207F) -- the
/// semantic analog of x86 `hlt`: halt the core until an interrupt arrives.
/// `asm { hlt }` lowers to this on AArch64.
pub fn encode_machine_halt_bytes() -> [u8; 4] {
    encode_instruction(0xD503207F)
}

pub fn encode_return_register_integer_write_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
    value: i64,
) -> Result<[u8; 4], Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_size}-byte return integers yet"
        )));
    }
    let omega_calling_conventions::MachineRegister::Aarch64X(register_index) = register else {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot use {register:?} as an integer result register"
        )));
    };
    if register_index > 30 {
        return Err(Diagnostic::error(format!(
            "AArch64 integer result register x{register_index} is outside the encodable set"
        )));
    }
    let immediate = u16::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write return integer `{value}` yet"
        ))
    })?;
    Ok(if byte_size == 8 {
        encode_movz(register_index, immediate)
    } else {
        encode_movz_w(register_index, immediate)
    })
}

#[cfg(test)]
mod result_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn constant_result_uses_the_plan_selected_x_register() {
        let bytes = encode_return_register_integer_write_bytes(MachineRegister::Aarch64X(3), 4, 7)
            .expect("w3 result write");
        assert_eq!(bytes, 0x5280_00e3u32.to_le_bytes());
    }
}

#[cfg(test)]
mod host_call_plan_register_tests {
    use super::*;

    fn placement(
        shape: omega_calling_conventions::ValueShape,
        location: ValueLocation,
    ) -> ValuePlacement {
        ValuePlacement {
            shape,
            locations: vec![location],
        }
    }

    #[test]
    fn import_arguments_use_the_plan_selected_registers() {
        let bytes = encode_host_call_sequence(
            &[Aarch64CallOperand::ImmediateInteger(7)],
            &[placement(
                omega_calling_conventions::ValueShape::integer(8, 8),
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(3),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            )],
        )
        .expect("noncanonical AAPCS register should encode");

        assert_eq!(&bytes[0..4], &0xd280_00e3u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &encode_branch_link_placeholder());
    }

    #[test]
    fn vtable_call_loads_the_planned_x0_receiver_and_dispatches_indirectly() {
        let operands = [
            Aarch64CallOperand::RuntimeScalarInteger {
                byte_offset: 0,
                byte_count: 8,
            },
            Aarch64CallOperand::ImmediateInteger(7),
        ];
        let placements = [
            placement(
                omega_calling_conventions::ValueShape::integer(8, 8),
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ),
            placement(
                omega_calling_conventions::ValueShape::integer(8, 8),
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(1),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ),
        ];
        let bytes = encode_vtable_call_sequence_from_operands(operands.into_iter(), &placements, 1)
            .expect("AAPCS64 vtable call");

        assert_eq!(&bytes[8..12], &encode_load_x_from_x(0, 0, 0).unwrap());
        assert_eq!(&bytes[16..20], &encode_load_x_from_x(16, 0, 8).unwrap());
        assert_eq!(&bytes[20..24], &encode_instruction(0xd63f_0200));
        assert_eq!(bytes.len(), 24);
        assert_eq!(vtable_call_dispatch_width(1), Some(8));
    }

    #[test]
    fn vtable_field_stores_the_plan_selected_scalar_result_after_dispatch() {
        let operands = [
            Aarch64CallOperand::RuntimeScalarInteger {
                byte_offset: 32,
                byte_count: 4,
            },
            Aarch64CallOperand::RuntimeScalarInteger {
                byte_offset: 0,
                byte_count: 8,
            },
            Aarch64CallOperand::ImmediateInteger(7),
        ];
        let placements = [
            placement(
                omega_calling_conventions::ValueShape::integer(8, 8),
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ),
            placement(
                omega_calling_conventions::ValueShape::integer(8, 8),
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(1),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ),
        ];
        let bytes = encode_vtable_call_sequence_at_offset_value_returning_from_operands(
            operands.into_iter(),
            &placements,
            MachineRegister::Aarch64X(0),
            24,
        )
        .expect("AAPCS64 value-returning vtable field call");

        assert_eq!(&bytes[16..20], &encode_load_x_from_x(16, 0, 24).unwrap());
        assert_eq!(&bytes[20..24], &encode_instruction(0xd63f_0200));
        assert_eq!(&bytes[24..28], &encode_adrp_placeholder(16));
        assert_eq!(bytes.len(), 36);
    }

    #[test]
    fn table_function_keeps_its_table_pointer_out_of_x0() {
        let operands = [
            Aarch64CallOperand::RuntimeScalarInteger {
                byte_offset: 40,
                byte_count: 8,
            },
            Aarch64CallOperand::ImmediateInteger(7),
        ];
        let placements = [placement(
            omega_calling_conventions::ValueShape::integer(8, 8),
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 8,
            },
        )];
        let bytes = encode_table_function_call_sequence_from_operands(
            operands.into_iter(),
            &placements,
            None,
            24,
        )
        .expect("AAPCS64 table-function call");

        assert_eq!(&bytes[0..4], &encode_movz(0, 7));
        assert_eq!(&bytes[4..8], &encode_adrp_placeholder(16));
        assert_eq!(&bytes[12..16], &encode_load_x_from_x(16, 16, 40).unwrap());
        assert_eq!(&bytes[16..20], &encode_load_x_from_x(16, 16, 24).unwrap());
        assert_eq!(&bytes[20..24], &encode_instruction(0xd63f_0200));
        assert_eq!(bytes.len(), 24);
    }

    #[test]
    fn import_results_use_the_plan_selected_register() {
        let bytes = encode_host_call_sequence_value_returning_from_operands(
            [
                Aarch64CallOperand::RuntimeScalarInteger {
                    byte_offset: 0,
                    byte_count: 8,
                },
                Aarch64CallOperand::ImmediateInteger(7),
            ]
            .into_iter(),
            &[placement(
                omega_calling_conventions::ValueShape::integer(8, 8),
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(3),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            )],
            MachineRegister::Aarch64X(5),
        )
        .expect("noncanonical AAPCS result register should encode");

        assert_eq!(&bytes[0..4], &0xd280_00e3u32.to_le_bytes());
        assert_eq!(
            &bytes[bytes.len() - 4..],
            &encode_store_x_to_x(5, 16, 0).expect("store x5")
        );
    }

    #[test]
    fn hfa_results_spill_each_plan_selected_vector_register() {
        let result = Aarch64CallOperand::RuntimeHomogeneousFloatAggregate {
            byte_offset: 32,
            member_byte_count: 8,
            members: 2,
        };
        let placement = ValuePlacement {
            shape: omega_calling_conventions::ValueShape::homogeneous_float_aggregate(8, 2),
            locations: vec![
                ValueLocation::Register {
                    register: MachineRegister::Aarch64V(3),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::Aarch64V(4),
                    value_byte_offset: 8,
                    byte_size: 8,
                },
            ],
        };

        let bytes = encode_host_call_sequence_hfa_returning_from_operands(
            [result].into_iter(),
            &[],
            &placement,
        )
        .expect("fragmented HFA result should encode");

        assert_eq!(&bytes[0..4], &encode_branch_link_placeholder());
        assert_eq!(&bytes[12..16], &encode_float_move_to_gpr(8, 17, 3).unwrap());
        assert_eq!(&bytes[16..20], &encode_store_x_to_x(17, 16, 32).unwrap());
        assert_eq!(&bytes[20..24], &encode_float_move_to_gpr(8, 17, 4).unwrap());
        assert_eq!(&bytes[24..28], &encode_store_x_to_x(17, 16, 40).unwrap());
        assert_eq!(bytes.len(), crate::aarch64::operand_width(&result) + 4);
    }

    #[test]
    fn import_register_bank_mismatches_are_rejected() {
        let error = encode_host_call_sequence(
            &[Aarch64CallOperand::ImmediateInteger(7)],
            &[placement(
                omega_calling_conventions::ValueShape::integer(8, 8),
                ValueLocation::Register {
                    register: MachineRegister::Aarch64V(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            )],
        )
        .expect_err("integer operand in vector register must reject");

        assert!(error.message.contains("non-GPR"));
    }

    #[test]
    fn scalar_stack_argument_is_reserved_stored_and_restored() {
        let bytes = encode_host_call_sequence(
            &[Aarch64CallOperand::ImmediateInteger(7)],
            &[placement(
                omega_calling_conventions::ValueShape::integer(8, 8),
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                },
            )],
        )
        .expect("scalar AAPCS64 stack argument");

        assert_eq!(&bytes[0..4], &0xd100_43ffu32.to_le_bytes());
        assert_eq!(&bytes[4..8], &0xd280_00eau32.to_le_bytes());
        assert_eq!(&bytes[8..12], &0xf900_03eau32.to_le_bytes());
        assert_eq!(&bytes[12..16], &encode_branch_link_placeholder());
        assert_eq!(&bytes[16..20], &0x9100_43ffu32.to_le_bytes());
    }

    #[test]
    fn float_stack_argument_uses_the_planned_d_slot() {
        let bytes = encode_host_call_sequence(
            &[Aarch64CallOperand::RuntimeScalarFloat {
                byte_offset: 0,
                byte_count: 8,
            }],
            &[placement(
                omega_calling_conventions::ValueShape::float(8),
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                },
            )],
        )
        .expect("f64 AAPCS64 stack argument");

        assert_eq!(&bytes[0..4], &0xd100_43ffu32.to_le_bytes());
        assert_eq!(&bytes[20..24], &0xfd00_03ffu32.to_le_bytes());
        assert_eq!(&bytes[24..28], &encode_branch_link_placeholder());
        assert_eq!(&bytes[28..32], &0x9100_43ffu32.to_le_bytes());
    }

    #[test]
    fn hfa_argument_loads_one_source_into_each_planned_vector_register() {
        let shape = omega_calling_conventions::ValueShape::homogeneous_float_aggregate(8, 2);
        let bytes = encode_host_call_sequence(
            &[Aarch64CallOperand::RuntimeHomogeneousFloatAggregate {
                byte_offset: 32,
                member_byte_count: 8,
                members: 2,
            }],
            &[ValuePlacement {
                shape,
                locations: vec![
                    ValueLocation::Register {
                        register: MachineRegister::Aarch64V(3),
                        value_byte_offset: 0,
                        byte_size: 8,
                    },
                    ValueLocation::Register {
                        register: MachineRegister::Aarch64V(4),
                        value_byte_offset: 8,
                        byte_size: 8,
                    },
                ],
            }],
        )
        .expect("fragmented HFA placement should encode");

        assert_eq!(&bytes[8..12], &encode_load_x_from_x(17, 16, 32).unwrap());
        assert_eq!(
            &bytes[12..16],
            &encode_float_move_from_gpr(8, 3, 17).unwrap()
        );
        assert_eq!(&bytes[16..20], &encode_load_x_from_x(17, 16, 40).unwrap());
        assert_eq!(
            &bytes[20..24],
            &encode_float_move_from_gpr(8, 4, 17).unwrap()
        );
        assert_eq!(&bytes[24..28], &encode_branch_link_placeholder());
        assert_eq!(
            bytes.len(),
            crate::aarch64::operand_width(&Aarch64CallOperand::RuntimeHomogeneousFloatAggregate {
                byte_offset: 32,
                member_byte_count: 8,
                members: 2,
            }) + 4
        );
    }

    #[test]
    fn hfa_stack_argument_copies_each_member_into_the_planned_area() {
        let shape = omega_calling_conventions::ValueShape::homogeneous_float_aggregate(8, 2);
        let operand = Aarch64CallOperand::RuntimeHomogeneousFloatAggregate {
            byte_offset: 32,
            member_byte_count: 8,
            members: 2,
        };
        let placement = ValuePlacement {
            shape,
            locations: vec![ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 16,
                alignment: 8,
            }],
        };
        let bytes = encode_host_call_sequence(&[operand], &[placement.clone()])
            .expect("stack-resident HFA should encode");

        assert_eq!(&bytes[0..4], &0xd100_43ffu32.to_le_bytes());
        assert_eq!(&bytes[12..16], &encode_load_x_from_x(17, 16, 32).unwrap());
        assert_eq!(&bytes[16..20], &encode_store_x_to_x(17, 31, 0).unwrap());
        assert_eq!(&bytes[20..24], &encode_load_x_from_x(17, 16, 40).unwrap());
        assert_eq!(&bytes[24..28], &encode_store_x_to_x(17, 31, 8).unwrap());
        assert_eq!(&bytes[28..32], &encode_branch_link_placeholder());
        assert_eq!(&bytes[32..36], &0x9100_43ffu32.to_le_bytes());
        assert_eq!(
            bytes.len(),
            crate::aarch64::operand_width(&operand)
                + crate::aarch64::host_call_stack_total_width_for_placements(&[placement])
                + 4
        );
    }
}

fn append_call_operands(
    bytes: &mut Vec<u8>,
    operands: impl Iterator<Item = Aarch64CallOperand>,
    argument_placements: &[ValuePlacement],
) -> Result<usize, Diagnostic> {
    let operands = operands.collect::<Vec<_>>();
    if operands.len() != argument_placements.len() {
        return Err(Diagnostic::error(format!(
            "AArch64 call plan supplied {} argument placements for {} operands",
            argument_placements.len(),
            operands.len()
        )));
    }

    let stack_bytes = argument_placements
        .iter()
        .flat_map(|placement| &placement.locations)
        .filter_map(|location| match location {
            ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } => Some(*stack_byte_offset as usize + usize::from(*byte_size)),
            ValueLocation::Register { .. } => None,
        })
        .max()
        .map(|bytes| (bytes + 15) & !15)
        .unwrap_or(0);
    append_call_stack_reserve(bytes, stack_bytes)?;

    for (index, (operand, placement)) in operands.into_iter().zip(argument_placements).enumerate() {
        if let RuntimeHomogeneousFloatAggregate {
            byte_offset,
            member_byte_count,
            members,
        } = operand
        {
            append_hfa_call_operand(
                bytes,
                byte_offset,
                member_byte_count,
                members,
                &placement.locations,
                index,
            )?;
            continue;
        }
        let [location] = placement.locations.as_slice() else {
            return Err(Diagnostic::error(format!(
                "AAPCS64 outbound scalar parameter {index} requires one location, got {:?}",
                placement.locations
            )));
        };
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                ..
            } => append_register_call_operand(bytes, operand, register)?,
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            } => {
                let offset = usize::try_from(stack_byte_offset).map_err(|_| {
                    Diagnostic::error("AAPCS64 outbound stack offset exceeds usize")
                })?;
                if matches!(operand, RuntimeScalarFloat { .. }) {
                    append_register_call_operand(bytes, operand, MachineRegister::Aarch64V(31))?;
                    bytes.extend(encode_store_float_to_sp(31, offset, byte_size)?);
                } else {
                    append_register_call_operand(bytes, operand, MachineRegister::Aarch64X(10))?;
                    match byte_size {
                        1 | 2 | 4 => {
                            bytes.extend(encode_store_w_to_x(10, 31, offset, byte_size.into())?)
                        }
                        8 => bytes.extend(encode_store_x_to_x(10, 31, offset)?),
                        _ => {
                            return Err(Diagnostic::error(format!(
                                "AAPCS64 outbound stack parameter {index} has unsupported width {byte_size}"
                            )));
                        }
                    }
                }
            }
            location => {
                return Err(Diagnostic::error(format!(
                    "AAPCS64 outbound parameter {index} has unsupported placement {location:?}"
                )));
            }
        }
    }

    Ok(stack_bytes)
}

fn append_hfa_call_operand(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    member_byte_count: usize,
    members: u8,
    locations: &[ValueLocation],
    parameter_index: usize,
) -> Result<(), Diagnostic> {
    if !matches!(member_byte_count, 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AAPCS64 outbound HFA parameter {parameter_index} has incompatible source/member placement"
        )));
    }
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if let [
        ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset: 0,
            byte_size,
            ..
        },
    ] = locations
    {
        let aggregate_byte_count = member_byte_count * usize::from(members);
        if usize::from(*byte_size) != aggregate_byte_count {
            return Err(Diagnostic::error(format!(
                "AAPCS64 outbound HFA parameter {parameter_index} stack width disagrees with its source shape"
            )));
        }
        let stack_byte_offset = usize::try_from(*stack_byte_offset)
            .map_err(|_| Diagnostic::error("AAPCS64 outbound HFA stack offset exceeds usize"))?;
        for member in 0..usize::from(members) {
            let member_offset = member * member_byte_count;
            append_load_data_from_x_offset(
                bytes,
                17,
                16,
                byte_offset.checked_add(member_offset).ok_or_else(|| {
                    Diagnostic::error("AAPCS64 outbound HFA source offset overflows usize")
                })?,
                member_byte_count,
                9,
            )?;
            let target_offset = stack_byte_offset
                .checked_add(member_offset)
                .ok_or_else(|| {
                    Diagnostic::error("AAPCS64 outbound HFA stack offset overflows usize")
                })?;
            if member_byte_count == 8 {
                bytes.extend(encode_store_x_to_x(17, 31, target_offset)?);
            } else {
                bytes.extend(encode_store_w_to_x(17, 31, target_offset, 4)?);
            }
        }
        return Ok(());
    }
    if locations.len() != usize::from(members) {
        return Err(Diagnostic::error(format!(
            "AAPCS64 outbound HFA parameter {parameter_index} has incompatible source/member placement"
        )));
    }
    for (member, location) in locations.iter().copied().enumerate() {
        let expected_offset = member * member_byte_count;
        let ValueLocation::Register {
            register: MachineRegister::Aarch64V(register),
            value_byte_offset,
            byte_size,
        } = location
        else {
            return Err(Diagnostic::error(format!(
                "AAPCS64 outbound HFA parameter {parameter_index} requires vector-register fragments, got {location:?}"
            )));
        };
        if usize::from(value_byte_offset) != expected_offset
            || usize::from(byte_size) != member_byte_count
        {
            return Err(Diagnostic::error(format!(
                "AAPCS64 outbound HFA parameter {parameter_index} member {member} disagrees with its normalized byte range"
            )));
        }
        let source_offset = byte_offset.checked_add(expected_offset).ok_or_else(|| {
            Diagnostic::error("AAPCS64 outbound HFA source offset overflows usize")
        })?;
        append_load_data_from_x_offset(bytes, 17, 16, source_offset, member_byte_count, 9)?;
        bytes.extend(encode_float_move_from_gpr(member_byte_count, register, 17)?);
    }
    Ok(())
}

fn encode_store_float_to_sp(
    source_register: u8,
    byte_offset: usize,
    byte_size: u16,
) -> Result<[u8; 4], Diagnostic> {
    let (opcode, scale) = match byte_size {
        4 => (0xbd00_0000, 4usize),
        8 => (0xfd00_0000, 8usize),
        _ => {
            return Err(Diagnostic::error(format!(
                "AAPCS64 outbound float stack width {byte_size} is unsupported"
            )));
        }
    };
    if !byte_offset.is_multiple_of(scale) || byte_offset / scale > 4095 {
        return Err(Diagnostic::error(format!(
            "AAPCS64 outbound float stack offset {byte_offset} is not encodable"
        )));
    }
    Ok(encode_instruction(
        opcode | (((byte_offset / scale) as u32) << 10) | (31 << 5) | u32::from(source_register),
    ))
}

fn append_call_stack_reserve(bytes: &mut Vec<u8>, stack_bytes: usize) -> Result<(), Diagnostic> {
    if stack_bytes == 0 {
        return Ok(());
    }
    if stack_bytes > 4095 || !stack_bytes.is_multiple_of(16) {
        return Err(Diagnostic::error(format!(
            "AAPCS64 outbound stack reservation {stack_bytes} is not directly encodable"
        )));
    }
    bytes.extend(encode_instruction(
        0xd100_0000 | ((stack_bytes as u32) << 10) | (31 << 5) | 31,
    ));
    Ok(())
}

fn append_call_stack_restore(bytes: &mut Vec<u8>, stack_bytes: usize) -> Result<(), Diagnostic> {
    if stack_bytes == 0 {
        return Ok(());
    }
    if stack_bytes > 4095 || !stack_bytes.is_multiple_of(16) {
        return Err(Diagnostic::error(format!(
            "AAPCS64 outbound stack restoration {stack_bytes} is not directly encodable"
        )));
    }
    bytes.extend(encode_instruction(
        0x9100_0000 | ((stack_bytes as u32) << 10) | (31 << 5) | 31,
    ));
    Ok(())
}

fn append_register_call_operand(
    bytes: &mut Vec<u8>,
    operand: Aarch64CallOperand,
    planned_register: MachineRegister,
) -> Result<(), Diagnostic> {
    match &operand {
        ImmediateInteger(value) => {
            append_immediate(bytes, integer_argument_register(planned_register)?, *value)?;
        }
        DataAddress { .. } => {
            let register = integer_argument_register(planned_register)?;
            bytes.extend(encode_adrp_placeholder(register));
            bytes.extend(encode_add_page_offset_placeholder(register));
        }
        RuntimeStringPointer {
            byte_offset,
            is_bounded_buffer,
        } => {
            let register = integer_argument_register(planned_register)?;
            bytes.extend(encode_adrp_placeholder(register));
            bytes.extend(encode_add_page_offset_placeholder(register));
            if *is_bounded_buffer {
                // Owned carrier: the content pointer is the COMPUTED
                // inline-bytes address `base + offset + 8`, not a stored
                // descriptor pointer. Same width as the load (12 total).
                bytes.extend(encode_add_x_immediate(register, register, byte_offset + 8)?);
            } else {
                bytes.extend(encode_load_x_from_x(register, register, *byte_offset)?);
            }
        }
        RuntimeStringLength {
            byte_offset,
            is_bounded_buffer,
        } => {
            let register = integer_argument_register(planned_register)?;
            bytes.extend(encode_adrp_placeholder(register));
            bytes.extend(encode_add_page_offset_placeholder(register));
            // Carrier length lives at offset 0 (the leading len word);
            // a descriptor's len word sits at +8 behind the pointer.
            bytes.extend(encode_load_x_from_x(
                register,
                register,
                if *is_bounded_buffer {
                    *byte_offset
                } else {
                    byte_offset + 8
                },
            )?);
        }
        RuntimePointeeStringPointer { byte_offset } => {
            let register = integer_argument_register(planned_register)?;
            bytes.extend(encode_adrp_placeholder(register));
            bytes.extend(encode_add_page_offset_placeholder(register));
            bytes.extend(encode_load_x_from_x(register, register, *byte_offset)?);
            bytes.extend(encode_load_x_from_x(register, register, 0)?);
        }
        RuntimePointeeStringLength { byte_offset } => {
            let register = integer_argument_register(planned_register)?;
            bytes.extend(encode_adrp_placeholder(register));
            bytes.extend(encode_add_page_offset_placeholder(register));
            bytes.extend(encode_load_x_from_x(register, register, *byte_offset)?);
            bytes.extend(encode_load_x_from_x(register, register, 8)?);
        }
        RuntimeScalarInteger {
            byte_offset,
            byte_count,
        } => {
            let register = integer_argument_register(planned_register)?;
            bytes.extend(encode_adrp_placeholder(register));
            bytes.extend(encode_add_page_offset_placeholder(register));
            // Load the scalar at its OWN width (LDR x for 8-byte, LDR w for <=4),
            // materializing a large field offset (a scalar declared after a big
            // array, offset > the LDR scaled-immediate range) via scratch x9 — x9
            // is not an arg register (args ride x0..x7) and is caller-saved, so the
            // already-marshalled args are untouched. `load_data_offset_width` (the
            // operand-width helper) tracks this in lockstep for both the width
            // self-check and the relocation planner.
            append_load_data_from_x_offset(
                bytes,
                register,
                register,
                *byte_offset,
                *byte_count,
                9,
            )?;
        }
        RuntimeScalarFloat {
            byte_offset,
            byte_count,
        } => {
            let register = float_argument_register(planned_register)?;
            // A float/double arg goes in the VECTOR-register sequence (v0..),
            // independent of the x-register (integer) sequence. Load the bits
            // into a scratch GPR (x16/IP0, caller-saved), then `fmov` them into
            // the next v-register. Width = adrp+add+load+fmov = 16 (one more than
            // an int scalar's 12), summed automatically so the BL/result-store
            // relocation offsets stay correct — no manual lockstep.
            bytes.extend(encode_adrp_placeholder(16));
            bytes.extend(encode_add_page_offset_placeholder(16));
            if *byte_count >= 8 {
                bytes.extend(encode_load_x_from_x(16, 16, *byte_offset)?);
            } else {
                bytes.extend(encode_load_w_from_x(16, 16, *byte_offset, *byte_count)?);
            }
            bytes.extend(encode_float_move_from_gpr(*byte_count, register, 16)?);
        }
        RuntimeHomogeneousFloatAggregate { .. } => {
            return Err(Diagnostic::error(
                "AAPCS64 HFA operands require their complete fragmented placement",
            ));
        }
        RuntimeStorageAddress { byte_offset } => {
            let register = integer_argument_register(planned_register)?;
            // The place's ADDRESS: adrp/add to the region base (relocated), then add
            // the field offset. No load — the pointer is the arg. `append_add_x_constant`
            // materializes a large offset (a field after a big array, offset > 4095)
            // via scratch x9 (not an arg register); `add_constant_width` tracks it in
            // lockstep for the width self-check + the relocation planner.
            bytes.extend(encode_adrp_placeholder(register));
            bytes.extend(encode_add_page_offset_placeholder(register));
            append_add_x_constant(bytes, register, register, *byte_offset, 9)?;
        }
        ByteLength(value) => {
            append_unsigned_immediate(
                bytes,
                integer_argument_register(planned_register)?,
                *value as u64,
            );
        }
    }

    Ok(())
}

fn integer_argument_register(register: MachineRegister) -> Result<u8, Diagnostic> {
    let MachineRegister::Aarch64X(register) = register else {
        return Err(Diagnostic::error(format!(
            "AArch64 call plan selected non-GPR register {register:?} for an integer argument"
        )));
    };
    Ok(register)
}

fn float_argument_register(register: MachineRegister) -> Result<u8, Diagnostic> {
    let MachineRegister::Aarch64V(register) = register else {
        return Err(Diagnostic::error(format!(
            "AArch64 call plan selected non-vector register {register:?} for a float argument"
        )));
    };
    Ok(register)
}
