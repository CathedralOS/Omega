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

use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use omega_target_operations::RuntimeStorageRegion;
use psi_diagnostics::Diagnostic;

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate, encode_add_page_offset_placeholder,
    encode_add_x_immediate, encode_add_x_register, encode_adrp_placeholder,
    encode_and_x_immediate_low_seven, encode_and_x_register, encode_asr_x_immediate, encode_cbnz_x,
    encode_compare_w_immediate, encode_compare_x_immediate, encode_compare_x_register,
    encode_conditional_branch_equal, encode_conditional_branch_higher,
    encode_conditional_branch_higher_or_same, encode_conditional_branch_less,
    encode_conditional_branch_less_or_equal, encode_conditional_branch_lower,
    encode_conditional_branch_lower_or_same, encode_conditional_branch_not_equal,
    encode_eor_x_register, encode_load_byte_w_post_increment, encode_lslv_x_register,
    encode_lsr_x_immediate, encode_movz, encode_orr_x_register, encode_unconditional_branch,
};
use super::widths::{
    read_wire_byte_slice_width, read_wire_expected_byte_width, read_wire_nested_close_width,
    read_wire_nested_open_width, read_wire_repeated_scalar_varint_width,
    read_wire_scalar_varint_width, wire_unzigzag_width, wire_varint_read_loop_width,
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

pub fn read_wire_expected_byte_clobbers(read_offset: usize, ok_offset: usize) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(21),
        MachineRegister::Aarch64X(23),
        MachineRegister::Aarch64X(24),
    ];
    if super::runtime_storage::data_offset_uses_scratch(read_offset, 8)
        || super::runtime_storage::data_offset_uses_scratch(ok_offset, 1)
    {
        registers.push(MachineRegister::Aarch64X(25));
    }
    RegisterSet::new(registers)
}

pub fn read_wire_expected_byte_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// LEB128-read a runtime scalar at the cursor into the target place. The
/// loop is a fixed fourteen-instruction body whose iteration count is data
/// dependent but whose EMITTED width is constant (the widths invariant):
/// truncation (cursor past the buffer length) and overlong varints (a
/// continuation past shift 63, i.e. more than ten groups) branch to the fail
/// arm. Signed targets un-zigzag (`(n >> 1) ^ -(n & 1)`) before the store;
/// the store truncates to the field width.
/// Borrowed `&[u8]` ZERO-COPY decode: LEB128-read a byte LENGTH at the cursor,
/// bounds-check the content against the buffer's compile-time length, store a
/// fat `{ptr, len}` descriptor VIEWING the buffer in place (ptr = the content's
/// address, len = the decoded length), and advance the cursor past the content.
/// Mirrors the x86_64 encoder; the length read reuses the shared LEB128 loop
/// (after it x26 = length, x16 = content pointer, x17 = cursor).
#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_byte_slice(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    predicate_mask: u8,
) -> Result<Vec<u8>, Diagnostic> {
    // The region only picks the relocation symbol; the encoded shape is identical.
    let _ = target_region;

    let mut bytes = Vec::with_capacity(read_wire_byte_slice_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        target_offset,
        predicate_mask,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;
    append_unsigned_immediate(&mut bytes, 24, buffer_length as u64);
    bytes.extend(encode_movz(23, 1)); // ok success bit
    bytes.extend(encode_movz(26, 0)); // value (length) accumulator
    bytes.extend(encode_movz(22, 0)); // shift

    // Shared canonical LEB128 read loop: reads the length varint into x26,
    // leaving x16 pointing at the CONTENT and x17 at the post-varint cursor.
    append_wire_varint_read_loop(&mut bytes)?;

    // Bounds + advance (fixed 24 bytes): end = cursor + len; if end >
    // buffer_length clear ok; cursor = end (matches the x86_64 `jbe`/clear
    // shape). x16 (content pointer) is preserved for the descriptor store.
    //   add  x19, x17, x26       ; end = cursor + len
    //   cmp  x19, x24            ; end vs buffer_length
    //   b.hi clear (+8)          ; out of bounds -> clear ok
    //   b    advance (+8)        ; in bounds -> skip the clear
    //   clear: movz x23, #0
    //   advance: add x17, x19, #0  (x17 = end)
    bytes.extend(encode_add_x_register(19, 17, 26));
    bytes.extend(encode_compare_x_register(19, 24));
    bytes.extend(encode_conditional_branch_higher(8)?);
    bytes.extend(encode_unconditional_branch(8)?);
    bytes.extend(encode_movz(23, 0));
    bytes.extend(encode_add_x_immediate(17, 19, 0)?);

    // Decode-boundary byte-domain validation over the just-decoded content
    // (ptr x16, len x26): every predicate in the mask checks the UNTRUSTED
    // bytes and clears the sticky ok bit (x23) on violation -- interp
    // parity for `&[u8] in Utf8`-style wire fields.
    append_wire_byte_predicate_checks(&mut bytes, predicate_mask)?;

    // Store the descriptor: ptr = x16 (content start) @ +0, len = x26 @ +8.
    bytes.extend(encode_adrp_placeholder(25));
    bytes.extend(encode_add_page_offset_placeholder(25));
    super::runtime_storage::append_store_data_to_x_offset(
        &mut bytes,
        16,
        25,
        target_offset,
        8,
        19,
    )?;
    super::runtime_storage::append_store_data_to_x_offset(
        &mut bytes,
        26,
        25,
        target_offset + 8,
        8,
        19,
    )?;

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_byte_slice_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            predicate_mask,
        )
    );
    Ok(bytes)
}

