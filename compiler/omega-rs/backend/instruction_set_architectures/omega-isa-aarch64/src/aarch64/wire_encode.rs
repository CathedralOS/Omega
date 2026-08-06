//! compact_binary v0 wire-encode sequences (chapter 20, decision 10).
//!
//! Both operations share one cursor convention: the caller's `written` slot
//! holds the running byte count, so every append loads it, stores through a
//! moving pointer (`out base + out offset + cursor`), and writes the advanced
//! cursor back. Chaining appends therefore needs no register state between
//! instructions, and the final cursor value IS the `written` out-parameter.
//!
//! Register use (the standard scratch family; x18 stays untouched):
//!   x16 = moving out pointer, x17 = cursor, x19 = byte/zigzag scratch,
//!   x20 = written page, x26 = runtime scalar value; the text append also
//!   uses x22 = remaining copy count, x24 = out capacity, x25 = source ptr.
//!
//! THE WIDTHS INVARIANT: every byte appended here must move
//! `append_wire_literal_byte_width` / `append_wire_scalar_varint_width` (and
//! the relocation offset functions next to them in `widths.rs`) in exact
//! lockstep, or relocations drift and the binary segfaults. Both encoders end
//! with a `debug_assert_eq!` against their width function.

use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use omega_target_operations::RuntimeStorageRegion;
use psi_diagnostics::Diagnostic;

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate, encode_add_page_offset_placeholder,
    encode_add_x_immediate, encode_add_x_register, encode_adrp_placeholder,
    encode_and_x_immediate_low_seven, encode_asr_x_immediate, encode_cbz_x,
    encode_compare_x_register, encode_conditional_branch_higher,
    encode_conditional_branch_higher_or_same, encode_eor_x_register,
    encode_load_byte_w_post_increment, encode_lsr_x_immediate, encode_move_x_register, encode_movz,
    encode_movz_w, encode_orr_x_immediate_bit_seven, encode_sign_extend_word_to_x,
    encode_store_byte_w_post_increment, encode_subs_x_immediate, encode_unconditional_branch,
};
use super::widths::{
    append_wire_literal_byte_width, append_wire_repeated_scalar_varint_width,
    append_wire_scalar_slice_width, append_wire_scalar_varint_width, append_wire_text_bytes_width,
    wire_text_copy_loop_width, wire_varint_emit_loop_width,
};

/// Shared prologue: x16 = out base + out offset + cursor, x17 = cursor,
/// x20 = the written slot's page (kept live for the cursor write-back).
/// Relocations: out page at the instruction start, written page at
/// `wire_written_page_offset`.
fn append_wire_append_prologue(
    bytes: &mut Vec<u8>,
    out_offset: usize,
    written_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_x_constant(bytes, 16, 16, out_offset, 19)?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    super::runtime_storage::append_load_data_from_x_offset(bytes, 17, 20, written_offset, 8, 19)?;
    bytes.extend(encode_add_x_register(16, 16, 17));
    Ok(())
}

/// Shared epilogue: store the advanced cursor back to the written slot.
fn append_wire_append_epilogue(
    bytes: &mut Vec<u8>,
    written_offset: usize,
) -> Result<(), Diagnostic> {
    super::runtime_storage::append_store_data_to_x_offset(bytes, 17, 20, written_offset, 8, 19)
}

pub fn append_wire_literal_byte_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
    ])
}

/// One compile-time framing byte (era/tag varint bytes): store it at the
/// cursor and advance by one.
pub fn encode_append_wire_literal_byte(
    out_offset: usize,
    written_offset: usize,
    value: u8,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(append_wire_literal_byte_width(out_offset, written_offset));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;
    bytes.extend(encode_movz_w(19, u16::from(value)));
    bytes.extend(encode_store_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    append_wire_append_epilogue(&mut bytes, written_offset)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_literal_byte_width(out_offset, written_offset)
    );
    Ok(bytes)
}

