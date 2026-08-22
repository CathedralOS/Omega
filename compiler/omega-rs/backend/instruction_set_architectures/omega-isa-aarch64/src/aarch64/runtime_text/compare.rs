use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

use super::super::primitives::{
    append_add_x_constant, append_unsigned_immediate_w_padded, encode_add_page_offset_placeholder,
    encode_adrp_placeholder, encode_cbz_x, encode_compare_w_immediate, encode_compare_w_register,
    encode_compare_w17_immediate, encode_compare_x_register, encode_conditional_branch_equal,
    encode_conditional_branch_lower, encode_conditional_branch_not_equal,
    encode_load_byte_w_from_x, encode_load_byte_w_post_increment, encode_load_byte_w17_from_x16,
    encode_load_x_from_x, encode_move_x_register, encode_runtime_text_input_delimiter_check_bytes,
    encode_subs_x_immediate, encode_unconditional_branch,
};
use super::super::widths::{
    runtime_text_literal_compare_width, runtime_text_storage_compare_width,
};

pub fn encode_runtime_text_literal_compare(
    literal: &[u8],
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

    for (byte_index, (expected_byte, failure_branch_distance)) in
        literal.iter().zip(failure_branch_distances).enumerate()
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

/// Exact register writes of the literal-buffer guard encoder. x16 holds the
/// relocated buffer base and w17 receives each compared/delimiter byte.
pub fn runtime_text_literal_compare_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)])
}

pub fn runtime_text_literal_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
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
/// The second distance is unused, like x86_64. `negated` (true when the
/// source operator is `!=`) SWAPS which outcome takes the external branch:
/// the frame-slot text-comparison writer emits [preset 1][compare:
/// hold->skip][write 0], so `==` skips on MATCH and `!=` must skip on
/// MISMATCH -- the flag was ignored and `!=` behaved as `==` (`name !=
/// "omega"` with equal strings kept the preset 1 and took the wrong arm,
/// native-vs-interp divergent; the interpreter honors the operator).
pub fn encode_runtime_text_storage_compare_bytes(
    source_offset: usize,
    literal_len: usize,
    match_branch_distance: isize,
    _delimiter_failure_branch_distance: isize,
    negated: bool,
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
    // Exit routing: i21 carries the EXTERNAL branch, i20 falls through to
    // the instruction end. `==` sends MATCH outcomes to i21; `!=` (negated)
    // sends MISMATCH outcomes there instead -- same 22-instruction body,
    // only the interior branch targets differ.
    let match_i: isize = if negated { 20 } else { 21 };
    let mismatch_i: isize = if negated { 21 } else { 20 };
    bytes.extend(encode_compare_x_register(19, 14)); // i0
    bytes.extend(encode_conditional_branch_lower((mismatch_i - 1) * 4)?); // i1
    bytes.extend(encode_move_x_register(15, 14)); // i2
    bytes.extend(encode_cbz_x(15, (10 - 3) * 4)?); // i3
    bytes.extend(encode_load_byte_w_post_increment(20, 26, 1)?); // i4
    bytes.extend(encode_load_byte_w_post_increment(21, 16, 1)?); // i5
    bytes.extend(encode_compare_w_register(20, 21)); // i6
    bytes.extend(encode_conditional_branch_not_equal((mismatch_i - 7) * 4)?); // i7
    bytes.extend(encode_subs_x_immediate(15, 15, 1)?); // i8
    bytes.extend(encode_unconditional_branch(-((9 - 3) * 4))?); // i9
    bytes.extend(encode_compare_x_register(19, 14)); // i10
    bytes.extend(encode_conditional_branch_equal((match_i - 11) * 4)?); // i11
    bytes.extend(encode_load_byte_w_from_x(17, 26, 0)?); // i12 stored[literal_len]
    bytes.extend(encode_compare_w_immediate(17, 10)?); // i13
    bytes.extend(encode_conditional_branch_equal((match_i - 14) * 4)?); // i14
    bytes.extend(encode_compare_w_immediate(17, 13)?); // i15
    bytes.extend(encode_conditional_branch_equal((match_i - 16) * 4)?); // i16
    bytes.extend(encode_compare_w_immediate(17, 0)?); // i17
    bytes.extend(encode_conditional_branch_equal((match_i - 18) * 4)?); // i18
    bytes.extend(encode_unconditional_branch((mismatch_i - 19) * 4)?); // i19
    bytes.extend(encode_unconditional_branch(8)?); // i20 fall-through -> END
    bytes.extend(encode_unconditional_branch(match_branch_distance)?); // i21 EXTERNAL
    debug_assert_eq!(
        bytes.len(),
        runtime_text_storage_compare_width(source_offset)
    );
    Ok(bytes)
}

/// Exact register writes of the descriptor-vs-literal content comparison.
/// This includes both relocated bases, descriptor pointer/length, fixed loop
/// counters, byte operands, and the large-offset/address scratch.
pub fn runtime_text_storage_compare_register_writes() -> RegisterSet {
    RegisterSet::new([14, 15, 16, 17, 19, 20, 21, 26].map(MachineRegister::Aarch64X))
}

pub fn runtime_text_storage_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
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