pub fn read_wire_byte_slice_clobbers() -> RegisterSet {
    read_wire_scalar_varint_clobbers()
}

pub fn read_wire_byte_slice_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// Decode-boundary byte-domain validation blocks (one per predicate in the
/// mask, in `ByteSequencePredicate::ALL` order so widths are deterministic).
/// Contract: content ptr in x16, length in x26, sticky ok bit in x23; x19
/// (walking pointer), x22 (end bound), x25 (lead byte), x24 (continuation
/// byte) are spent scratch at this point in the byte-slice sequence -- the
/// target adrp pair claims x25 only AFTER these checks. Widths MUST match
/// `wire_byte_predicate_checks_width`.
fn append_wire_byte_predicate_checks(
    bytes: &mut Vec<u8>,
    predicate_mask: u8,
) -> Result<(), Diagnostic> {
    use psi_language_semantics::byte_predicates::ByteSequencePredicate;
    for predicate in ByteSequencePredicate::in_mask(predicate_mask) {
        match predicate {
            ByteSequencePredicate::NonEmpty => {
                // A zero length violates non_empty: cbnz skips the clear.
                bytes.extend(encode_cbnz_x(26, 8)?);
                bytes.extend(encode_movz(23, 0));
            }
            ByteSequencePredicate::NoNul => {
                bytes.extend(encode_add_x_immediate(19, 16, 0)?); // p = start
                bytes.extend(encode_add_x_register(22, 16, 26)); // end
                bytes.extend(encode_compare_x_register(19, 22)); // loop:
                bytes.extend(encode_conditional_branch_higher_or_same(16)?); // -> done
                bytes.extend(encode_load_byte_w_post_increment(25, 19, 1)?);
                bytes.extend(encode_cbnz_x(25, -12)?); // nonzero -> loop
                bytes.extend(encode_movz(23, 0)); // a NUL byte -> clear ok
            }
            ByteSequencePredicate::AsciiOnly => {
                bytes.extend(encode_add_x_immediate(19, 16, 0)?);
                bytes.extend(encode_add_x_register(22, 16, 26));
                bytes.extend(encode_compare_x_register(19, 22)); // loop:
                bytes.extend(encode_conditional_branch_higher_or_same(20)?); // -> done
                bytes.extend(encode_load_byte_w_post_increment(25, 19, 1)?);
                bytes.extend(encode_compare_w_immediate(25, 0x80)?);
                bytes.extend(encode_conditional_branch_lower(-16)?); // < 0x80 -> loop
                bytes.extend(encode_movz(23, 0));
            }
            ByteSequencePredicate::ValidUtf8 => {
                append_wire_utf8_validation(bytes)?;
            }
        }
    }
    Ok(())
}

