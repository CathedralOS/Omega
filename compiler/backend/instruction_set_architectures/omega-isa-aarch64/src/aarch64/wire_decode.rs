//! compact_binary v0 wire-decode sequences (chapter 20, wire stage 2b).
//!
//! Both operations share one cursor convention with the encoder: the caller's
//! `read` slot holds the running byte count, so every read loads it, reads
//! through a moving pointer (`buffer base + buffer offset + cursor`), and
//! writes the advanced cursor back. The success flag lives in the caller's
//! `ok` slot and is STICKY: each operation computes its own success bit and
//! ANDs it into the slot, never setting it, so the first failure (wrong era,
//! unexpected tag, truncated input, overlong varint) makes the whole decode
//! report failure no matter what later operations read. After a failure the
//! remaining operations still execute -- the contract is the flag, not the
//! partial payload -- but every byte read stays bounds-checked against the
//! buffer's compile-time length, so a failed decode never reads out of
//! bounds.
//!
//! Register use (the standard scratch family; x18 stays untouched):
//!   x16 = moving buffer pointer, x17 = cursor, x19 = byte scratch,
//!   x20 = read page, x21 = ok page, x22 = shift, x23 = this-op success,
//!   x24 = buffer length, x25 = 7-bit chunk / target page, x26 = value.
//!
//! THE WIDTHS INVARIANT: every byte appended here must move
//! `read_wire_expected_byte_width` / `read_wire_scalar_varint_width` (and the
//! `wire_decode_*_offset` relocation functions next to them in `widths.rs`)
//! in exact lockstep, or relocations drift and the binary segfaults. Both
//! encoders end with a `debug_assert_eq!` against their width function.

use omega_core::diagnostics::Diagnostic;
use omega_target_operations::RuntimeStorageRegion;

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate, encode_add_page_offset_placeholder,
    encode_add_x_immediate, encode_add_x_register, encode_adrp_placeholder,
    encode_and_x_immediate_low_seven, encode_and_x_register, encode_asr_x_immediate,
    encode_cbnz_x, encode_compare_w_immediate, encode_compare_x_register,
    encode_conditional_branch_equal, encode_conditional_branch_higher,
    encode_conditional_branch_higher_or_same, encode_eor_x_register,
    encode_load_byte_w_post_increment, encode_lslv_x_register, encode_lsr_x_immediate,
    encode_movz, encode_orr_x_register, encode_unconditional_branch,
};
use super::widths::{
    read_wire_expected_byte_width, read_wire_nested_close_width, read_wire_nested_open_width,
    read_wire_repeated_scalar_varint_width, read_wire_scalar_varint_width, wire_unzigzag_width,
    wire_varint_read_loop_width,
};

/// Shared prologue: x16 = buffer base + buffer offset + cursor, x17 = cursor,
/// x20 = the read slot's page (kept live for the cursor write-back), x21 =
/// the ok slot's page (kept live for the sticky-flag merge). Relocations:
/// buffer page at the instruction start, read page at
/// `wire_decode_read_page_offset`, ok page at `wire_decode_ok_page_offset`.
fn append_wire_decode_prologue(
    bytes: &mut Vec<u8>,
    buffer_offset: usize,
    read_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_x_constant(bytes, 16, 16, buffer_offset, 19)?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    super::runtime_storage::append_load_data_from_x_offset(bytes, 17, 20, read_offset, 8, 19)?;
    bytes.extend(encode_add_x_register(16, 16, 17));
    bytes.extend(encode_adrp_placeholder(21));
    bytes.extend(encode_add_page_offset_placeholder(21));
    Ok(())
}

/// Shared epilogue: AND this operation's success bit (x23) into the sticky ok
/// slot, then store the advanced cursor back to the read slot.
fn append_wire_decode_epilogue(
    bytes: &mut Vec<u8>,
    read_offset: usize,
    ok_offset: usize,
) -> Result<(), Diagnostic> {
    super::runtime_storage::append_load_data_from_x_offset(bytes, 19, 21, ok_offset, 1, 25)?;
    bytes.extend(encode_and_x_register(19, 19, 23));
    super::runtime_storage::append_store_data_to_x_offset(bytes, 19, 21, ok_offset, 1, 25)?;
    super::runtime_storage::append_store_data_to_x_offset(bytes, 17, 20, read_offset, 8, 25)
}

