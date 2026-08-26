use super::super::{
    Reg64, append_load_rax_from_r15_width, append_load_unsigned_reg_from_r15, append_mov_r15_imm64,
    append_mov_rax_imm64, append_mov_rcx_imm64, append_store_r11_to_r15_width, place_copy,
    store_width, unsigned_load_width,
};
use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

pub fn runtime_machine_integer_write_width(_byte_offset: usize, byte_size: usize) -> usize {
    // mov r15,imm64 (10) + mov rax,imm64 (10) + store [r15+disp32] (7; 8 with
    // the 0x66 prefix for a 2-byte store).
    if byte_size == 2 { 28 } else { 27 }
}

pub fn encode_runtime_machine_integer_write(
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    // Write rung 1b: DELEGATES byte-for-byte to the place materializer
    // (unit-pinned identity). The region on the transitional place is
    // documentation only -- a direct place's bytes never consult it; the
    // walker patches the base from the instruction's own region.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(byte_offset))
            .expect("a direct place is two steps, within PLACE_MAX_STEPS");
    place_copy::encode_place_integer_write(&target, value, byte_size).map(|(bytes, _)| bytes)
}

fn bit_fragment_container_bytes(
    fragment: &omega_target_operations::RuntimeBitFieldFragment,
) -> Result<usize, Diagnostic> {
    match fragment.container_width_bits {
        8 => Ok(1),
        16 => Ok(2),
        32 => Ok(4),
        64 => Ok(8),
        width => Err(Diagnostic::error(format!(
            "X86_64 bit-field container width `{width}` is not 8, 16, 32, or 64"
        ))),
    }
}

fn bit_width_mask(width: u16) -> Result<u64, Diagnostic> {
    match width {
        1..=63 => Ok((1_u64 << width) - 1),
        64 => Ok(u64::MAX),
        _ => Err(Diagnostic::error("X86_64 bit-field width must be 1..=64")),
    }
}

pub fn runtime_storage_bit_field_write_width(
    fragments: &[omega_target_operations::RuntimeBitFieldFragment],
) -> Result<usize, Diagnostic> {
    let mut width = 10;
    for fragment in fragments {
        let container_bytes = bit_fragment_container_bytes(fragment)?;
        width +=
            unsigned_load_width(container_bytes) + 10 + 3 + 10 + 3 + store_width(container_bytes);
    }
    Ok(width)
}

pub fn encode_runtime_storage_bit_field_write(
    base_byte_offset: usize,
    fragments: &[omega_target_operations::RuntimeBitFieldFragment],
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if fragments.is_empty() {
        return Err(Diagnostic::error(
            "X86_64 bit-field write requires at least one fragment",
        ));
    }
    let mut bytes = Vec::with_capacity(runtime_storage_bit_field_write_width(fragments)?);
    append_mov_r15_imm64(&mut bytes, 0);
    for fragment in fragments {
        let container_bytes = bit_fragment_container_bytes(fragment)?;
        let destination_mask = bit_width_mask(fragment.width)?
            .checked_shl(u32::from(fragment.destination_lsb))
            .ok_or_else(|| {
                Diagnostic::error("X86_64 bit-field destination mask overflows 64 bits")
            })?;
        let source_bits = (value as u64)
            .checked_shr(u32::from(fragment.source_lsb))
            .unwrap_or(0)
            & bit_width_mask(fragment.width)?;
        let inserted = source_bits
            .checked_shl(u32::from(fragment.destination_lsb))
            .ok_or_else(|| Diagnostic::error("X86_64 bit-field value overflows 64 bits"))?;
        let offset = base_byte_offset
            .checked_add(fragment.container_byte_offset)
            .ok_or_else(|| Diagnostic::error("X86_64 bit-field offset overflows"))?;
        append_load_unsigned_reg_from_r15(&mut bytes, Reg64::R11, offset, container_bytes)?;
        append_mov_rax_imm64(&mut bytes, !destination_mask);
        bytes.extend([0x49, 0x21, 0xc3]); // and r11, rax
        append_mov_rax_imm64(&mut bytes, inserted);
        bytes.extend([0x49, 0x09, 0xc3]); // or r11, rax
        append_store_r11_to_r15_width(&mut bytes, offset, container_bytes)?;
    }
    Ok(bytes)
}

/// Closed may-write ceiling of the immediate bit-field read/modify/write
/// encoder. r15 owns the relocated base, r11 stages each container, and rax
/// materializes the clear/insert masks.
pub fn runtime_storage_bit_field_write_register_write_ceiling() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86R11,
        MachineRegister::X86R15,
    ])
}