/// UTF-8 validation over [x16, x16+x26): a decoded-scalar walk with pure
/// compare/branch range checks (no logical-immediate encodings). Lead-byte
/// classes: ASCII < 0x80; C2..DF need one continuation; E0..EF need two,
/// with E0 requiring cont1 >= A0 (overlongs) and ED requiring cont1 < A0
/// (surrogates); F0..F4 need three, with F0 requiring cont1 >= 90 and F4
/// requiring cont1 < 90 (above U+10FFFF); 0x80..0xC1 and 0xF5.. are invalid
/// leads. Every continuation must sit in [0x80, 0xC0). On any violation the
/// sticky ok bit (x23) clears. Assembled with a local two-pass label
/// resolver -- 60+ hand-computed offsets would be write-only.
fn append_wire_utf8_validation(bytes: &mut Vec<u8>) -> Result<(), Diagnostic> {
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum Label {
        Loop,
        Two,
        Three,
        NotE0,
        NotEd,
        NotF0,
        NotF4,
        Fail,
        Done,
    }
    enum Ins {
        Fixed([u8; 4]),
        /// (condition-encoder index, target): resolved on the second pass.
        BHs(Label),
        BLo(Label),
        BNe(Label),
        B(Label),
    }
    use Ins::*;
    use Label::*;

    // One plain continuation read: bounds, load, range [0x80, 0xC0).
    fn plain_continuation(program: &mut Vec<(Option<Label>, Ins)>, at: Option<Label>) {
        program.push((at, Fixed(encode_compare_x_register(19, 22))));
        program.push((None, BHs(Fail))); // truncated sequence
        program.push((
            None,
            Fixed(
                encode_load_byte_w_post_increment(24, 19, 1)
                    .expect("byte load with +1 post-increment encodes"),
            ),
        ));
        program.push((
            None,
            Fixed(encode_compare_w_immediate(24, 0x80).expect("imm12 compare encodes")),
        ));
        program.push((None, BLo(Fail)));
        program.push((
            None,
            Fixed(encode_compare_w_immediate(24, 0xC0).expect("imm12 compare encodes")),
        ));
        program.push((None, BHs(Fail)));
    }

    let mut program: Vec<(Option<Label>, Ins)> = Vec::new();
    program.push((None, Fixed(encode_add_x_immediate(19, 16, 0)?))); // p = start
    program.push((None, Fixed(encode_add_x_register(22, 16, 26)))); // end
    program.push((Some(Loop), Fixed(encode_compare_x_register(19, 22))));
    program.push((None, BHs(Done)));
    program.push((None, Fixed(encode_load_byte_w_post_increment(25, 19, 1)?))); // lead
    program.push((None, Fixed(encode_compare_w_immediate(25, 0x80)?)));
    program.push((None, BLo(Loop))); // ASCII
    program.push((None, Fixed(encode_compare_w_immediate(25, 0xC2)?)));
    program.push((None, BLo(Fail))); // 0x80..0xC1: invalid lead
    program.push((None, Fixed(encode_compare_w_immediate(25, 0xE0)?)));
    program.push((None, BLo(Two))); // C2..DF
    program.push((None, Fixed(encode_compare_w_immediate(25, 0xF0)?)));
    program.push((None, BLo(Three))); // E0..EF
    program.push((None, Fixed(encode_compare_w_immediate(25, 0xF5)?)));
    program.push((None, BHs(Fail))); // F5..: invalid lead

    // FOUR-byte lead F0..F4: cont1 (with the F0/F4 specials), then two plain.
    plain_continuation(&mut program, None);
    program.push((None, Fixed(encode_compare_w_immediate(25, 0xF0)?)));
    program.push((None, BNe(NotF0)));
    program.push((None, Fixed(encode_compare_w_immediate(24, 0x90)?)));
    program.push((None, BLo(Fail))); // F0: cont1 >= 0x90 (overlong)
    program.push((Some(NotF0), Fixed(encode_compare_w_immediate(25, 0xF4)?)));
    program.push((None, BNe(NotF4)));
    program.push((None, Fixed(encode_compare_w_immediate(24, 0x90)?)));
    program.push((None, BHs(Fail))); // F4: cont1 < 0x90 (above U+10FFFF)
    program.push((Some(NotF4), Fixed(encode_compare_x_register(19, 22))));
    program.push((None, BHs(Fail)));
    program.push((None, Fixed(encode_load_byte_w_post_increment(24, 19, 1)?)));
    program.push((None, Fixed(encode_compare_w_immediate(24, 0x80)?)));
    program.push((None, BLo(Fail)));
    program.push((None, Fixed(encode_compare_w_immediate(24, 0xC0)?)));
    program.push((None, BHs(Fail)));
    plain_continuation(&mut program, None);
    program.push((None, B(Loop)));

    // THREE-byte lead E0..EF: cont1 (with the E0/ED specials), then one plain.
    plain_continuation(&mut program, Some(Three));
    program.push((None, Fixed(encode_compare_w_immediate(25, 0xE0)?)));
    program.push((None, BNe(NotE0)));
    program.push((None, Fixed(encode_compare_w_immediate(24, 0xA0)?)));
    program.push((None, BLo(Fail))); // E0: cont1 >= 0xA0 (overlong)
    program.push((Some(NotE0), Fixed(encode_compare_w_immediate(25, 0xED)?)));
    program.push((None, BNe(NotEd)));
    program.push((None, Fixed(encode_compare_w_immediate(24, 0xA0)?)));
    program.push((None, BHs(Fail))); // ED: cont1 < 0xA0 (no surrogates)
    program.push((Some(NotEd), Fixed(encode_compare_x_register(19, 22))));
    program.push((None, BHs(Fail)));
    program.push((None, Fixed(encode_load_byte_w_post_increment(24, 19, 1)?)));
    program.push((None, Fixed(encode_compare_w_immediate(24, 0x80)?)));
    program.push((None, BLo(Fail)));
    program.push((None, Fixed(encode_compare_w_immediate(24, 0xC0)?)));
    program.push((None, BHs(Fail)));
    program.push((None, B(Loop)));

    // TWO-byte lead C2..DF: one plain continuation.
    plain_continuation(&mut program, Some(Two));
    program.push((None, B(Loop)));

    program.push((Some(Fail), Fixed(encode_movz(23, 0))));
    // Done = the first instruction AFTER the block.

    // Pass 1: label -> instruction index.
    let mut positions = std::collections::HashMap::new();
    for (index, (label, _)) in program.iter().enumerate() {
        if let Some(label) = label {
            positions.insert(*label, index);
        }
    }
    positions.insert(Done, program.len());
    // Pass 2: emit with resolved byte offsets.
    for (index, (_, instruction)) in program.iter().enumerate() {
        let offset =
            |target: &Label| -> isize { (positions[target] as isize - index as isize) * 4 };
        match instruction {
            Fixed(word) => bytes.extend(word),
            BHs(target) => bytes.extend(encode_conditional_branch_higher_or_same(offset(target))?),
            BLo(target) => bytes.extend(encode_conditional_branch_lower(offset(target))?),
            BNe(target) => bytes.extend(encode_conditional_branch_not_equal(offset(target))?),
            B(target) => bytes.extend(encode_unconditional_branch(offset(target))?),
        }
    }
    Ok(())
}

