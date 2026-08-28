use psi_diagnostics::Diagnostic;

#[cfg(test)]
use omega_calling_conventions::{MachineRegister, RegisterSet};
#[cfg(test)]
use omega_target_operations::StateGuardOperator;

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate, append_unsigned_immediate_padded,
    encode_add_page_offset_placeholder, encode_add_x_immediate, encode_add_x_register,
    encode_adrp_placeholder, encode_load_w_from_x, encode_load_x_from_x, encode_move_x_register,
    encode_movz, encode_mul_x_register, encode_store_w_to_x, encode_store_x_to_x,
};
#[cfg(test)]
use super::primitives::{
    encode_atomic_load, encode_atomic_store, encode_float_add, encode_float_divide,
    encode_float_fused_multiply_add, encode_float_multiply, encode_float_sqrt,
    encode_float_subtract, encode_ldadd, encode_read_fpcr, encode_swp, encode_write_fpcr,
};
#[cfg(test)]
use super::widths::{
    runtime_frame_indexed_string_write_width_with_index_region,
    runtime_machine_indexed_string_write_width_with_index_region,
};

// x18 is NEVER used as a scratch register: it is the reserved platform register
// on Darwin arm64 and the kernel zeroes it on every kernel->user return, so any
// value held in x18 across an interrupt window is silently lost (this corrupted
// dungeon-crawler frame-slot copies nondeterministically). x26 takes its place.
const RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS: &[u8] = &[26, 15, 14, 13, 12, 11, 10, 9];
const RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS: &[u8] = &[15, 14, 13, 12, 11, 10, 9];

mod atomics;
pub use atomics::*;

mod conversion;
pub use conversion::*;

mod comparison;
pub use comparison::*;

mod scalar_writes;
pub use scalar_writes::*;

mod string_writes;
pub use string_writes::*;

mod bounded_buffers;
pub use bounded_buffers::*;

mod address_writes;
pub use address_writes::*;

mod indexed_writes;
pub use indexed_writes::*;

mod storage_copies;
pub use storage_copies::*;

mod runtime_values;
#[cfg(test)]
use runtime_values::float_policy_guard_bytes;
pub(in crate::aarch64) use runtime_values::*;
use runtime_values::{
    append_runtime_binary_operation, append_runtime_binary_operation_with_domain,
    append_runtime_float_binary_operation, append_runtime_value_operand,
    append_shift_count_trap_guard,
};

pub(in crate::aarch64) fn append_runtime_frame_index_target_address_with_index_region(
    bytes: &mut Vec<u8>,
    address_register: u8,
    index_region: omega_target_operations::RuntimeStorageRegion,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    append_runtime_frame_index_target_address_with_index_width(
        bytes,
        address_register,
        index_region,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        index_scratch,
        scale_scratch,
    )
}

fn append_runtime_frame_index_target_address_with_index_width(
    bytes: &mut Vec<u8>,
    address_register: u8,
    index_region: omega_target_operations::RuntimeStorageRegion,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_fixed_width_load_x_from_x_offset(bytes, address_register, 20, descriptor_offset, 19);
    let index_base = if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
        // The fixed-width index load below uses x21 as its offset scratch.
        // Prefer x15 for the machine base, except when the caller already
        // holds the pointee address there (runtime value operands do); x19 is
        // free after the descriptor load and is excluded from caller picks.
        let machine_base = if address_register == 15 { 19 } else { 15 };
        bytes.extend(encode_adrp_placeholder(machine_base)); // machine base [reloc @ 32]
        bytes.extend(encode_add_page_offset_placeholder(machine_base));
        machine_base
    } else {
        20
    };
    append_fixed_width_load_unsigned_index_from_x_offset(
        bytes,
        index_scratch,
        index_base,
        index_offset,
        index_byte_size,
        21,
    );
    append_scale_x_register_by_constant(bytes, scale_scratch, index_scratch, element_byte_size)?;
    bytes.extend(encode_add_x_register(
        address_register,
        address_register,
        scale_scratch,
    ));
    append_add_constant_to_x_register(bytes, address_register, field_byte_offset)?;
    Ok(())
}