pub fn runtime_storage_bit_field_write_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub(super) fn runtime_bit_field_operand_width(
    fragments: &[omega_target_operations::RuntimeBitFieldFragment],
) -> Result<usize, Diagnostic> {
    let mut width = 13; // mov r15,imm64 + xor destination,destination
    for fragment in fragments {
        let container_bytes = bit_fragment_container_bytes(fragment)?;
        width += unsigned_load_width(container_bytes)
            + usize::from(fragment.destination_lsb != 0) * 4
            + 10
            + 3
            + usize::from(fragment.source_lsb != 0) * 4
            + 3;
    }
    Ok(width)
}

pub(super) fn append_runtime_bit_field_operand(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    base_byte_offset: usize,
    fragments: &[omega_target_operations::RuntimeBitFieldFragment],
) -> Result<(), Diagnostic> {
    if fragments.is_empty() {
        return Err(Diagnostic::error(
            "X86_64 bit-field operand requires at least one fragment",
        ));
    }
    append_mov_r15_imm64(bytes, 0);
    match destination {
        Reg64::R10 => bytes.extend([0x4d, 0x31, 0xd2]), // xor r10,r10
        Reg64::R11 => bytes.extend([0x4d, 0x31, 0xdb]), // xor r11,r11
    }
    for fragment in fragments {
        let container_bytes = bit_fragment_container_bytes(fragment)?;
        let offset = base_byte_offset
            .checked_add(fragment.container_byte_offset)
            .ok_or_else(|| Diagnostic::error("X86_64 bit-field offset overflows"))?;
        append_load_rax_from_r15_width(bytes, offset, container_bytes)?;
        if fragment.destination_lsb != 0 {
            bytes.extend([0x48, 0xc1, 0xe8, fragment.destination_lsb as u8]); // shr rax, imm8
        }
        append_mov_rcx_imm64(bytes, bit_width_mask(fragment.width)?);
        bytes.extend([0x48, 0x21, 0xc8]); // and rax, rcx
        if fragment.source_lsb != 0 {
            bytes.extend([0x48, 0xc1, 0xe0, fragment.source_lsb as u8]); // shl rax, imm8
        }
        match destination {
            Reg64::R10 => bytes.extend([0x49, 0x09, 0xc2]), // or r10,rax
            Reg64::R11 => bytes.extend([0x49, 0x09, 0xc3]), // or r11,rax
        }
    }
    Ok(())
}

pub fn runtime_machine_indexed_integer_write_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_byte_size: usize,
    _element_byte_size: usize,
    _byte_size: usize,
) -> usize {
    // mov r15,imm64 (10) [+ mov r10,imm64 (10) for RuntimeFrame index]
    // + mov rax,[base+index_off] (7) + imul rax,rax,imm32 (7)
    // + add r15,rax (3) + mov rax,imm64 (10) + store [r15+disp] (7).
    let index_load_width = unsigned_load_width(index_byte_size);
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => 47 + index_load_width,
        omega_target_operations::RuntimeStorageRegion::Machine => 37 + index_load_width,
    }
}

/// For x86_64 the runtime-frame index base is loaded by the second instruction
/// (`mov r10, imm64`), which begins 10 bytes into the sequence; the relocation
/// planner adds the +2 immediate offset itself.
pub fn runtime_machine_indexed_integer_runtime_frame_address_offset() -> usize {
    10
}

pub fn encode_runtime_machine_indexed_integer_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte machine indexed integers yet"
        )));
    }
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale machine index by element size `{element_byte_size}`"
        ))
    })?;
    let _ = element_scale;
    // Write rung 1c: DELEGATES to the place materializer -- a REGISTER
    // RENAME canonicalization (the retired layout staged the index through
    // RAX and a frame-resident index base through r10; the materializer
    // uses the r11 discipline). Same instruction WIDTHS at every position,
    // so the walker's +10 frame-base offset and the width fn hold as-is;
    // the differential legs oracle the byte change.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a machine-indexed place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_indexed_integer_write_width(
            index_region,
            index_byte_size,
            element_byte_size,
            byte_size,
        )
    );
    Ok(bytes)
}

/// Relocation imm offset (pre-`+2`) of the frame base loaded for the target slot
/// store in `encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame`.
pub const FRAME_BASE_INDEXED_COPY_TARGET_FRAME_IMM_OFFSET: usize = 41;

/// Start of the SECOND `mov r15,imm64` (the machine base) inside the
/// frame-source variant of the write half -- the machine relocation; the
/// relocation planner adds the +2 immediate offset itself.
pub fn runtime_storage_copy_to_runtime_machine_indexed_frame_source_machine_base_offset() -> usize {
    17
}