fn append_wire_varint_read_loop(bytes: &mut Vec<u8>) -> Result<(), Diagnostic> {
    let start = bytes.len();
    // Canonical unsigned LEB128. x22 is the current shift before a load and
    // the next shift after it. The tenth-group range check occurs before x25
    // is shifted; after a terminal group, shifted x25 is zero exactly when
    // the terminal payload was zero. This preserves x20/x21 for the epilogue.
    bytes.extend(encode_compare_x_register(17, 24));
    bytes.extend(encode_conditional_branch_higher_or_same(76)?);
    bytes.extend(encode_compare_w_immediate(22, 63)?);
    bytes.extend(encode_conditional_branch_higher(68)?);
    bytes.extend(encode_load_byte_w_post_increment(19, 16, 1)?);
    bytes.extend(encode_add_x_immediate(17, 17, 1)?);
    bytes.extend(encode_and_x_immediate_low_seven(25, 19));
    bytes.extend(encode_compare_w_immediate(22, 63)?);
    bytes.extend(encode_conditional_branch_not_equal(12)?);
    bytes.extend(encode_compare_w_immediate(25, 1)?);
    bytes.extend(encode_conditional_branch_higher(40)?);
    bytes.extend(encode_lslv_x_register(25, 25, 22));
    bytes.extend(encode_orr_x_register(26, 26, 25));
    bytes.extend(encode_add_x_immediate(22, 22, 7)?);
    bytes.extend(encode_lsr_x_immediate(19, 19, 7));
    bytes.extend(encode_cbnz_x(19, -60)?);
    bytes.extend(encode_compare_w_immediate(22, 7)?);
    bytes.extend(encode_conditional_branch_equal(16)?);
    // x25 is the SHIFTED terminal payload. At the legal ten-group
    // u64::MAX boundary it is bit 63, whose low W view is zero; compare the
    // full X register so that canonical value is not mistaken for an
    // overlong zero terminal group.
    bytes.extend(encode_compare_x_immediate(25, 0)?);
    bytes.extend(encode_conditional_branch_not_equal(8)?);
    bytes.extend(encode_movz(23, 0));
    debug_assert_eq!(bytes.len() - start, wire_varint_read_loop_width());
    Ok(())
}