fn append_runtime_frame_fixed_index_target_address(
    bytes: &mut Vec<u8>,
    address_register: u8,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    let scaled_index = element_index
        .checked_mul(element_byte_size)
        .ok_or_else(|| {
            Diagnostic::error(
                "AArch64 MVP encoder cannot address overflowing fixed indexed operand",
            )
        })?;
    let byte_offset = scaled_index.checked_add(field_byte_offset).ok_or_else(|| {
        Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed operand")
    })?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_load_data_from_x_offset(bytes, address_register, 20, descriptor_offset, 8, 19)?;
    append_add_constant_to_x_register(bytes, address_register, byte_offset)?;
    Ok(())
}

/// Fixed-shape element-address recipe for the dual/double-indexed encoders:
/// unlike `append_runtime_machine_index_target_address` (whose adds/loads are
/// value-dependent in width), every element here is a fixed 4-byte instruction,
/// so the positions of the relocated `adrp` pairs are REGION-DEPENDENT
/// CONSTANTS -- which is what the relocation-offset helpers (which only see the
/// regions, never the offsets) require. Shape:
///
///   mov  x20, <base>                      (4)  index base default
///   [frame index: adrp/add x20]           (8)  frame base (RELOCATED)
///   ldr{b,h,w,x} x17, [x20, #index_offset] (4) exact selected-width index
///   movz x26, #element_byte_size          (4)
///   mul  x26, x17, x26                    (4)
///   add  <base>, <base>, x26              (4)
///   add  <base>, <base>, #(array + field) (4)  unconditional (#0 is valid)
///
/// The index offset must fit the LDR scaled immediate and the combined
/// array+field offset must fit the ADD immediate; both fail LOUDLY otherwise.
fn append_fixed_shape_index_element_address(
    bytes: &mut Vec<u8>,
    base_register: u8,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    combined_byte_offset: usize,
) -> Result<(), Diagnostic> {
    if element_byte_size > 0xffff {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot scale a runtime index by element size \
             `{element_byte_size}` yet"
        )));
    }
    bytes.extend(encode_move_x_register(20, base_register));
    if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        bytes.extend(encode_adrp_placeholder(20));
        bytes.extend(encode_add_page_offset_placeholder(20));
    }
    append_direct_unsigned_index_load(bytes, 17, 20, index_offset, index_byte_size)?;
    bytes.extend(encode_movz(26, element_byte_size as u16));
    bytes.extend(encode_mul_x_register(26, 17, 26));
    bytes.extend(encode_add_x_register(base_register, base_register, 26));
    bytes.extend(encode_add_x_immediate(
        base_register,
        base_register,
        combined_byte_offset,
    )?);
    Ok(())
}

fn append_single_index_address_math(
    bytes: &mut Vec<u8>,
    index_base_register: u8,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    combined_byte_offset: usize,
) -> Result<(), Diagnostic> {
    if element_byte_size > 0xffff {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot scale a runtime index by element size `{element_byte_size}` yet"
        )));
    }
    append_direct_unsigned_index_load(
        bytes,
        17,
        index_base_register,
        index_offset,
        index_byte_size,
    )?;
    bytes.extend(encode_movz(26, element_byte_size as u16));
    bytes.extend(encode_mul_x_register(26, 17, 26));
    bytes.extend(encode_add_x_register(16, 16, 26));
    bytes.extend(encode_add_x_immediate(16, 16, combined_byte_offset)?);
    Ok(())
}

/// Fixed-shape double-index address math (36 bytes, nine 4-byte instructions):
/// after the caller has materialized the machine base into x16 (and, when any
/// index is frame-resident, the frame base into x15), walk x16 to the element
/// `base + outer*outer_stride + inner*inner_stride + combined_offset`. Every
/// element is fixed width so the relocated adrp positions around it are
/// constants. Clobbers x14/x17/x26.
#[allow(clippy::too_many_arguments)]
pub(in crate::aarch64) fn append_double_index_address_math(
    bytes: &mut Vec<u8>,
    outer_base_register: u8,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_base_register: u8,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    combined_byte_offset: usize,
) -> Result<(), Diagnostic> {
    for stride in [outer_stride, inner_stride] {
        if stride > 0xffff {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot scale a double index by stride `{stride}` yet"
            )));
        }
    }
    append_direct_unsigned_index_load(
        bytes,
        17,
        outer_base_register,
        outer_index_offset,
        outer_index_byte_size,
    )?;
    append_direct_unsigned_index_load(
        bytes,
        26,
        inner_base_register,
        inner_index_offset,
        inner_index_byte_size,
    )?;
    bytes.extend(encode_movz(14, outer_stride as u16));
    bytes.extend(encode_mul_x_register(17, 17, 14));
    bytes.extend(encode_movz(14, inner_stride as u16));
    bytes.extend(encode_mul_x_register(26, 26, 14));
    bytes.extend(encode_add_x_register(16, 16, 17));
    bytes.extend(encode_add_x_register(16, 16, 26));
    bytes.extend(encode_add_x_immediate(16, 16, combined_byte_offset)?);
    Ok(())
}