/// Start of the `mov r10,imm64` (the frame base for a FRAME-resident index)
/// inside the write half -- the frame relocation; sits after the source
/// load (+17) and after the frame-source machine re-load when present (+10).
pub fn runtime_storage_copy_to_runtime_machine_indexed_frame_index_base_offset(
    source_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    if source_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        27
    } else {
        17
    }
}

pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_width(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    // Read part: mov r15,imm64 (10) + mov eax,[idx base] (7) + imul rax,imm32 (7)
    // + add r15,rax (3) + load rax,[r15+disp] (7) = 34.
    // Write part: mov r15,imm64 (10) + mov r10d,[idx base] (7) + imul r10,imm32
    // (7) + add r15,r10 (3) + store [r15+disp] (7) = 34.
    // A FRAME-resident index on either side inserts its own frame-base
    // `mov r10,imm64` (+10) before that side's index load.
    runtime_storage_copy_machine_indexed_read_part_width(source_index_region)
        + if target_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            44
        } else {
            34
        }
}

/// Width of the READ half of the dual-indexed copy (also the start of the
/// WRITE part's machine-base `mov r15,imm64`).
pub fn runtime_storage_copy_machine_indexed_read_part_width(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    if source_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        44
    } else {
        34
    }
}

/// Width of [`encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage`].
/// MUST equal the emitter exactly. Any frame-resident index adds one r10
/// frame-base load (mov r10,imm64 at +10).
pub fn runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    // mov r15,imm64 (10) [+ mov r10,imm64 (10) if any frame index]
    // + mov eax,[..+outer] (7) + mov r11d,[..+inner] (7)
    // + imul rax,imm32 (7) + imul r11,imm32 (7) + add r15,rax (3)
    // + add r15,r11 (3) + load rax,[r15+disp] (7)
    // + mov r15,imm64 (10) + store [r15+target] (7)
    if double_indexed_any_frame(outer_index_region, inner_index_region) {
        78
    } else {
        68
    }
}

fn double_indexed_any_frame(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> bool {
    outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
}

/// Start of the `mov r10,imm64` frame-base load inside the double-indexed
/// read (pre-`+2`; present only when an index is frame-resident).
/// Write rung 1c (the canonicalized double-indexed WRITE): the OUTER
/// frame-resident index base (`mov r11,imm64`) begins right after the opening
/// machine mov.
pub fn runtime_machine_double_indexed_integer_write_outer_frame_offset() -> usize {
    10
}

/// The INNER frame-resident index base (`mov r10,imm64`): after the opening
/// mov + the outer index sequence (17 cross-region / 7 same-region) + its imul.
pub fn runtime_machine_double_indexed_integer_write_inner_frame_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    let outer = if outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        17
    } else {
        7
    };
    10 + outer + 7
}

pub fn runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset() -> usize {
    10
}

/// Start of the WRITE-half `mov r15,imm64` (the target-region relocation,
/// pre-`+2`) inside the double-indexed read.
pub fn runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
        outer_index_region,
        inner_index_region,
    ) - 17
}

/// Width of [`encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage`].
pub fn runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    // mov r15,imm64 (10) [+ mov r10,imm64 (10) if any frame place]
    // + mov r14,[..+src] (7) + mov eax,[..+outer] (7) + mov r11d,[..+inner] (7)
    // + imul rax,imm32 (7) + imul r11,imm32 (7) + add r15,rax (3)
    // + add r15,r11 (3) + store [r15+disp],r14 (7)
    if double_indexed_write_any_frame(source_region, outer_index_region, inner_index_region) {
        68
    } else {
        58
    }
}

fn double_indexed_write_any_frame(
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> bool {
    [source_region, outer_index_region, inner_index_region]
        .iter()
        .any(|region| *region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
}

/// Width of [`encode_runtime_machine_double_indexed_integer_write`].
pub fn runtime_machine_double_indexed_integer_write_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
) -> usize {
    // Canonicalized by the place materializer (Write rung 1c): mov r15,imm64
    // (10) + per-index [cross-region: mov reg,imm64 (10) + load (7) | same-
    // region: load (7)] + imul (7) each + add r15,r11 (3) + add r15,r10 (3)
    // + mov rax,imm64 (10) + store (7). Each FRAME index adds its OWN base
    // (r11 for the outer, r10 for the inner) -- no shared r10 anymore.
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    47 + unsigned_load_width(outer_index_byte_size)
        + unsigned_load_width(inner_index_byte_size)
        + if outer_index_region == frame { 10 } else { 0 }
        + if inner_index_region == frame { 10 } else { 0 }
}

/// Const-value write into a both-runtime nested element (`grid[i][j] = 70`):
/// the address computation of the double-indexed read, then `mov rax, imm64`
/// and a width-correct store (rax is free after the adds).
pub fn encode_runtime_machine_double_indexed_integer_write(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte double-indexed integers yet"
        )));
    }
    for region in [outer_index_region, inner_index_region] {
        if !matches!(
            region,
            omega_target_operations::RuntimeStorageRegion::Machine
                | omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        ) {
            return Err(Diagnostic::error(
                "X86_64 MVP encoder cannot write a double-indexed integer with this index region yet",
            ));
        }
    }
    // Write rung 1c: DELEGATES to the place materializer -- CANONICALIZED:
    // the retired layout materialized ONE shared r10 frame base for BOTH
    // frame-resident indices and staged the outer index in RAX; the
    // materializer materializes each cross-region index base separately
    // (r11 then r10). Widths and frame-base reloc positions move -- the
    // width fn and the walker's per-index arm move in lockstep.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: outer_index_region,
                    index_offset: outer_index_offset,
                    index_byte_size: outer_index_byte_size,
                    element_byte_size: outer_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: inner_index_region,
                    index_offset: inner_index_offset,
                    index_byte_size: inner_index_byte_size,
                    element_byte_size: inner_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a double-indexed place is five steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_double_indexed_integer_write_width(
            outer_index_region,
            outer_index_byte_size,
            inner_index_region,
            inner_index_byte_size,
        )
    );
    Ok(bytes)
}