pub fn encode_read_wire_scalar_varint(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
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
        range,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;
    append_unsigned_immediate(&mut bytes, 24, buffer_length as u64);
    bytes.extend(encode_movz(23, 1));
    bytes.extend(encode_movz(26, 0));
    bytes.extend(encode_movz(22, 0));

    // Canonical LEB128 read loop (fixed width; see the shared emitter).
    append_wire_varint_read_loop(&mut bytes)?;

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
    let mut store = Vec::new();
    super::runtime_storage::append_store_data_to_x_offset(
        &mut store,
        26,
        25,
        target_offset,
        byte_size,
        19,
    )?;
    if let Some(range) = range {
        // Establish the destination interval before the constrained field is
        // written. x26 retains the decoded value, x19 carries each bound, and
        // x23 is this operation's sticky success bit.
        append_unsigned_immediate(&mut bytes, 19, range.minimum as u64);
        bytes.extend(encode_compare_x_register(26, 19));
        let maximum_width = super::widths::unsigned_immediate_width(range.maximum as u64);
        bytes.extend(if range.signed {
            encode_conditional_branch_less((maximum_width + 12) as isize)?
        } else {
            encode_conditional_branch_lower((maximum_width + 12) as isize)?
        });
        append_unsigned_immediate(&mut bytes, 19, range.maximum as u64);
        bytes.extend(encode_compare_x_register(26, 19));
        bytes.extend(if range.signed {
            encode_conditional_branch_less_or_equal(12)?
        } else {
            encode_conditional_branch_lower_or_same(12)?
        });
        bytes.extend(encode_movz(23, 0)); // fail
        bytes.extend(encode_unconditional_branch((store.len() + 4) as isize)?);
    }
    bytes.extend(store);

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
            zigzag,
            range,
        )
    );
    Ok(bytes)
}

pub fn read_wire_scalar_varint_clobbers() -> RegisterSet {
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
    ])
}

pub fn read_wire_scalar_varint_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn read_wire_repeated_scalar_varint_clobbers() -> RegisterSet {
    read_wire_scalar_varint_clobbers()
}

pub fn read_wire_repeated_scalar_varint_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// LEB128-read one packed repeated element at the cursor into the target
/// slot, ONLY IF the cursor sits strictly below the end bound the
/// surrounding nested OPEN stored; the taken path also increments the
/// FixedVec `length` slot. A skipped read changes nothing -- the branch lands
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
    range: Option<psi_language_semantics::wire::WireScalarRange>,
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
        range,
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
        + range.map_or(0, |range| {
            super::widths::unsigned_immediate_width(range.minimum as u64)
                + super::widths::unsigned_immediate_width(range.maximum as u64)
                + 24
        })
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

    append_wire_varint_read_loop(&mut bytes)?;

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
    let mut store = Vec::new();
    super::runtime_storage::append_store_data_to_x_offset(
        &mut store,
        26,
        25,
        target_offset,
        byte_size,
        19,
    )?;
    if let Some(range) = range {
        // Preserve the old element when hostile bytes violate its declared
        // range. The consumed element still advances the cursor and count;
        // x23 makes the overall verdict sticky-invalid.
        append_unsigned_immediate(&mut bytes, 19, range.minimum as u64);
        bytes.extend(encode_compare_x_register(26, 19));
        let maximum_width = super::widths::unsigned_immediate_width(range.maximum as u64);
        bytes.extend(if range.signed {
            encode_conditional_branch_less((maximum_width + 12) as isize)?
        } else {
            encode_conditional_branch_lower((maximum_width + 12) as isize)?
        });
        append_unsigned_immediate(&mut bytes, 19, range.maximum as u64);
        bytes.extend(encode_compare_x_register(26, 19));
        bytes.extend(if range.signed {
            encode_conditional_branch_less_or_equal(12)?
        } else {
            encode_conditional_branch_lower_or_same(12)?
        });
        bytes.extend(encode_movz(23, 0)); // fail
        bytes.extend(encode_unconditional_branch((store.len() + 4) as isize)?);
    }
    bytes.extend(store);

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
    super::runtime_storage::append_store_data_to_x_offset(&mut bytes, 19, 25, count_offset, 8, 22)?;

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
            zigzag,
            range,
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

pub fn read_wire_nested_open_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(21),
        MachineRegister::Aarch64X(23),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(25),
        MachineRegister::Aarch64X(26),
    ])
}