fn append_direct_unsigned_index_load(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
            destination_register,
            base_register,
            byte_offset,
            byte_size,
        )?),
        8 => bytes.extend(encode_load_x_from_x(
            destination_register,
            base_register,
            byte_offset,
        )?),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot load {byte_size}-byte runtime indexes yet"
            )));
        }
    }
    Ok(())
}

/// Materialize the double-indexed bases: the machine base into x16 (relocated
/// at instruction start) and -- when any index is frame-resident -- the SHARED
/// frame base into x15 (relocated at the constant
/// `runtime_machine_double_indexed_frame_base_offset` = 8). Returns the
/// (outer_base, inner_base) index-load registers.
fn append_double_index_bases(
    bytes: &mut Vec<u8>,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> (u8, u8) {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if outer_index_region == frame || inner_index_region == frame {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    (
        if outer_index_region == frame { 15 } else { 16 },
        if inner_index_region == frame { 15 } else { 16 },
    )
}

/// Read `grid[i][j]` (both indices runtime) into a storage slot: the
/// double-index address math walks x16 to the element, the element loads into
/// x17, then a second relocated base pair addresses the target region for the
/// store. Historically silently dropped on aarch64 (the zero-width hole).
#[allow(clippy::too_many_arguments)]
fn append_runtime_machine_index_target_address(
    bytes: &mut Vec<u8>,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    append_add_constant_to_x_register(bytes, 16, base_byte_offset)?;
    // Load the index at its declared width and zero-extend narrow forms so
    // adjacent slot bytes cannot be spliced into the address. `append_load_data_from_x_offset`
    // materializes a large `index_offset` (a loop counter declared AFTER a big array,
    // offset > 16380) into scratch x19 — it moves the base (x20) into x19 first, so
    // x20 is preserved. Its width is `machine_index_load_width(index_region,
    // index_offset)`, which the width + relocation-address-offset functions consume in
    // lockstep so the source/target adrp positions stay exact for large offsets.
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            bytes.extend(encode_adrp_placeholder(20));
            bytes.extend(encode_add_page_offset_placeholder(20));
            append_load_data_from_x_offset(bytes, 17, 20, index_offset, index_byte_size, 19)?;
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            append_load_data_from_x_offset(bytes, 17, 20, index_offset, index_byte_size, 19)?;
        }
    }
    append_scale_x_register_by_constant(bytes, 26, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 26));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

/// `index_scratch`/`scale_scratch` are parameters because this runs in TWO
/// register climates: write-TARGET address setup (pre-operands; 17/26 are
/// free -- the historical hardcodes) and OPERAND-position evaluation, where
/// hardcoded 17 CLOBBERED the left operand's result while addressing the
/// right one (`self.double(arr[i])` doubled the INDEX: d = i + arr[i] -- the
/// local-array value-operand ZII/garbage divergence; x86_64 is immune because
/// it stashes the left result on the stack).
pub(in crate::aarch64) fn append_runtime_frame_base_index_target_address(
    bytes: &mut Vec<u8>,
    address_register: u8,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    append_runtime_frame_base_index_target_address_with_index_width(
        bytes,
        address_register,
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        index_scratch,
        scale_scratch,
    )
}

fn append_runtime_frame_base_index_target_address_with_index_width(
    bytes: &mut Vec<u8>,
    address_register: u8,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(address_register, 20));
    append_add_constant_to_x_register(bytes, address_register, base_byte_offset)?;
    append_load_data_from_x_offset(bytes, index_scratch, 20, index_offset, index_byte_size, 19)?;
    append_scale_x_register_by_constant(bytes, scale_scratch, index_scratch, element_byte_size)?;
    bytes.extend(encode_add_x_register(
        address_register,
        address_register,
        scale_scratch,
    ));
    append_add_constant_to_x_register(bytes, address_register, field_byte_offset)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_runtime_frame_base_index_target_address_with_index_region(
    bytes: &mut Vec<u8>,
    address_register: u8,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(address_register, 20));
    append_add_constant_to_x_register(bytes, address_register, base_byte_offset)?;
    let index_base = if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        20
    } else {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
        15
    };
    append_load_data_from_x_offset(
        bytes,
        index_scratch,
        index_base,
        index_offset,
        index_byte_size,
        19,
    )?;
    append_scale_x_register_by_constant(bytes, scale_scratch, index_scratch, element_byte_size)?;
    bytes.extend(encode_add_x_register(
        address_register,
        address_register,
        scale_scratch,
    ));
    append_add_constant_to_x_register(bytes, address_register, field_byte_offset)?;
    Ok(())
}