/// Expect one compile-time framing byte (era/tag varint bytes) at the cursor:
/// out of bounds clears ok without consuming; a mismatch consumes the byte
/// and clears ok; a match consumes the byte.
pub fn encode_read_wire_expected_byte(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    expected: u8,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(read_wire_expected_byte_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;
    append_unsigned_immediate(&mut bytes, 24, buffer_length as u64);
    bytes.extend(encode_movz(23, 1));

    // Fixed seven-instruction check block (28 bytes):
    //         cmp  x17, x24
    //         b.hs fail            (+20: skip ldrb/add/cmp/b.eq)
    //         ldrb w19, [x16], #1
    //         add  x17, x17, #1
    //         cmp  w19, #expected
    //         b.eq done            (+8: skip the fail movz)
    //   fail: movz x23, #0
    //   done:
    bytes.extend(encode_compare_x_register(17, 24));
    bytes.extend(encode_conditional_branch_higher_or_same(20)?);
    bytes.extend(encode_load_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    bytes.extend(encode_compare_w_immediate(19, u32::from(expected))?);
    bytes.extend(encode_conditional_branch_equal(8)?);
    bytes.extend(encode_movz(23, 0));

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_expected_byte_width(buffer_offset, buffer_length, read_offset, ok_offset)
    );
    Ok(bytes)
}

/// LEB128-read a runtime scalar at the cursor into the target place. The
/// loop is a fixed fourteen-instruction body whose iteration count is data
/// dependent but whose EMITTED width is constant (the widths invariant):
/// truncation (cursor past the buffer length) and overlong varints (a
/// continuation past shift 63, i.e. more than ten groups) branch to the fail
/// arm. Signed targets un-zigzag (`(n >> 1) ^ -(n & 1)`) before the store;
/// the store truncates to the field width.
#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_scalar_varint(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 wire decoder cannot varint-decode {byte_size}-byte scalars yet"
        )));
    }
    // The region only picks the relocation symbol; the encoded shape is
    // identical for machine and frame targets.
    let _ = target_region;

    let mut bytes = Vec::with_capacity(read_wire_scalar_varint_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        target_offset,
        byte_size,
        zigzag,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;
    append_unsigned_immediate(&mut bytes, 24, buffer_length as u64);
    bytes.extend(encode_movz(23, 1));
    bytes.extend(encode_movz(26, 0));
    bytes.extend(encode_movz(22, 0));

    // LEB128 read loop (fixed 56 bytes, `wire_varint_read_loop_width`):
    //   loop: cmp  x17, x24
    //         b.hs fail            (+48: truncated input)
    //         cmp  w22, #63
    //         b.hi fail            (+40: overlong varint, >10 groups)
    //         ldrb w19, [x16], #1
    //         add  x17, x17, #1
    //         and  x25, x19, #0x7f
    //         lslv x25, x25, x22
    //         orr  x26, x26, x25
    //         add  x22, x22, #7
    //         lsr  x19, x19, #7    (continuation bit)
    //         cbnz x19, loop       (-44)
    //         b    done            (+8: skip the fail movz)
    //   fail: movz x23, #0
    //   done:
    bytes.extend(encode_compare_x_register(17, 24));
    bytes.extend(encode_conditional_branch_higher_or_same(48)?);
    bytes.extend(encode_compare_w_immediate(22, 63)?);
    bytes.extend(encode_conditional_branch_higher(40)?);
    bytes.extend(encode_load_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    bytes.extend(encode_and_x_immediate_low_seven(25, 19));
    bytes.extend(encode_lslv_x_register(25, 25, 22));
    bytes.extend(encode_orr_x_register(26, 26, 25));
    bytes.extend(encode_add_x_immediate(22, 22, 7)?);
    bytes.extend(encode_lsr_x_immediate(19, 19, 7));
    bytes.extend(encode_cbnz_x(19, -44)?);
    bytes.extend(encode_unconditional_branch(8)?);
    bytes.extend(encode_movz(23, 0));
    debug_assert_eq!(
        wire_varint_read_loop_width(),
        56,
        "the read loop above is fourteen fixed instructions"
    );

    if zigzag {
        // unzigzag(n) = (n >> 1) ^ -(n & 1); the mask comes from sign-
        // extending bit 0 (lsl #63 then asr #63).
        bytes.extend(encode_movz(22, 63));
        bytes.extend(encode_lslv_x_register(19, 26, 22));
        bytes.extend(encode_asr_x_immediate(19, 19, 63));
        bytes.extend(encode_lsr_x_immediate(26, 26, 1));
        bytes.extend(encode_eor_x_register(26, 26, 19));
        debug_assert_eq!(wire_unzigzag_width(), 20);
    }

    // x25 = the target page, then the truncating store at the field width.
    bytes.extend(encode_adrp_placeholder(25));
    bytes.extend(encode_add_page_offset_placeholder(25));
    super::runtime_storage::append_store_data_to_x_offset(
        &mut bytes,
        26,
        25,
        target_offset,
        byte_size,
        19,
    )?;

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            byte_size,
            zigzag
        )
    );
    Ok(bytes)
}