pub fn runtime_pointee_integer_write_width(_field_byte_offset: usize, _byte_size: usize) -> usize {
    // mov r15,imm64 (10) + mov r15,[r15+ptr] (7) + mov rax,imm64 (10) + store [r15+field] (7)
    34
}

pub fn encode_runtime_pointee_integer_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte pointee integers yet"
        )));
    }
    // Write rung 1b: DELEGATES byte-for-byte to the place materializer
    // ([Const(ptr), Deref, Const(field)]; unit-pinned identity).
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                pointer_byte_offset,
            ))
            .and_then(|place| place.with_step(omega_target_operations::PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a pointee place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_integer_write_width(field_byte_offset, byte_size)
    );
    Ok(bytes)
}

pub fn runtime_frame_indexed_integer_write_width(
    index_byte_size: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
) -> usize {
    // mov r14,imm64 (10) + mov r15,[r14+desc] (7) + mov r11,[r14+idx] (7)
    // + imul r11,r11,elem (7) + add r15,r11 (3) + mov rax,imm64 (10) + store [r15+field] (7)
    44 + unsigned_load_width(index_byte_size)
}

pub fn encode_runtime_frame_indexed_integer_write(
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte frame indexed integers yet"
        )));
    }
    // Write rung 1c: DELEGATES to the place materializer -- the SAME
    // instruction multiset REORDERED (the index pre-loads into r11 while
    // r15 still equals the frame base, BEFORE the descriptor deref consumes
    // it; the retired layout loaded the descriptor first through a separate
    // r14 base). Same width, one start relocation -- the Copy rung-1c-i
    // reorder precedent.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                descriptor_offset,
            ))
            .and_then(|place| place.with_step(omega_target_operations::PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a frame-indexed place is five steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_indexed_integer_write_width(
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        )
    );
    Ok(bytes)
}

pub fn runtime_frame_base_indexed_integer_write_width(
    _base_byte_offset: usize,
    index_byte_size: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
) -> usize {
    // Canonicalized by the place materializer (Write rung 1c): mov r15,imm64
    // (10) + exact-width load-zx r11,[r15+idx] + imul r11,r11,elem (7)
    // + add r15,r11 (3) + mov rax,imm64 (10) + store [r15+base+field] (7).
    // The retired layout's redundant `mov r15,r14` is gone.
    37 + unsigned_load_width(index_byte_size)
}

pub fn encode_runtime_frame_base_indexed_integer_write(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte frame base-indexed integers yet"
        )));
    }
    // Write rung 1c: DELEGATES to the place materializer -- CANONICALIZED
    // 47 -> 44 bytes (the retired layout staged the base in r14 and copied
    // it to r15 with a redundant `mov r15,r14`; the materializer opens in
    // r15 directly). The one frame-base relocation stays at instruction
    // start; the width fn shrinks in lockstep.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a frame-base-indexed place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size
        )
    );
    Ok(bytes)
}

/// Bytes inserted between the left and right operand evaluations of a binary
/// write on x86_64: a single `push r10` that preserves the left result while the
/// right operand is evaluated (both accumulate in r10). Relocation planning adds
/// this to the right operand's start offset.
pub const BINARY_RIGHT_OPERAND_PUSH_WIDTH: usize = 2;
/// Relative address-materialization sites inside recursive value operands.
pub const MACHINE_INDEXED_OPERAND_FRAME_INDEX_BASE_OFFSET: usize = 13;
pub const FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET: usize = 17;