fn append_runtime_storage_load(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    context: &str,
) -> Result<(), Diagnostic> {
    if byte_offset > 0 {
        append_add_constant_to_x_register(bytes, base_register, byte_offset)?;
    }

    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
            destination_register,
            base_register,
            0,
            byte_size,
        )?),
        8 => bytes.extend(encode_load_x_from_x(
            destination_register,
            base_register,
            0,
        )?),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot load {context} width `{byte_size}` yet"
            )));
        }
    }

    Ok(())
}

/// Run `destination OP right` on the values already materialized in the GPRs,
/// leaving the result in `destination_register`.
///
/// `byte_size` is the OPERAND width (see `runtime_binary_operation_byte_size`):
/// signedness-sensitive operations (division, arithmetic right shift, min/max,
/// ordered comparisons) run in the 32-bit `W` forms when the operands are 4
/// bytes or narrower, so an i32 sign bit loaded zero-extended is honored —
/// mirroring how the x86_64 backend sizes `idiv`/`sar`/`cmp` to the operands.
/// Every arm emits the same byte count for either width, so
/// `runtime_binary_operation_width` stays width-independent.
/// Narrow SIGNED divide/modulo operands may arrive ZERO-extended (the
/// guard-subject load path), so a 32-bit `sdiv` would divide i8 -20 as 236.
/// Sign-extend both to the operation width first -- idempotent when they are
/// already sign-extended (the storage-write path); unsigned division is
/// correct zero-extended and skips this. Mirrors the x86_64
/// `append_integer_divide_modulo_core` fix.
fn append_runtime_storage_result_write(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    match byte_size {
        1 | 2 | 4 | 8 => append_store_data_to_x_offset(bytes, 17, 16, byte_offset, byte_size, 19)?,
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot write {byte_size}-byte runtime storage results yet"
            )));
        }
    }

    Ok(())
}

fn append_scale_x_register_by_constant(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    source_register: u8,
    scale: usize,
) -> Result<(), Diagnostic> {
    if scale == 0 {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder cannot scale indexed runtime storage by zero",
        ));
    }

    append_unsigned_immediate(bytes, destination_register, 0);
    let working_register = 19u8;
    bytes.extend(encode_move_x_register(working_register, source_register));

    let highest_bit = usize::BITS - scale.leading_zeros();
    for bit_index in 0..highest_bit {
        if (scale >> bit_index) & 1 == 1 {
            bytes.extend(encode_add_x_register(
                destination_register,
                destination_register,
                working_register,
            ));
        }

        if bit_index + 1 < highest_bit {
            bytes.extend(encode_add_x_register(
                working_register,
                working_register,
                working_register,
            ));
        }
    }

    Ok(())
}

fn append_add_constant_to_x_register(
    bytes: &mut Vec<u8>,
    register: u8,
    value: usize,
) -> Result<(), Diagnostic> {
    let scratch_register = if register == 19 { 26 } else { 19 };
    append_add_x_constant(bytes, register, register, value, scratch_register)
}

fn append_store_x_to_x_offset(
    bytes: &mut Vec<u8>,
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    if byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095 {
        bytes.extend(encode_store_x_to_x(
            source_register,
            base_register,
            byte_offset,
        )?);
    } else {
        append_add_constant_to_x_register(bytes, base_register, byte_offset)?;
        bytes.extend(encode_store_x_to_x(source_register, base_register, 0)?);
    }

    Ok(())
}