/// LEB128-read one packed repeated element at the cursor into the target
/// slot, ONLY IF the cursor sits strictly below the end bound the
/// surrounding nested OPEN stored; the taken path also increments the
/// count-companion slot. A skipped read changes nothing -- the branch lands
/// past the epilogue, so cursor, ok, target, and count all stay put.
/// Selection unrolls the declared maximum of these, so a payload packing
/// more elements leaves the cursor short of the bound and the closing
/// nested CLOSE clears ok (the hostile-count cap); every taken read stays
/// bounds-checked against the buffer like any other wire read.
#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_repeated_scalar_varint(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
    count_region: RuntimeStorageRegion,
    count_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 wire decoder cannot varint-decode {byte_size}-byte scalars yet"
        )));
    }
    // The regions only pick the relocation symbols; the encoded shape is
    // identical for machine and frame places.
    let _ = (count_region, target_region);

    let mut bytes = Vec::with_capacity(read_wire_repeated_scalar_varint_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        end_offset,
        count_offset,
        target_offset,
        byte_size,
        zigzag,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    // Guard: x25 = the end-slot page (relocated at the nested end page
    // offset), x26 = the absolute end bound stored there; skip everything
    // (including the epilogue) when cursor >= end.
    bytes.extend(encode_adrp_placeholder(25));
    bytes.extend(encode_add_page_offset_placeholder(25));
    super::runtime_storage::append_load_data_from_x_offset(&mut bytes, 26, 25, end_offset, 8, 19)?;
    let zigzag_width = if zigzag { wire_unzigzag_width() } else { 0 };
    let remaining_after_branch = super::widths::unsigned_immediate_width(buffer_length as u64)
        + 12
        + wire_varint_read_loop_width()
        + zigzag_width
        + 8
        + super::widths::store_data_offset_width(target_offset, byte_size)
        + 8
        + super::widths::load_data_offset_width(count_offset, 8)
        + 4
        + super::widths::store_data_offset_width(count_offset, 8)
        + super::widths::load_data_offset_width(ok_offset, 1)
        + 4
        + super::widths::store_data_offset_width(ok_offset, 1)
        + super::widths::store_data_offset_width(read_offset, 8);
    bytes.extend(encode_compare_x_register(17, 26));
    bytes.extend(encode_conditional_branch_higher_or_same(
        (remaining_after_branch + 4) as isize,
    )?);

    // The unguarded scalar-varint body (see `encode_read_wire_scalar_varint`).
    append_unsigned_immediate(&mut bytes, 24, buffer_length as u64);
    bytes.extend(encode_movz(23, 1));
    bytes.extend(encode_movz(26, 0));
    bytes.extend(encode_movz(22, 0));

    bytes.extend(encode_compare_x_register(17, 24));
    bytes.extend(encode_conditional_branch_higher_or_same(48)?);
    bytes.extend(encode_compare_w_immediate(22, 63)?);
    bytes.extend(encode_conditional_branch_higher(40)?);
    bytes.extend(encode_load_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    bytes.extend(encode_and_x_immediate_low_seven(25, 19));
    bytes.extend(encode_lslv_x_register(25, 25, 22));
    bytes.extend(encode_orr_x_register(26, 26, 25));
    bytes.extend(encode_add_x_immediate(22, 22, 7)?);
    bytes.extend(encode_lsr_x_immediate(19, 19, 7));
    bytes.extend(encode_cbnz_x(19, -44)?);
    bytes.extend(encode_unconditional_branch(8)?);
    bytes.extend(encode_movz(23, 0));
    debug_assert_eq!(wire_varint_read_loop_width(), 56);

    if zigzag {
        bytes.extend(encode_movz(22, 63));
        bytes.extend(encode_lslv_x_register(19, 26, 22));
        bytes.extend(encode_asr_x_immediate(19, 19, 63));
        bytes.extend(encode_lsr_x_immediate(26, 26, 1));
        bytes.extend(encode_eor_x_register(26, 26, 19));
        debug_assert_eq!(wire_unzigzag_width(), 20);
    }

    // x25 = the target page, then the truncating store at the field width.
    bytes.extend(encode_adrp_placeholder(25));
    bytes.extend(encode_add_page_offset_placeholder(25));
    super::runtime_storage::append_store_data_to_x_offset(
        &mut bytes,
        26,
        25,
        target_offset,
        byte_size,
        19,
    )?;

    // Count bump: x25 = the count page, x19 = count + 1 (x22 is free -- the
    // read loop's shift use ended above).
    bytes.extend(encode_adrp_placeholder(25));
    bytes.extend(encode_add_page_offset_placeholder(25));
    super::runtime_storage::append_load_data_from_x_offset(
        &mut bytes,
        19,
        25,
        count_offset,
        8,
        22,
    )?;
    bytes.extend(encode_add_x_immediate(19, 19, 1)?);
    super::runtime_storage::append_store_data_to_x_offset(
        &mut bytes,
        19,
        25,
        count_offset,
        8,
        22,
    )?;

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_repeated_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_offset,
            target_offset,
            byte_size,
            zigzag
        )
    );
    Ok(bytes)
}

