use crate::Aarch64CallOperand;
use crate::Aarch64CallOperand::*;
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

pub fn encode_host_call_sequence(operands: &[Aarch64CallOperand]) -> Result<Vec<u8>, Diagnostic> {
    encode_host_call_sequence_from_operands(operands.iter().copied())
}

pub fn encode_host_call_sequence_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(host_call_sequence_width_from_operands(operands.clone()));
    append_call_operands(&mut bytes, operands)?;
    bytes.extend(encode_branch_link_placeholder());
    Ok(bytes)
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
    append_call_operands(&mut bytes, args.iter().copied())?;
    bytes.extend(encode_branch_link_placeholder());
    // Result store: x16 <- result region base (adrp/add relocated), then store
    // the return register at the field's offset.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize a large result-field offset (a scalar after a big array) via scratch
    // x17 (caller-saved; x0 holds the result, x16 the region base). Kept in lockstep
    // with operand_width's store_data_offset_width accounting + the relocation planner.
    append_store_data_to_x_offset(&mut bytes, 0, 16, byte_offset, byte_count, 17)?;
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
    append_call_operands(&mut bytes, args.iter().copied())?;
    bytes.extend(encode_branch_link_placeholder());
    // Deref the returned pointer: `ldr w0, [x0]` (load errno through &errno).
    bytes.extend(encode_instruction(0xB940_0000));
    // Result store: x16 <- result region base (adrp/add relocated), then store.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize a large result-field offset (a scalar after a big array) via scratch
    // x17 (caller-saved; x0 holds the result, x16 the region base). Kept in lockstep
    // with operand_width's store_data_offset_width accounting + the relocation planner.
    append_store_data_to_x_offset(&mut bytes, 0, 16, byte_offset, byte_count, 17)?;
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
    append_call_operands(&mut bytes, args.iter().copied())?;
    bytes.extend(encode_branch_link_placeholder());
    // Move the float return (`d0`/`s0`) into `x0` so the integer result-store can
    // spill the raw bits: `fmov x0, d0` (double) / `fmov w0, s0` (single).
    bytes.extend(encode_float_move_to_gpr(byte_count.max(4), 0, 0)?);
    // Result store: x16 <- result region base (adrp/add relocated), then store.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize a large result-field offset (a scalar after a big array) via scratch
    // x17 (caller-saved; x0 holds the result, x16 the region base). Kept in lockstep
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
    append_call_operands(&mut bytes, register_args.iter().copied())?;
    // `mode` -> [sp,#0]: reserve a 16-byte-aligned slot, materialize into w9, store.
    bytes.extend(encode_instruction(0xD100_43FF)); // sub sp, sp, #16
    append_immediate(&mut bytes, 9, mode)?; // movz w9, #mode (+ movk if wide)
    bytes.extend(encode_instruction(0xB900_03E9)); // str w9, [sp]
    bytes.extend(encode_branch_link_placeholder());
    bytes.extend(encode_instruction(0x9100_43FF)); // add sp, sp, #16
    // Result store: x16 <- result region base (adrp/add relocated), then store.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize a large result-field offset (a scalar after a big array) via scratch
    // x17 (caller-saved; x0 holds the result, x16 the region base). Kept in lockstep
    // with operand_width's store_data_offset_width accounting + the relocation planner.
    append_store_data_to_x_offset(&mut bytes, 0, 16, byte_offset, byte_count, 17)?;
    Ok(bytes)
}

pub fn encode_syscall_sequence(
    operands: &[Aarch64CallOperand],
    syscall_number: u32,
    number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    encode_syscall_sequence_from_operands(
        operands.iter().copied(),
        syscall_number,
        number_register,
        supervisor_call,
    )
}

pub fn encode_syscall_sequence_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    syscall_number: u32,
    number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(syscall_sequence_width_from_operands(
        operands.clone(),
        syscall_number,
    ));
    append_call_operands(&mut bytes, operands)?;
    append_unsigned_immediate(&mut bytes, number_register, u64::from(syscall_number));
    bytes.extend(encode_svc(supervisor_call));
    Ok(bytes)
}

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

pub fn encode_return_register_integer_write_bytes(
    byte_size: usize,
    value: i64,
) -> Result<[u8; 4], Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_size}-byte return integers yet"
        )));
    }
    let immediate = u16::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write return integer `{value}` yet"
        ))
    })?;
    Ok(if byte_size == 8 {
        encode_movz(0, immediate)
    } else {
        encode_movz_w(0, immediate)
    })
}

fn append_call_operands(
    bytes: &mut Vec<u8>,
    operands: impl Iterator<Item = Aarch64CallOperand>,
) -> Result<(), Diagnostic> {
    let mut next_register = 0u8;
    // arm64 passes float/double args in the SEPARATE v0.. vector-register sequence.
    let mut next_vreg = 0u8;

    for operand in operands {
        match &operand {
            ImmediateInteger(value) => {
                append_immediate(bytes, next_register, *value)?;
                next_register += 1;
            }
            DataAddress { .. } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                next_register += 1;
            }
            RuntimeStringPointer { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                next_register += 1;
            }
            RuntimeStringLength { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    byte_offset + 8,
                )?);
                next_register += 1;
            }
            RuntimePointeeStringPointer { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                bytes.extend(encode_load_x_from_x(next_register, next_register, 0)?);
                next_register += 1;
            }
            RuntimePointeeStringLength { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                bytes.extend(encode_load_x_from_x(next_register, next_register, 8)?);
                next_register += 1;
            }
            RuntimeScalarInteger {
                byte_offset,
                byte_count,
            } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                // Load the scalar at its OWN width (LDR x for 8-byte, LDR w for <=4),
                // materializing a large field offset (a scalar declared after a big
                // array, offset > the LDR scaled-immediate range) via scratch x9 — x9
                // is not an arg register (args ride x0..x7) and is caller-saved, so the
                // already-marshalled args are untouched. `load_data_offset_width` (the
                // operand-width helper) tracks this in lockstep for both the width
                // self-check and the relocation planner.
                append_load_data_from_x_offset(
                    bytes,
                    next_register,
                    next_register,
                    *byte_offset,
                    *byte_count,
                    9,
                )?;
                next_register += 1;
            }
            RuntimeScalarFloat {
                byte_offset,
                byte_count,
            } => {
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
                bytes.extend(encode_float_move_from_gpr(*byte_count, next_vreg, 16)?);
                next_vreg += 1;
            }
            RuntimeStorageAddress { byte_offset } => {
                // The place's ADDRESS: adrp/add to the region base (relocated), then add
                // the field offset. No load — the pointer is the arg. `append_add_x_constant`
                // materializes a large offset (a field after a big array, offset > 4095)
                // via scratch x9 (not an arg register); `add_constant_width` tracks it in
                // lockstep for the width self-check + the relocation planner.
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                append_add_x_constant(bytes, next_register, next_register, *byte_offset, 9)?;
                next_register += 1;
            }
            ByteLength(value) => {
                append_unsigned_immediate(bytes, next_register, *value as u64);
                next_register += 1;
            }
        }
    }

    Ok(())
}