pub fn read_wire_nested_open_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
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

pub fn read_wire_nested_close_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(21),
        MachineRegister::Aarch64X(23),
        MachineRegister::Aarch64X(25),
        MachineRegister::Aarch64X(26),
    ])
}

pub fn read_wire_nested_close_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_byte_clobbers_include_offset_scratch_only_when_encoded() {
        assert_eq!(
            read_wire_expected_byte_clobbers(0, 8).as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(23),
                MachineRegister::Aarch64X(24),
            ]
        );
        assert!(
            read_wire_expected_byte_clobbers(32_768, 8).contains(MachineRegister::Aarch64X(25))
        );
        assert!(read_wire_expected_byte_clobbers(0, 4096).contains(MachineRegister::Aarch64X(25)));
    }

    #[test]
    fn ranged_scalar_decode_widths_match_encoded_bytes() {
        let ranges = [
            None,
            Some(psi_language_semantics::wire::WireScalarRange {
                minimum: 0,
                maximum: 100,
                signed: false,
            }),
            Some(psi_language_semantics::wire::WireScalarRange {
                minimum: -40,
                maximum: 9000,
                signed: true,
            }),
            Some(psi_language_semantics::wire::WireScalarRange {
                minimum: i64::MIN,
                maximum: i64::MAX,
                signed: true,
            }),
        ];
        for &range in &ranges {
            for &(byte_size, zigzag) in &[(4usize, false), (4, true), (8, false), (8, true)] {
                let bytes = encode_read_wire_scalar_varint(
                    200,
                    4096,
                    300,
                    308,
                    RuntimeStorageRegion::Machine,
                    5000,
                    byte_size,
                    zigzag,
                    range,
                )
                .expect("aarch64 ranged scalar decode should encode");
                assert_eq!(
                    bytes.len(),
                    read_wire_scalar_varint_width(
                        200, 4096, 300, 308, 5000, byte_size, zigzag, range,
                    )
                );
            }
        }
    }

    // The byte-slice decode's EMITTED length must equal its width function for
    // every operand combination (a drift segfaults via relocation misplacement).
    // The encoder's own `debug_assert_eq` enforces this in debug builds; this
    // test pins it across the variable-width prologue / buffer-length / store
    // paths and confirms the target-page adrp sits within the emitted bytes.
    #[test]
    fn byte_slice_decode_widths_match_encoded_bytes() {
        use psi_language_semantics::byte_predicates::ByteSequencePredicate;
        let all_predicates: u8 = ByteSequencePredicate::ALL
            .iter()
            .map(|p| p.mask_bit())
            .fold(0, |m, b| m | b);
        let masks = [
            0u8,
            ByteSequencePredicate::ValidUtf8.mask_bit(),
            ByteSequencePredicate::NonEmpty.mask_bit(),
            all_predicates,
        ];
        for &buffer_offset in &[0usize, 4, 200, 5000] {
            for &buffer_length in &[2usize, 64, 4096] {
                for &(read_offset, ok_offset, target_offset) in
                    &[(0usize, 8usize, 16usize), (40, 48, 56), (300, 308, 4096)]
                {
                    for &predicate_mask in &masks {
                        let bytes = encode_read_wire_byte_slice(
                            buffer_offset,
                            buffer_length,
                            read_offset,
                            ok_offset,
                            RuntimeStorageRegion::Machine,
                            target_offset,
                            predicate_mask,
                        )
                        .expect("aarch64 byte-slice decode should encode");
                        let width = read_wire_byte_slice_width(
                            buffer_offset,
                            buffer_length,
                            read_offset,
                            ok_offset,
                            target_offset,
                            predicate_mask,
                        );
                        assert_eq!(
                            bytes.len(),
                            width,
                            "width mismatch for buffer_offset={buffer_offset} buffer_length={buffer_length} read={read_offset} ok={ok_offset} target={target_offset} mask={predicate_mask:#04b}"
                        );
                        // The target-page adrp pair must land inside the
                        // instruction stream, before the two 8-byte descriptor
                        // stores + epilogue.
                        let target_page =
                            super::super::widths::wire_decode_byte_slice_target_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                predicate_mask,
                            );
                        assert!(
                            target_page + 8 <= bytes.len(),
                            "target-page offset {target_page} past end {} ",
                            bytes.len()
                        );
                        assert_eq!(
                            target_page % 4,
                            0,
                            "aarch64 instructions are 4-byte aligned"
                        );
                    }
                }
            }
        }
    }
}