/// Open a nested sub-message region (chapter 20, nested message fields): the
/// end slot holds the sub-message LENGTH the caller just varint-read into it;
/// replace it with the ABSOLUTE end bound (`cursor + length`) and clear ok
/// when that bound exceeds the buffer's compile-time length. The cursor does
/// not move (the epilogue's write-back stores it unchanged, keeping the
/// shared prologue/epilogue and their relocation offsets identical to the
/// other wire decodes).
pub fn encode_read_wire_nested_open(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(read_wire_nested_open_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        end_offset,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    // x25 = the end-slot page, x26 = the LENGTH stored there.
    bytes.extend(encode_adrp_placeholder(25));
    bytes.extend(encode_add_page_offset_placeholder(25));
    super::runtime_storage::append_load_data_from_x_offset(&mut bytes, 26, 25, end_offset, 8, 19)?;

    // ok &= length <= buffer length (a raw length past the buffer could wrap
    // the 64-bit end sum back inside the bound -- reject it before adding);
    // then end = cursor + length and ok &= end <= buffer length. The cursor
    // never exceeds the buffer length and the length just passed its own
    // check, so the sum cannot wrap. Fixed eight-instruction block (32
    // bytes):
    //          x24 = buffer_length (materialized above this block)
    //          movz x23, #1
    //          cmp  x24, x26
    //          b.hs len_ok          (+8: skip the fail movz)
    //   fail1: movz x23, #0
    //  len_ok: add  x26, x26, x17
    //          cmp  x24, x26
    //          b.hs done            (+8: bound fits -- skip the fail movz)
    //   fail2: movz x23, #0
    //   done:
    append_unsigned_immediate(&mut bytes, 24, buffer_length as u64);
    bytes.extend(encode_movz(23, 1));
    bytes.extend(encode_compare_x_register(24, 26));
    bytes.extend(encode_conditional_branch_higher_or_same(8)?);
    bytes.extend(encode_movz(23, 0));
    bytes.extend(encode_add_x_register(26, 26, 17));
    bytes.extend(encode_compare_x_register(24, 26));
    bytes.extend(encode_conditional_branch_higher_or_same(8)?);
    bytes.extend(encode_movz(23, 0));

    super::runtime_storage::append_store_data_to_x_offset(&mut bytes, 26, 25, end_offset, 8, 19)?;

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_nested_open_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset
        )
    );
    Ok(bytes)
}

/// Close a nested sub-message region (chapter 20, nested message fields):
/// clear ok unless the cursor landed EXACTLY on the end bound the matching
/// open stored -- the declared sub-message length must equal the bytes its
/// fields consumed. The cursor does not move.
pub fn encode_read_wire_nested_close(
    buffer_offset: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(read_wire_nested_close_width(
        buffer_offset,
        read_offset,
        ok_offset,
        end_offset,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    // x25 = the end-slot page, x26 = the end bound stored there.
    bytes.extend(encode_adrp_placeholder(25));
    bytes.extend(encode_add_page_offset_placeholder(25));
    super::runtime_storage::append_load_data_from_x_offset(&mut bytes, 26, 25, end_offset, 8, 19)?;

    // ok &= cursor == end:
    //         movz x23, #1
    //         cmp  x17, x26
    //         b.eq done            (+8: skip the fail movz)
    //   fail: movz x23, #0
    //   done:
    bytes.extend(encode_movz(23, 1));
    bytes.extend(encode_compare_x_register(17, 26));
    bytes.extend(encode_conditional_branch_equal(8)?);
    bytes.extend(encode_movz(23, 0));

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_nested_close_width(buffer_offset, read_offset, ok_offset, end_offset)
    );
    Ok(bytes)
}