pub(in crate::aarch64) fn append_load_data_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if data_offset_encodable(byte_offset, byte_size) {
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                base_register,
                byte_offset,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(
                destination_register,
                base_register,
                byte_offset,
            )?),
            _ => unreachable!("runtime data loads are 1, 2, 4, or 8 bytes"),
        }
    } else {
        bytes.extend(encode_move_x_register(scratch_register, base_register));
        if scratch_register == base_register || byte_offset <= 4095 {
            append_add_constant_to_x_register(bytes, scratch_register, byte_offset)?;
        } else {
            // Preserve the historical leading-move width, but keep the actual
            // address formation inside the caller-supplied scratch contract.
            // The base is still intact, so the scratch may hold the constant
            // directly; no hidden x19/x26 register enters boundary marshalling.
            append_unsigned_immediate(bytes, scratch_register, byte_offset as u64);
            bytes.extend(encode_add_x_register(
                scratch_register,
                base_register,
                scratch_register,
            ));
        }
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                scratch_register,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(
                destination_register,
                scratch_register,
                0,
            )?),
            _ => unreachable!("runtime data loads are 1, 2, 4, or 8 bytes"),
        }
    }

    Ok(())
}

pub(in crate::aarch64) fn append_store_data_to_x_offset(
    bytes: &mut Vec<u8>,
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if data_offset_encodable(byte_offset, byte_size) {
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_store_w_to_x(
                source_register,
                base_register,
                byte_offset,
                byte_size,
            )?),
            8 => bytes.extend(encode_store_x_to_x(
                source_register,
                base_register,
                byte_offset,
            )?),
            _ => unreachable!("runtime data stores are 1, 2, 4, or 8 bytes"),
        }
    } else {
        bytes.extend(encode_move_x_register(scratch_register, base_register));
        append_add_constant_to_x_register(bytes, scratch_register, byte_offset)?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_store_w_to_x(
                source_register,
                scratch_register,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_store_x_to_x(source_register, scratch_register, 0)?),
            _ => unreachable!("runtime data stores are 1, 2, 4, or 8 bytes"),
        }
    }

    Ok(())
}

fn append_fixed_width_load_x_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) {
    append_unsigned_immediate_padded(bytes, scratch_register, byte_offset as u64);
    bytes.extend(encode_add_x_register(
        scratch_register,
        base_register,
        scratch_register,
    ));
    bytes.extend(
        encode_load_x_from_x(destination_register, scratch_register, 0)
            .expect("zero-offset x-register load should always encode"),
    );
}

/// Compute `base + byte_offset` in the same fixed 24-byte envelope as
/// `append_fixed_width_load_x_from_x_offset`. Carrier text equality needs an
/// inline byte address where descriptor equality performs a pointer load; the
/// padded self-move keeps relocation offsets and operand widths identical.
fn append_fixed_width_address_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) {
    append_unsigned_immediate_padded(bytes, scratch_register, byte_offset as u64);
    bytes.extend(encode_add_x_register(
        destination_register,
        base_register,
        scratch_register,
    ));
    bytes.extend(encode_move_x_register(
        destination_register,
        destination_register,
    ));
}

/// Loads an unsigned array index at its declared width. Narrow loads target a
/// W register and therefore zero-extend into the full X register; an 8-byte
/// index uses the corresponding X load.
///
/// Emits the SAME 24-byte sequence as `append_fixed_width_load_x_from_x_offset`
/// (padded 4-instruction immediate = 16 bytes, ADD = 4, load = 4) — only the
/// final load differs (`LDR Wt` vs `LDR Xt`, both 4 bytes) — so width functions
/// are unchanged.
fn append_fixed_width_load_unsigned_index_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    scratch_register: u8,
) {
    append_unsigned_immediate_padded(bytes, scratch_register, byte_offset as u64);
    bytes.extend(encode_add_x_register(
        scratch_register,
        base_register,
        scratch_register,
    ));
    match byte_size {
        1 | 2 | 4 => bytes.extend(
            encode_load_w_from_x(destination_register, scratch_register, 0, byte_size)
                .expect("zero-offset w-register load should always encode"),
        ),
        8 => bytes.extend(
            encode_load_x_from_x(destination_register, scratch_register, 0)
                .expect("zero-offset x-register load should always encode"),
        ),
        _ => unreachable!("validated index width"),
    }
}

fn data_offset_encodable(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        2 => byte_offset.is_multiple_of(2) && byte_offset / 2 <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}

pub(in crate::aarch64) fn data_offset_uses_scratch(byte_offset: usize, byte_size: usize) -> bool {
    !data_offset_encodable(byte_offset, byte_size)
}

#[cfg(test)]
#[path = "runtime_storage_tests.rs"]
mod tests;
