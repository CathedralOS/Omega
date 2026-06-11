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
//!   x20 = written page, x26 = runtime scalar value.
//!
//! THE WIDTHS INVARIANT: every byte appended here must move
//! `append_wire_literal_byte_width` / `append_wire_scalar_varint_width` (and
//! the relocation offset functions next to them in `widths.rs`) in exact
//! lockstep, or relocations drift and the binary segfaults. Both encoders end
//! with a `debug_assert_eq!` against their width function.

use omega_core::diagnostics::Diagnostic;
use omega_target_operations::RuntimeStorageRegion;

use super::primitives::{
    append_add_x_constant, encode_add_page_offset_placeholder, encode_add_x_immediate,
    encode_add_x_register, encode_adrp_placeholder, encode_and_x_immediate_low_seven,
    encode_asr_x_immediate, encode_cbz_x, encode_eor_x_register, encode_lsr_x_immediate,
    encode_movz_w, encode_orr_x_immediate_bit_seven, encode_sign_extend_word_to_x,
    encode_store_byte_w_post_increment, encode_unconditional_branch,
};
use super::widths::{
    append_wire_literal_byte_width, append_wire_scalar_varint_width, wire_varint_emit_loop_width,
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
    super::runtime_storage::append_load_data_from_x_offset(
        bytes,
        17,
        20,
        written_offset,
        8,
        19,
    )?;
    bytes.extend(encode_add_x_register(16, 16, 17));
    Ok(())
}

/// Shared epilogue: store the advanced cursor back to the written slot.
fn append_wire_append_epilogue(
    bytes: &mut Vec<u8>,
    written_offset: usize,
) -> Result<(), Diagnostic> {
    super::runtime_storage::append_store_data_to_x_offset(
        bytes,
        17,
        20,
        written_offset,
        8,
        19,
    )
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
