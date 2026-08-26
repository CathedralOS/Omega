use omega_target_operations::{RuntimeValueOperandHandle, RuntimeValueOperandSource};
use psi_diagnostics::Diagnostic;

use super::{
    RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS, RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
    append_add_constant_to_x_register, append_runtime_value_operand,
};
use crate::aarch64::primitives::{
    append_add_x_constant, encode_add_page_offset_placeholder, encode_adrp_placeholder,
    encode_atomic_load, encode_atomic_store, encode_cas, encode_ldadd, encode_ldclr, encode_ldeor,
    encode_ldset, encode_mvn_register, encode_store_w_to_x, encode_store_x_to_x,
    encode_sub_w_register, encode_sub_x_register, encode_swp,
};
use crate::aarch64::widths::{add_constant_width, runtime_value_operand_width};

pub fn encode_atomic_load_to_storage(
    source_offset: usize,
    byte_size: usize,
    result_offset: usize,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_load_to_storage_width(
        source_offset,
        byte_size,
        result_offset,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    bytes.extend(encode_atomic_load(17, 16, byte_size, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(17, 16, 0)?),
        _ => unreachable!("atomic-load width validation accepts only 1, 2, 4, or 8 bytes"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_load_to_storage_width(source_offset, byte_size, result_offset)
    );
    Ok(bytes)
}

pub fn runtime_atomic_load_to_storage_width(
    source_offset: usize,
    _byte_size: usize,
    result_offset: usize,
) -> usize {
    8 + add_constant_width(source_offset) + 4 + 8 + add_constant_width(result_offset) + 4
}

pub fn runtime_atomic_load_result_address_offset(source_offset: usize) -> usize {
    8 + add_constant_width(source_offset) + 4
}

pub fn encode_atomic_store_from_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_store_from_operand_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        value,
    )?;
    append_add_constant_to_x_register(&mut bytes, 16, target_offset)?;
    bytes.extend(encode_atomic_store(17, 16, byte_size, ordering)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_store_from_operand_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_store_from_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    8 + runtime_value_operand_width(runtime_value_operands, value)
        + add_constant_width(target_offset)
        + 4
}

/// AArch64 atomic `fetch_add` via the ordering-selected LSE `LDADD*` form.
/// The single RMW returns its observed prior in x26, which is then stored into
/// the language result place.
/// (An earlier fence-era comment here claimed this was unimplemented; the
/// LDADDAL path is live and pinned by tests/canaries/pass/atomics on arm64 hosts.)
pub fn encode_atomic_fetch_add(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        // The field address is `base + target_offset`; a single ADD immediate
        // only reaches 4095. Atomic fields sit at small offsets in practice;
        // a clear error beats a silent miscompile.
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_add target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_add_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        delta,
    ));
    // x16 = the atomic field's storage-region base, relocated at the instruction
    // start (same adrp/add convention as the binary write, so the shared
    // relocation record patches it). The `delta` operand is loaded NEXT, at the
    // binary-write left-operand offset (8), so its relocations land correctly;
    // the address ADD comes AFTER so it never shifts the operand's position.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        delta,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    // LDADD* w17/x17, w26/x26, [x16] -- prior returned in x26.
    bytes.extend(encode_ldadd(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("LDADD width validation accepts only 1, 2, 4, or 8 bytes"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_add_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_add_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    // adrp + add-page-offset (8) + delta operand load + the address ADD (0 when
    // the offset is 0, else 4) + the single LDADDAL (4).
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, delta)
        + address_add
        + 4
        + 8
        + add_constant_width(result_offset)
        + 4
}

pub fn runtime_atomic_fetch_add_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, delta) + address_add + 4
}

pub fn encode_atomic_fetch_sub(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_sub target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_sub_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        delta,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        delta,
    )?;
    // LDADD of the width-truncated two's-complement negation implements
    // wrapping fetch_sub while preserving the instruction-observed prior.
    bytes.extend(match byte_size {
        1 | 2 | 4 => encode_sub_w_register(17, 31, 17),
        8 => encode_sub_x_register(17, 31, 17),
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 atomic fetch_sub cannot encode a {other}-byte width"
            )));
        }
    });
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_ldadd(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("fetch_sub width validated before LDADD"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_sub_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_sub_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_width(
        runtime_value_operands,
        target_offset,
        0,
        result_offset,
        delta,
    ) + 4
}

pub fn runtime_atomic_fetch_sub_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_result_address_offset(runtime_value_operands, target_offset, delta) + 4
}

pub fn encode_atomic_fetch_xor(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_xor target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_xor_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        value,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_ldeor(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("fetch_xor width validated before LDEOR"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_xor_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_xor_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    )
}

pub fn runtime_atomic_fetch_xor_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_result_address_offset(runtime_value_operands, target_offset, value)
}

pub fn encode_atomic_fetch_or(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_or target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_or_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        value,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_ldset(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("fetch_or width validated before LDSET"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_or_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_or_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    )
}

pub fn runtime_atomic_fetch_or_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_result_address_offset(runtime_value_operands, target_offset, value)
}

pub fn encode_atomic_fetch_and(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_and target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_and_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        value,
    )?;
    bytes.extend(encode_mvn_register(byte_size, 17, 17)?);
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_ldclr(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("fetch_and width validated before LDCLR"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_and_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_and_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_or_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    ) + 4
}

pub fn runtime_atomic_fetch_and_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_or_result_address_offset(runtime_value_operands, target_offset, value) + 4
}

pub fn encode_atomic_swap(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic swap target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_swap_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        new_value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        new_value,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_swp(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("SWP width validation accepts only 1, 2, 4, or 8 bytes"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_swap_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            new_value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_swap_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, new_value)
        + address_add
        + 4
        + 8
        + add_constant_width(result_offset)
        + 4
}

pub fn runtime_atomic_swap_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, new_value) + address_add + 4
}

pub fn encode_atomic_compare_exchange(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
    success_ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic compare_exchange target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_compare_exchange_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        expected,
        new_value,
    ));
    // x16 = the atomic field's region base (relocated at the instruction start).
    // new_value loads FIRST at offset 8 (the binary-write left-operand offset, so
    // its relocations land correctly), then expected; the address ADD comes after
    // so it never shifts the operand positions. CAS* clobbers x26 (expected ->
    // prior) and stores x17 (new_value) only on a match.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        new_value,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        26,
        RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
        expected,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    // CASAL Ws=x26 (expected), Wt=x17 (new_value), [x16].
    bytes.extend(encode_cas(byte_size, 26, 17, 16, success_ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("CAS width validation accepts only 1, 2, 4, or 8 bytes"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_compare_exchange_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            expected,
            new_value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_compare_exchange_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    // adrp + add-page-offset (8) + new_value load + expected load + the address
    // ADD (0 when offset is 0, else 4) + the single CASAL (4).
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, new_value)
        + runtime_value_operand_width(runtime_value_operands, expected)
        + address_add
        + 4
        + 8
        + add_constant_width(result_offset)
        + 4
}

pub fn runtime_atomic_compare_exchange_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, new_value)
        + runtime_value_operand_width(runtime_value_operands, expected)
        + address_add
        + 4
}
