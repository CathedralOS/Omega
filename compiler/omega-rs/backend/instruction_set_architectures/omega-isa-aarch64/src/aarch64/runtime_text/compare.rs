use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    append_add_x_constant, append_unsigned_immediate_w_padded,
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_cbz_x,
    encode_compare_w_immediate, encode_compare_w_register, encode_compare_w17_immediate,
    encode_compare_x_register, encode_conditional_branch_equal, encode_conditional_branch_lower,
    encode_conditional_branch_not_equal, encode_load_byte_w_from_x,
    encode_load_byte_w_post_increment, encode_load_byte_w17_from_x16, encode_load_x_from_x,
    encode_move_x_register, encode_runtime_text_input_delimiter_check_bytes,
    encode_subs_x_immediate, encode_unconditional_branch,
};
use super::super::widths::{
    runtime_text_literal_compare_width, runtime_text_storage_compare_width,
};

pub fn encode_runtime_text_literal_compare(
    literal: &str,
    failure_branch_distances: impl ExactSizeIterator<Item = isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let failure_branch_distance_count = failure_branch_distances.len();
    if literal.len() != failure_branch_distance_count {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime text guard expected {} branch distance(s), got {}",
            literal.len(),
            failure_branch_distance_count
        )));
    }

    let mut bytes = Vec::with_capacity(runtime_text_literal_compare_width(literal));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));

    for (byte_index, (expected_byte, failure_branch_distance)) in literal
        .as_bytes()
        .iter()
        .zip(failure_branch_distances)
        .enumerate()
    {
        bytes.extend(encode_load_byte_w17_from_x16(byte_index)?);
        bytes.extend(encode_compare_w17_immediate(u32::from(*expected_byte))?);
        bytes.extend(encode_conditional_branch_not_equal(
            failure_branch_distance,
        )?);
    }

    bytes.extend(encode_runtime_text_input_delimiter_check_bytes(
        literal.len(),
        delimiter_failure_branch_distance,
    )?);
    Ok(bytes)
}

/// Content-compare a stored `{ptr, len}` text descriptor against a literal,
/// mirroring the x86_64 reference EXACTLY:
///
///   stored.len < literal_len            -> MISMATCH (fall through to the end)
///   any of the first literal_len bytes  -> MISMATCH
///   stored.len == literal_len           -> MATCH (external branch, distance 1)
///   else stored[literal_len] is \n/\r/\0 -> MATCH (read_line's trailing
///                                          terminator convention), else MISMATCH
///
/// The old shape looped `stored.len` bytes with no length compare and checked
/// the delimiter on the LITERAL side -- rodata past the literal's end -- so
/// two EQUAL strings compared unequal whenever the next rodata byte was not a
/// terminator. MATCH branches externally by `match_branch_distance` (anchored
/// at the terminal branch, `runtime_text_storage_compare_failure_branch_offset`);
/// MISMATCH falls through to the instruction end (the trailing "write 0").
/// The second distance and `branch_when_equal` are unused, like x86_64.
pub fn encode_runtime_text_storage_compare_bytes(
    source_offset: usize,
    literal_len: usize,
    match_branch_distance: isize,
    _delimiter_failure_branch_distance: isize,
    _branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let literal_len_w = u32::try_from(literal_len).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 encoder cannot compare a text literal of length `{literal_len}` yet"
        ))
    })?;
    let mut bytes = Vec::with_capacity(runtime_text_storage_compare_width(source_offset));
    bytes.extend(encode_adrp_placeholder(16)); // literal base [reloc @ 0]
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17)); // source region base [reloc @ 8]
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 26, 17, source_offset, 15)?; // stored.ptr
    append_load_x_from_x_offset(&mut bytes, 19, 17, source_offset + 8, 15)?; // stored.len
    append_unsigned_immediate_w_padded(&mut bytes, 14, literal_len_w); // fixed 8 bytes

    // Fixed 22-instruction body; indexes below are instruction counts from
    // here, so every branch distance is a compile-time constant.
    //  i0 cmp  x19, x14          i11 b.eq  -> MATCH(i21)
    //  i1 b.lo -> MISMATCH(i20)  i12 ldrb  w17, [x26]
    //  i2 mov  x15, x14          i13 cmp   w17, #10
    //  i3 cbz  x15 -> TAIL(i10)  i14 b.eq  -> MATCH
    //  i4 ldrb w20, [x26], #1    i15 cmp   w17, #13
    //  i5 ldrb w21, [x16], #1    i16 b.eq  -> MATCH
    //  i6 cmp  w20, w21          i17 cmp   w17, #0
    //  i7 b.ne -> MISMATCH       i18 b.eq  -> MATCH
    //  i8 subs x15, x15, #1      i19 b     -> MISMATCH
    //  i9 b    -> i3             i20 MISMATCH: b -> END (+8)
    // i10 cmp  x19, x14          i21 MATCH: b <match_branch_distance>
    bytes.extend(encode_compare_x_register(19, 14)); // i0
    bytes.extend(encode_conditional_branch_lower((20 - 1) * 4)?); // i1
    bytes.extend(encode_move_x_register(15, 14)); // i2
    bytes.extend(encode_cbz_x(15, (10 - 3) * 4)?); // i3
    bytes.extend(encode_load_byte_w_post_increment(20, 26, 1)?); // i4
    bytes.extend(encode_load_byte_w_post_increment(21, 16, 1)?); // i5
    bytes.extend(encode_compare_w_register(20, 21)); // i6
    bytes.extend(encode_conditional_branch_not_equal((20 - 7) * 4)?); // i7
    bytes.extend(encode_subs_x_immediate(15, 15, 1)?); // i8
    bytes.extend(encode_unconditional_branch(-((9 - 3) * 4))?); // i9
    bytes.extend(encode_compare_x_register(19, 14)); // i10
    bytes.extend(encode_conditional_branch_equal((21 - 11) * 4)?); // i11
    bytes.extend(encode_load_byte_w_from_x(17, 26, 0)?); // i12 stored[literal_len]
    bytes.extend(encode_compare_w_immediate(17, 10)?); // i13
    bytes.extend(encode_conditional_branch_equal((21 - 14) * 4)?); // i14
    bytes.extend(encode_compare_w_immediate(17, 13)?); // i15
    bytes.extend(encode_conditional_branch_equal((21 - 16) * 4)?); // i16
    bytes.extend(encode_compare_w_immediate(17, 0)?); // i17
    bytes.extend(encode_conditional_branch_equal((21 - 18) * 4)?); // i18
    bytes.extend(encode_unconditional_branch(4)?); // i19 -> i20
    bytes.extend(encode_unconditional_branch(8)?); // i20 MISMATCH -> END
    bytes.extend(encode_unconditional_branch(match_branch_distance)?); // i21 MATCH
    debug_assert_eq!(
        bytes.len(),
        runtime_text_storage_compare_width(source_offset)
    );
    Ok(bytes)
}

fn append_load_x_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if data_offset_encodable(byte_offset, 8) {
        bytes.extend(encode_load_x_from_x(
            destination_register,
            base_register,
            byte_offset,
        )?);
    } else {
        bytes.extend(encode_move_x_register(scratch_register, base_register));
        append_add_x_constant(bytes, scratch_register, scratch_register, byte_offset, 14)?;
        bytes.extend(encode_load_x_from_x(
            destination_register,
            scratch_register,
            0,
        )?);
    }

    Ok(())
}

fn data_offset_encodable(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}