/// LEB128-encode a runtime scalar at the cursor. The value loads zero-extended
/// at its source width; signed sources (`zigzag`) sign-extend to 64 bits and
/// zigzag (`(n << 1) ^ (n >> 63)`) before the emit loop.
pub fn encode_append_wire_scalar_varint(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 wire encoder cannot varint-encode {byte_size}-byte scalars yet"
        )));
    }
    // The region only picks the relocation symbol; the encoded shape is
    // identical for machine and frame sources.
    let _ = source_region;

    let mut bytes = Vec::with_capacity(append_wire_scalar_varint_width(
        source_offset,
        byte_size,
        zigzag,
        out_offset,
        written_offset,
    ));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;

    // x26 = the source page, then the scalar itself (zero-extended load).
    bytes.extend(encode_adrp_placeholder(26));
    bytes.extend(encode_add_page_offset_placeholder(26));
    super::runtime_storage::append_load_data_from_x_offset(
        &mut bytes,
        26,
        26,
        source_offset,
        byte_size,
        19,
    )?;

    if zigzag {
        if byte_size == 4 {
            bytes.extend(encode_sign_extend_word_to_x(26, 26));
        }
        // zigzag(n) = (n << 1) ^ (n >> 63); x19 holds the sign mask.
        bytes.extend(encode_asr_x_immediate(19, 26, 63));
        bytes.extend(encode_add_x_register(26, 26, 26));
        bytes.extend(encode_eor_x_register(26, 26, 19));
    }

    // LEB128 emit loop (fixed 36 bytes, see `wire_varint_emit_loop_width`):
    //   loop: and  x19, x26, #0x7f
    //         lsr  x26, x26, #7
    //         cbz  x26, last          (+20: skip orr/strb/add/b)
    //         orr  x19, x19, #0x80
    //         strb w19, [x16], #1
    //         add  x17, x17, #1
    //         b    loop               (-24)
    //   last: strb w19, [x16], #1
    //         add  x17, x17, #1
    bytes.extend(encode_and_x_immediate_low_seven(19, 26));
    bytes.extend(encode_lsr_x_immediate(26, 26, 7));
    bytes.extend(encode_cbz_x(26, 20)?);
    bytes.extend(encode_orr_x_immediate_bit_seven(19, 19));
    bytes.extend(encode_store_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    bytes.extend(encode_unconditional_branch(-24)?);
    bytes.extend(encode_store_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    debug_assert_eq!(
        wire_varint_emit_loop_width(),
        36,
        "the emit loop above is nine fixed instructions"
    );

    append_wire_append_epilogue(&mut bytes, written_offset)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset
        )
    );
    Ok(bytes)
}

pub fn append_wire_scalar_varint_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

pub fn append_wire_repeated_scalar_varint_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

pub fn append_wire_repeated_scalar_varint_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// LEB128-encode element `index` of a packed repeated field at the cursor,
/// ONLY IF `index < count` (the FixedVec `length` slot, read as unsigned
/// 64-bit). A skipped element leaves the cursor untouched, so the staged
/// payload holds exactly the live elements. Counts past the declared maximum
/// clamp for free: selection unrolls only `max` of these.
#[allow(clippy::too_many_arguments)]
pub fn encode_append_wire_repeated_scalar_varint(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    index: u64,
    count_region: RuntimeStorageRegion,
    count_offset: usize,
    out_offset: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 wire encoder cannot varint-encode {byte_size}-byte scalars yet"
        )));
    }
    // The regions only pick the relocation symbols; the encoded shape is
    // identical for machine and frame places.
    let _ = (source_region, count_region);

    let mut bytes = Vec::with_capacity(append_wire_repeated_scalar_varint_width(
        source_offset,
        byte_size,
        zigzag,
        index,
        count_offset,
        out_offset,
        written_offset,
    ));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;

    // Guard: x26 = count (from the relocated count page), x19 = the
    // compile-time element index; skip the whole append (including the
    // cursor write-back) when index >= count.
    bytes.extend(encode_adrp_placeholder(26));
    bytes.extend(encode_add_page_offset_placeholder(26));
    super::runtime_storage::append_load_data_from_x_offset(
        &mut bytes,
        26,
        26,
        count_offset,
        8,
        19,
    )?;
    append_unsigned_immediate(&mut bytes, 19, index);
    let zigzag_width = if zigzag {
        super::widths::wire_zigzag_width(byte_size)
    } else {
        0
    };
    let remaining_after_branch = 8
        + super::widths::load_data_offset_width(source_offset, byte_size)
        + zigzag_width
        + wire_varint_emit_loop_width()
        + super::widths::store_data_offset_width(written_offset, 8);
    bytes.extend(encode_compare_x_register(19, 26));
    bytes.extend(encode_conditional_branch_higher_or_same(
        (remaining_after_branch + 4) as isize,
    )?);

    // The unguarded scalar-varint body (see `encode_append_wire_scalar_varint`):
    // x26 = the source page, then the scalar itself (zero-extended load).
    bytes.extend(encode_adrp_placeholder(26));
    bytes.extend(encode_add_page_offset_placeholder(26));
    super::runtime_storage::append_load_data_from_x_offset(
        &mut bytes,
        26,
        26,
        source_offset,
        byte_size,
        19,
    )?;

    if zigzag {
        if byte_size == 4 {
            bytes.extend(encode_sign_extend_word_to_x(26, 26));
        }
        bytes.extend(encode_asr_x_immediate(19, 26, 63));
        bytes.extend(encode_add_x_register(26, 26, 26));
        bytes.extend(encode_eor_x_register(26, 26, 19));
    }

    // The same fixed nine-instruction LEB128 emit loop as the scalar varint.
    bytes.extend(encode_and_x_immediate_low_seven(19, 26));
    bytes.extend(encode_lsr_x_immediate(26, 26, 7));
    bytes.extend(encode_cbz_x(26, 20)?);
    bytes.extend(encode_orr_x_immediate_bit_seven(19, 19));
    bytes.extend(encode_store_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    bytes.extend(encode_unconditional_branch(-24)?);
    bytes.extend(encode_store_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    debug_assert_eq!(wire_varint_emit_loop_width(), 36);

    append_wire_append_epilogue(&mut bytes, written_offset)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_repeated_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            index,
            count_offset,
            out_offset,
            written_offset
        )
    );
    Ok(bytes)
}

/// Append a runtime `String` field: the source place holds a `{ptr @ +0,
/// len @ +8}` text descriptor; emit len as an unsigned LEB128 varint, then
/// copy len raw bytes from ptr. The length varint reuses the scalar emit loop
/// (validation's worst-case budget covers its ten bytes -- String fields
/// encode LAST). The byte-copy is the one append whose size is
/// runtime-unbounded, so every copy store is bounds-checked against
/// `out_length` and content past capacity is DROPPED: the cursor stops at
/// `out_length`, never past it.
pub fn encode_append_wire_text_bytes(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    // The region only picks the relocation symbol; the encoded shape is
    // identical for machine and frame sources.
    let _ = source_region;

    let mut bytes = Vec::with_capacity(append_wire_text_bytes_width(
        source_offset,
        out_offset,
        out_length,
        written_offset,
    ));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;

    // x26 = the descriptor page, then x25 = ptr (+0) and x26 = len (+8); the
    // page register is consumed by the len load LAST.
    bytes.extend(encode_adrp_placeholder(26));
    bytes.extend(encode_add_page_offset_placeholder(26));
    super::runtime_storage::append_load_data_from_x_offset(
        &mut bytes,
        25,
        26,
        source_offset,
        8,
        19,
    )?;
    super::runtime_storage::append_load_data_from_x_offset(
        &mut bytes,
        26,
        26,
        source_offset + 8,
        8,
        19,
    )?;
    // x22 keeps the byte count for the copy loop; the emit loop consumes x26.
    bytes.extend(encode_move_x_register(22, 26));

    // The same fixed nine-instruction LEB128 emit loop as the scalar varint
    // (see `encode_append_wire_scalar_varint`), here emitting the LENGTH.
    bytes.extend(encode_and_x_immediate_low_seven(19, 26));
    bytes.extend(encode_lsr_x_immediate(26, 26, 7));
    bytes.extend(encode_cbz_x(26, 20)?);
    bytes.extend(encode_orr_x_immediate_bit_seven(19, 19));
    bytes.extend(encode_store_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    bytes.extend(encode_unconditional_branch(-24)?);
    bytes.extend(encode_store_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    debug_assert_eq!(wire_varint_emit_loop_width(), 36);

    // x24 = the out buffer's compile-time capacity, bounding every copy store.
    append_unsigned_immediate(&mut bytes, 24, out_length as u64);

    // Bounded byte-copy loop (fixed 32 bytes, `wire_text_copy_loop_width`):
    //   copy: cbz  x22, done       (+32: all bytes copied)
    //         cmp  x17, x24
    //         b.hs done            (+24: capacity full -- drop the rest)
    //         ldrb w19, [x25], #1
    //         strb w19, [x16], #1
    //         add  x17, x17, #1
    //         subs x22, x22, #1
    //         b    copy            (-28)
    //   done:
    bytes.extend(encode_cbz_x(22, 32)?);
    bytes.extend(encode_compare_x_register(17, 24));
    bytes.extend(encode_conditional_branch_higher_or_same(24)?);
    bytes.extend(encode_load_byte_w_post_increment(19, 25, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    bytes.extend(encode_subs_x_immediate(22, 22, 1)?);
    bytes.extend(encode_unconditional_branch(-28)?);
    debug_assert_eq!(
        wire_text_copy_loop_width(),
        32,
        "the copy loop above is eight fixed instructions"
    );

    append_wire_append_epilogue(&mut bytes, written_offset)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_text_bytes_width(source_offset, out_offset, out_length, written_offset)
    );
    Ok(bytes)
}

pub fn append_wire_text_bytes_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(22),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(25),
        MachineRegister::Aarch64X(26),
    ])
}

pub fn append_wire_text_bytes_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WireSliceLabel {
    MeasureOuter,
    MeasureVarint,
    MeasureScalarDone,
    MeasureDone,
    PrefixCount,
    PrefixDone,
    EmitOuter,
    Done,
}

enum WireSliceInstruction {
    Fixed(Vec<u8>),
    Cbz(u8, WireSliceLabel),
    BHi(WireSliceLabel),
    B(WireSliceLabel),
}

fn wire_slice_fixed(word: [u8; 4]) -> WireSliceInstruction {
    WireSliceInstruction::Fixed(word.to_vec())
}

fn append_wire_slice_scalar_load(
    program: &mut Vec<(Option<WireSliceLabel>, WireSliceInstruction)>,
    element_byte_size: usize,
    zigzag: bool,
) -> Result<(), Diagnostic> {
    let mut load = Vec::new();
    super::runtime_storage::append_load_data_from_x_offset(
        &mut load,
        26,
        21,
        0,
        element_byte_size,
        19,
    )?;
    program.push((None, WireSliceInstruction::Fixed(load)));
    program.push((
        None,
        wire_slice_fixed(encode_add_x_immediate(21, 21, element_byte_size)?),
    ));
    if zigzag {
        if element_byte_size == 4 {
            program.push((None, wire_slice_fixed(encode_sign_extend_word_to_x(26, 26))));
        }
        program.push((None, wire_slice_fixed(encode_asr_x_immediate(19, 26, 63))));
        program.push((None, wire_slice_fixed(encode_add_x_register(26, 26, 26))));
        program.push((None, wire_slice_fixed(encode_eor_x_register(26, 26, 19))));
    }
    Ok(())
}

fn append_wire_slice_varint_emit(
    program: &mut Vec<(Option<WireSliceLabel>, WireSliceInstruction)>,
) -> Result<(), Diagnostic> {
    for word in [
        encode_and_x_immediate_low_seven(19, 26),
        encode_lsr_x_immediate(26, 26, 7),
        encode_cbz_x(26, 20)?,
        encode_orr_x_immediate_bit_seven(19, 19),
        encode_store_byte_w_post_increment(19, 16, 1)?,
        encode_add_x_immediate(17, 17, 1)?,
        encode_unconditional_branch(-24)?,
        encode_store_byte_w_post_increment(19, 16, 1)?,
        encode_add_x_immediate(17, 17, 1)?,
    ] {
        program.push((None, wire_slice_fixed(word)));
    }
    Ok(())
}

fn emit_wire_slice_program(
    bytes: &mut Vec<u8>,
    program: &[(Option<WireSliceLabel>, WireSliceInstruction)],
) -> Result<(), Diagnostic> {
    let mut positions = std::collections::HashMap::new();
    let mut cursor = 0usize;
    for (label, instruction) in program {
        if let Some(label) = label {
            positions.insert(*label, cursor);
        }
        cursor += match instruction {
            WireSliceInstruction::Fixed(bytes) => bytes.len(),
            _ => 4,
        };
    }
    positions.insert(WireSliceLabel::Done, cursor);

    cursor = 0;
    for (_, instruction) in program {
        let offset =
            |target: &WireSliceLabel| -> isize { positions[target] as isize - cursor as isize };
        match instruction {
            WireSliceInstruction::Fixed(fixed) => bytes.extend(fixed),
            WireSliceInstruction::Cbz(register, target) => {
                bytes.extend(encode_cbz_x(*register, offset(target))?)
            }
            WireSliceInstruction::BHi(target) => {
                bytes.extend(encode_conditional_branch_higher(offset(target))?)
            }
            WireSliceInstruction::B(target) => {
                bytes.extend(encode_unconditional_branch(offset(target))?)
            }
        }
        cursor += match instruction {
            WireSliceInstruction::Fixed(fixed) => fixed.len(),
            _ => 4,
        };
    }
    Ok(())
}

/// Encode a borrowed scalar slice without staging allocation. The first pass
/// measures the exact packed-varint body; after the exact remaining-capacity
/// check, the second pass emits the length prefix and body.
pub fn encode_append_wire_scalar_slice(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    element_byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(element_byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 wire encoder cannot varint-encode {element_byte_size}-byte slice elements yet"
        )));
    }
    let _ = source_region;
    let mut bytes = Vec::with_capacity(append_wire_scalar_slice_width(
        source_offset,
        element_byte_size,
        zigzag,
        out_offset,
        out_length,
        written_offset,
    ));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;

    // x25 = original ptr, x21 = walking ptr; x23 = original count, x22 =
    // remaining count; x24 = exact body bytes; x28 = output capacity.
    bytes.extend(encode_adrp_placeholder(26));
    bytes.extend(encode_add_page_offset_placeholder(26));
    super::runtime_storage::append_load_data_from_x_offset(
        &mut bytes,
        25,
        26,
        source_offset,
        8,
        19,
    )?;
    super::runtime_storage::append_load_data_from_x_offset(
        &mut bytes,
        22,
        26,
        source_offset + 8,
        8,
        19,
    )?;
    bytes.extend(encode_move_x_register(21, 25));
    bytes.extend(encode_move_x_register(23, 22));
    bytes.extend(encode_movz(24, 0));
    append_unsigned_immediate(&mut bytes, 28, out_length as u64);

    use WireSliceInstruction::{B, BHi, Cbz};
    use WireSliceLabel::{
        Done, EmitOuter, MeasureDone, MeasureOuter, MeasureScalarDone, MeasureVarint, PrefixCount,
        PrefixDone,
    };
    let mut program = Vec::new();
    program.push((None, wire_slice_fixed(encode_compare_x_register(22, 28))));
    program.push((None, BHi(Done)));
    program.push((Some(MeasureOuter), Cbz(22, MeasureDone)));
    append_wire_slice_scalar_load(&mut program, element_byte_size, zigzag)?;
    program.push((
        Some(MeasureVarint),
        wire_slice_fixed(encode_add_x_immediate(24, 24, 1)?),
    ));
    program.push((None, wire_slice_fixed(encode_lsr_x_immediate(26, 26, 7))));
    program.push((None, Cbz(26, MeasureScalarDone)));
    program.push((None, B(MeasureVarint)));
    program.push((
        Some(MeasureScalarDone),
        wire_slice_fixed(encode_subs_x_immediate(22, 22, 1)?),
    ));
    program.push((None, B(MeasureOuter)));

    program.push((
        Some(MeasureDone),
        wire_slice_fixed(encode_move_x_register(26, 24)),
    ));
    program.push((None, wire_slice_fixed(encode_movz(27, 0))));
    program.push((
        Some(PrefixCount),
        wire_slice_fixed(encode_add_x_immediate(27, 27, 1)?),
    ));
    program.push((None, wire_slice_fixed(encode_lsr_x_immediate(26, 26, 7))));
    program.push((None, Cbz(26, PrefixDone)));
    program.push((None, B(PrefixCount)));
    program.push((
        Some(PrefixDone),
        wire_slice_fixed(encode_add_x_register(19, 17, 24)),
    ));
    program.push((None, wire_slice_fixed(encode_add_x_register(19, 19, 27))));
    program.push((None, wire_slice_fixed(encode_compare_x_register(19, 28))));
    program.push((None, BHi(Done)));
    program.push((None, wire_slice_fixed(encode_move_x_register(26, 24))));
    append_wire_slice_varint_emit(&mut program)?;
    program.push((None, wire_slice_fixed(encode_move_x_register(21, 25))));
    program.push((None, wire_slice_fixed(encode_move_x_register(22, 23))));
    program.push((Some(EmitOuter), Cbz(22, Done)));
    append_wire_slice_scalar_load(&mut program, element_byte_size, zigzag)?;
    append_wire_slice_varint_emit(&mut program)?;
    program.push((None, wire_slice_fixed(encode_subs_x_immediate(22, 22, 1)?)));
    program.push((None, B(EmitOuter)));
    emit_wire_slice_program(&mut bytes, &program)?;

    append_wire_append_epilogue(&mut bytes, written_offset)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_scalar_slice_width(
            source_offset,
            element_byte_size,
            zigzag,
            out_offset,
            out_length,
            written_offset
        )
    );
    Ok(bytes)
}

pub fn append_wire_scalar_slice_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(21),
        MachineRegister::Aarch64X(22),
        MachineRegister::Aarch64X(23),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(25),
        MachineRegister::Aarch64X(26),
        MachineRegister::Aarch64X(27),
        MachineRegister::Aarch64X(28),
    ])
}

pub fn append_wire_scalar_slice_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}
