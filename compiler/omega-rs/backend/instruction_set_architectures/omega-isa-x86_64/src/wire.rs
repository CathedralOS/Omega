use super::{
    Reg64, append_add_r15_imm32, append_load_r10_from_r14, append_mov_r14_imm64,
    append_mov_r15_imm64, append_mov_reg_imm64, append_store_r10_to_r14, disp32,
};
use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

// ---- compact_binary v0 wire-encode appends (chapter 20, decision 10) ----
//
// Both operations share one cursor convention: the caller's `written` slot
// holds the running byte count, so every append loads it, stores through a
// moving pointer (`out base + out offset + cursor`), and writes the advanced
// cursor back. Register use: r15 = moving out pointer, r14 = written page,
// r10 = cursor, rax = runtime scalar, r11 = byte/zigzag scratch; the text
// append also uses r9 = source ptr and rcx = remaining copy count (r12 is the
// dispatch-state register and stays untouched).
//
// THE WIDTHS INVARIANT: every emitted byte must move the `_width` functions
// and the `wire_append_*_offset` relocation offsets below in exact lockstep,
// or relocations drift and the binary segfaults.

/// Shared prologue: `mov r15, imm64(out)` (10, relocated at the instruction
/// start) + `add r15, imm32(out_offset)` (7) + `mov r14, imm64(written)` (10,
/// relocated at +17) + `mov r10, [r14+written_offset]` (7) + `add r15, r10`
/// (3).
fn wire_append_prologue_width() -> usize {
    37
}

fn append_wire_append_prologue(
    bytes: &mut Vec<u8>,
    out_offset: usize,
    written_offset: usize,
) -> Result<(), Diagnostic> {
    append_mov_r15_imm64(bytes, 0);
    append_add_r15_imm32(bytes, out_offset)?;
    append_mov_r14_imm64(bytes, 0);
    append_load_r10_from_r14(bytes, written_offset)?;
    bytes.extend([0x4d, 0x01, 0xd7]); // add r15, r10
    Ok(())
}

pub fn append_wire_literal_byte_width(_out_offset: usize, _written_offset: usize) -> usize {
    // Prologue + `mov byte [r15], imm8` (4) + `inc r10` (3) + cursor store (7).
    wire_append_prologue_width() + 4 + 3 + 7
}

pub fn append_wire_literal_byte_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86R10,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn append_wire_literal_byte_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
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
    bytes.extend([0x41, 0xc6, 0x07, value]); // mov byte [r15], imm8
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_literal_byte_width(out_offset, written_offset)
    );
    Ok(bytes)
}

/// The sized scalar load from `[r11 + source_offset]` into rax: 64-bit and
/// 32-bit moves are 7 bytes, the zero-extending byte load (movzx) is 8, and a
/// 4-byte SIGNED source loads sign-extending (movsxd, 7).
fn wire_varint_source_load_width(byte_size: usize) -> usize {
    if byte_size == 1 { 8 } else { 7 }
}

/// `mov r11, rax` + `sar r11, 63` + `shl rax, 1` + `xor rax, r11`.
fn wire_zigzag_width() -> usize {
    14
}

/// The fixed LEB128 emit loop + final-byte tail (see the encoder body).
fn wire_varint_emit_loop_width() -> usize {
    40
}

pub fn append_wire_scalar_varint_width(
    _source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    _out_offset: usize,
    _written_offset: usize,
) -> usize {
    wire_append_prologue_width()
        + 10
        + wire_varint_source_load_width(byte_size)
        + if zigzag { wire_zigzag_width() } else { 0 }
        + wire_varint_emit_loop_width()
        + 7
}

pub fn append_wire_scalar_varint_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn append_wire_scalar_varint_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// LEB128-encode a runtime scalar at the cursor. The value loads zero-extended
/// at its source width; signed sources (`zigzag`) sign-extend to 64 bits and
/// zigzag (`(n << 1) ^ (n >> 63)`) before the emit loop.
pub fn encode_append_wire_scalar_varint(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 wire encoder cannot varint-encode {byte_size}-byte scalars yet"
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

    // r11 = source base (imm64 relocated at +37), rax = the scalar.
    append_mov_reg_imm64(&mut bytes, Reg64::R11, 0);
    let displacement = disp32(source_offset)?;
    match (byte_size, zigzag) {
        (8, _) => bytes.extend([0x49, 0x8b, 0x83]), // mov rax, [r11+disp32]
        (4, false) => bytes.extend([0x41, 0x8b, 0x83]), // mov eax, [r11+disp32]
        (4, true) => bytes.extend([0x49, 0x63, 0x83]), // movsxd rax, dword [r11+disp32]
        (1, _) => bytes.extend([0x41, 0x0f, 0xb6, 0x83]), // movzx eax, byte [r11+disp32]
        _ => unreachable!("byte_size validated above"),
    }
    bytes.extend(displacement.to_le_bytes());

    if zigzag {
        // zigzag(n) = (n << 1) ^ (n >> 63); r11 holds the sign mask.
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
        bytes.extend([0x48, 0xc1, 0xe0, 0x01]); // shl rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }

    // LEB128 emit loop (fixed 40 bytes, `wire_varint_emit_loop_width`):
    //   loop: mov  r11, rax
    //         and  r11, 0x7f
    //         shr  rax, 7
    //         test rax, rax
    //         je   last            (+18: skip or/store/inc/inc/jmp)
    //         or   r11, 0x80
    //         mov  [r15], r11b
    //         inc  r15
    //         inc  r10
    //         jmp  loop            (-34)
    //   last: mov  [r15], r11b
    //         inc  r10
    bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
    bytes.extend([0x49, 0x83, 0xe3, 0x7f]); // and r11, 0x7f
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x74, 0x12]); // je +18 -> last
    bytes.extend([0x49, 0x81, 0xcb, 0x80, 0x00, 0x00, 0x00]); // or r11, 0x80
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0xeb, 0xde]); // jmp -34 -> loop
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10

    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
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

/// The fixed bounds-checked byte-copy loop in `encode_append_wire_text_bytes`.
fn wire_text_copy_loop_width() -> usize {
    35
}

/// The compile-time out-buffer capacity as a `cmp r10, imm32` operand.
fn wire_encode_capacity_imm32(out_length: usize) -> Result<i32, Diagnostic> {
    i32::try_from(out_length).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 wire encoder cannot bounds-check a {out_length}-byte buffer yet"
        ))
    })
}

pub fn append_wire_text_bytes_width(
    _source_offset: usize,
    _out_offset: usize,
    _out_length: usize,
    _written_offset: usize,
) -> usize {
    // Prologue + source imm64 (10) + ptr load (7) + len load (7) + count copy
    // (3) + length-varint emit loop + dest-pointer re-sync inc (3) + bounded
    // copy loop + cursor store (7).
    wire_append_prologue_width()
        + 10
        + 7
        + 7
        + 3
        + wire_varint_emit_loop_width()
        + 3
        + wire_text_copy_loop_width()
        + 7
}

pub fn append_wire_text_bytes_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn append_wire_text_bytes_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
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
    source_region: omega_target_operations::RuntimeStorageRegion,
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

    // r11 = source base (imm64 relocated at +37), r9 = ptr, rax = len.
    append_mov_reg_imm64(&mut bytes, Reg64::R11, 0);
    bytes.extend([0x4d, 0x8b, 0x8b]); // mov r9, [r11+disp32]
    bytes.extend(disp32(source_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x8b, 0x83]); // mov rax, [r11+disp32]
    bytes.extend(disp32(source_offset + 8)?.to_le_bytes());
    // rcx keeps the byte count for the copy loop; the emit loop consumes rax.
    bytes.extend([0x48, 0x89, 0xc1]); // mov rcx, rax

    // The same fixed 40-byte LEB128 emit loop as the scalar varint (see
    // `encode_append_wire_scalar_varint`), here emitting the LENGTH.
    bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
    bytes.extend([0x49, 0x83, 0xe3, 0x7f]); // and r11, 0x7f
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x74, 0x12]); // je +18 -> last
    bytes.extend([0x49, 0x81, 0xcb, 0x80, 0x00, 0x00, 0x00]); // or r11, 0x80
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0xeb, 0xde]); // jmp -34 -> loop
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    // The emit loop's final store does not advance the dest pointer (the
    // scalar append ends there); re-sync r15 with the cursor for the copy.
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15

    // Bounded byte-copy loop (fixed 35 bytes, `wire_text_copy_loop_width`):
    //   copy: test rcx, rcx
    //         je   done            (+30: all bytes copied)
    //         cmp  r10, imm32(N)
    //         jae  done            (+21: capacity full -- drop the rest)
    //         movzx r11d, byte [r9]
    //         inc  r9
    //         mov  [r15], r11b
    //         inc  r15
    //         inc  r10
    //         dec  rcx
    //         jmp  copy            (-35)
    //   done:
    bytes.extend([0x48, 0x85, 0xc9]); // test rcx, rcx
    bytes.extend([0x74, 0x1e]); // je +30 -> done
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_encode_capacity_imm32(out_length)?.to_le_bytes());
    bytes.extend([0x73, 0x15]); // jae +21 -> done
    bytes.extend([0x45, 0x0f, 0xb6, 0x19]); // movzx r11d, byte [r9]
    bytes.extend([0x49, 0xff, 0xc1]); // inc r9
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0x48, 0xff, 0xc9]); // dec rcx
    bytes.extend([0xeb, 0xdd]); // jmp -35 -> copy

    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_text_bytes_width(source_offset, out_offset, out_length, written_offset)
    );
    Ok(bytes)
}

fn append_wire_rel32_branch(bytes: &mut Vec<u8>, opcode: &[u8]) -> usize {
    bytes.extend_from_slice(opcode);
    let displacement = bytes.len();
    bytes.extend(0i32.to_le_bytes());
    displacement
}

fn patch_wire_rel32(bytes: &mut [u8], displacement: usize, target: usize) {
    let relative = target as isize - (displacement as isize + 4);
    bytes[displacement..displacement + 4].copy_from_slice(&(relative as i32).to_le_bytes());
}

fn append_wire_slice_scalar_load(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    zigzag: bool,
) -> Result<(), Diagnostic> {
    match (byte_size, zigzag) {
        (8, _) => bytes.extend([0x49, 0x8b, 0x00]), // mov rax, [r8]
        (4, false) => bytes.extend([0x41, 0x8b, 0x00]), // mov eax, [r8]
        (4, true) => bytes.extend([0x49, 0x63, 0x00]), // movsxd rax, dword [r8]
        (1, _) => bytes.extend([0x41, 0x0f, 0xb6, 0x00]), // movzx eax, byte [r8]
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 wire encoder cannot varint-encode {byte_size}-byte slice elements yet"
            )));
        }
    }
    bytes.extend([0x49, 0x83, 0xc0, byte_size as u8]); // add r8, stride
    if zigzag {
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
        bytes.extend([0x48, 0xc1, 0xe0, 0x01]); // shl rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }
    Ok(())
}

fn append_wire_varint_emit_loop(bytes: &mut Vec<u8>) {
    bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
    bytes.extend([0x49, 0x83, 0xe3, 0x7f]); // and r11, 0x7f
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x74, 0x12]); // je last
    bytes.extend([0x49, 0x81, 0xcb, 0x80, 0x00, 0x00, 0x00]); // or r11, 0x80
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0xeb, 0xde]); // jmp loop
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
}

fn build_append_wire_scalar_slice(
    source_offset: usize,
    element_byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let capacity = wire_encode_capacity_imm32(out_length)?;
    let mut bytes = Vec::new();
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;

    // Descriptor and stable loop inputs: r9 = original ptr, r8 = walking
    // ptr, rdi = original count, rcx = remaining count, rdx = exact body
    // byte count.
    append_mov_reg_imm64(&mut bytes, Reg64::R11, 0);
    bytes.extend([0x4d, 0x8b, 0x8b]); // mov r9, [r11+ptr]
    bytes.extend(disp32(source_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x8b, 0x8b]); // mov rcx, [r11+len]
    bytes.extend(disp32(source_offset + 8)?.to_le_bytes());
    bytes.extend([0x4d, 0x89, 0xc8]); // mov r8, r9
    bytes.extend([0x48, 0x89, 0xcf]); // mov rdi, rcx
    bytes.extend([0x31, 0xd2]); // xor edx, edx

    // Every scalar needs at least one output byte. A count larger than the
    // whole output capacity violates both the work and capacity precondition;
    // fail closed without walking an unbounded descriptor.
    bytes.extend([0x48, 0x81, 0xf9]); // cmp rcx, imm32
    bytes.extend(capacity.to_le_bytes());
    let count_over_capacity = append_wire_rel32_branch(&mut bytes, &[0x0f, 0x87]); // ja done

    let measure_outer = bytes.len();
    bytes.extend([0x48, 0x85, 0xc9]); // test rcx, rcx
    let measure_done_fixup = append_wire_rel32_branch(&mut bytes, &[0x0f, 0x84]); // jz
    append_wire_slice_scalar_load(&mut bytes, element_byte_size, zigzag)?;
    let measure_varint = bytes.len();
    bytes.extend([0x48, 0xff, 0xc2]); // inc rdx
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    let measure_more = append_wire_rel32_branch(&mut bytes, &[0x0f, 0x85]); // jnz
    patch_wire_rel32(&mut bytes, measure_more, measure_varint);
    bytes.extend([0x48, 0xff, 0xc9]); // dec rcx
    let measure_next = append_wire_rel32_branch(&mut bytes, &[0xe9]);
    patch_wire_rel32(&mut bytes, measure_next, measure_outer);

    let measure_done = bytes.len();
    patch_wire_rel32(&mut bytes, measure_done_fixup, measure_done);

    // rsi = canonical byte width of the packed-body length.
    bytes.extend([0x48, 0x89, 0xd0]); // mov rax, rdx
    bytes.extend([0x31, 0xf6]); // xor esi, esi
    let prefix_count = bytes.len();
    bytes.extend([0x48, 0xff, 0xc6]); // inc rsi
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    let prefix_more = append_wire_rel32_branch(&mut bytes, &[0x0f, 0x85]); // jnz
    patch_wire_rel32(&mut bytes, prefix_more, prefix_count);

    // Require cursor + exact prefix + exact body <= output capacity before
    // mutating the payload region.
    bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
    bytes.extend([0x48, 0x01, 0xd0]); // add rax, rdx
    bytes.extend([0x48, 0x01, 0xf0]); // add rax, rsi
    bytes.extend([0x48, 0x3d]); // cmp rax, imm32
    bytes.extend(capacity.to_le_bytes());
    let insufficient = append_wire_rel32_branch(&mut bytes, &[0x0f, 0x87]); // ja done

    // Emit the length, then walk the descriptor again and emit each scalar.
    bytes.extend([0x48, 0x89, 0xd0]); // mov rax, rdx
    append_wire_varint_emit_loop(&mut bytes);
    bytes.extend([0x49, 0xff, 0xc7]); // final emit did not advance r15
    bytes.extend([0x4d, 0x89, 0xc8]); // mov r8, r9
    bytes.extend([0x48, 0x89, 0xf9]); // mov rcx, rdi

    let emit_outer = bytes.len();
    bytes.extend([0x48, 0x85, 0xc9]); // test rcx, rcx
    let emit_done_fixup = append_wire_rel32_branch(&mut bytes, &[0x0f, 0x84]); // jz
    append_wire_slice_scalar_load(&mut bytes, element_byte_size, zigzag)?;
    append_wire_varint_emit_loop(&mut bytes);
    bytes.extend([0x49, 0xff, 0xc7]); // re-sync dest after final byte
    bytes.extend([0x48, 0xff, 0xc9]); // dec rcx
    let emit_next = append_wire_rel32_branch(&mut bytes, &[0xe9]);
    patch_wire_rel32(&mut bytes, emit_next, emit_outer);

    let done = bytes.len();
    patch_wire_rel32(&mut bytes, count_over_capacity, done);
    patch_wire_rel32(&mut bytes, insufficient, done);
    patch_wire_rel32(&mut bytes, emit_done_fixup, done);
    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
    Ok(bytes)
}

pub fn append_wire_scalar_slice_width(
    source_offset: usize,
    element_byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> usize {
    build_append_wire_scalar_slice(
        source_offset,
        element_byte_size,
        zigzag,
        out_offset,
        out_length,
        written_offset,
    )
    .expect("validated x86_64 wire scalar-slice shape")
    .len()
}

pub fn append_wire_scalar_slice_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn append_wire_scalar_slice_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn encode_append_wire_scalar_slice(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    element_byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let _ = source_region;
    let bytes = build_append_wire_scalar_slice(
        source_offset,
        element_byte_size,
        zigzag,
        out_offset,
        out_length,
        written_offset,
    )?;
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

/// Byte offset of the WRITTEN page mov inside both wire appends (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_append_written_page_offset(_out_offset: usize) -> usize {
    17
}

/// Byte offset of the SOURCE page mov inside the varint append AND the
/// text-bytes append (both materialize the source page right after the shared
/// prologue).
pub fn wire_append_varint_source_page_offset(_out_offset: usize, _written_offset: usize) -> usize {
    37
}

// ---- compact_binary v0 wire-decode reads (chapter 20, wire stage 2b) ----
//
// Both operations share the encoder's cursor convention: the caller's `read`
// slot holds the running byte count, so every read loads it, reads through a
// moving pointer (`buffer base + buffer offset + cursor`), and writes the
// advanced cursor back. The success flag in the caller's `ok` slot is STICKY:
// each operation ANDs its own success bit into the slot and never sets it, so
// the first failure makes the whole decode report failure while later
// operations keep executing (every byte read stays bounds-checked against the
// buffer's compile-time length, so a failed decode never reads out of
// bounds). Register use: r15 = moving buffer pointer, r14 = read page,
// r13 = ok page, r10 = cursor, rax = value, rcx = shift, r11 = byte scratch,
// r9 = this-op success, r8 = 7-bit chunk / target page (r12 is the
// dispatch-state register and stays untouched).
//
// THE WIDTHS INVARIANT: every emitted byte must move the `_width` functions
// and the `wire_decode_*_offset` relocation offsets below in exact lockstep,
// or relocations drift and the binary segfaults.

/// Shared prologue: `mov r15, imm64(buffer)` (10, relocated at the
/// instruction start) + `add r15, imm32(buffer_offset)` (7) +
/// `mov r14, imm64(read)` (10, relocated at +17) +
/// `mov r10, [r14+read_offset]` (7) + `add r15, r10` (3) +
/// `mov r13, imm64(ok)` (10, relocated at +37).
fn wire_decode_prologue_width() -> usize {
    47
}

fn append_wire_decode_prologue(
    bytes: &mut Vec<u8>,
    buffer_offset: usize,
    read_offset: usize,
) -> Result<(), Diagnostic> {
    append_mov_r15_imm64(bytes, 0);
    append_add_r15_imm32(bytes, buffer_offset)?;
    append_mov_r14_imm64(bytes, 0);
    append_load_r10_from_r14(bytes, read_offset)?;
    bytes.extend([0x4d, 0x01, 0xd7]); // add r15, r10
    bytes.extend([0x49, 0xbd]); // mov r13, imm64(ok page)
    bytes.extend(0u64.to_le_bytes());
    Ok(())
}

/// Shared epilogue: AND this operation's success bit (r9) into the sticky ok
/// slot, then store the advanced cursor back to the read slot.
/// `movzx r11d, byte [r13+ok]` (8) + `and r11, r9` (3) +
/// `mov [r13+ok], r11b` (7) + cursor store (7).
fn wire_decode_tail_width() -> usize {
    25
}

fn append_wire_decode_epilogue(
    bytes: &mut Vec<u8>,
    read_offset: usize,
    ok_offset: usize,
) -> Result<(), Diagnostic> {
    let ok_displacement = disp32(ok_offset)?;
    bytes.extend([0x45, 0x0f, 0xb6, 0x9d]); // movzx r11d, byte [r13+disp32]
    bytes.extend(ok_displacement.to_le_bytes());
    bytes.extend([0x4d, 0x21, 0xcb]); // and r11, r9
    bytes.extend([0x45, 0x88, 0x9d]); // mov [r13+disp32], r11b
    bytes.extend(ok_displacement.to_le_bytes());
    append_store_r10_to_r14(bytes, read_offset, 8)
}

/// The compile-time buffer length as a `cmp r10, imm32` operand.
fn wire_decode_length_imm32(buffer_length: usize) -> Result<i32, Diagnostic> {
    i32::try_from(buffer_length).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 wire decoder cannot bounds-check a {buffer_length}-byte buffer yet"
        ))
    })
}

pub fn read_wire_expected_byte_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
) -> usize {
    // Prologue + the fixed check block (success-bit mov + bounds cmp/jae +
    // byte load + cursor inc + expected cmp/je + fail xor) + epilogue.
    wire_decode_prologue_width() + 34 + wire_decode_tail_width()
}

pub fn read_wire_expected_byte_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R13,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn read_wire_expected_byte_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
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

    // Fixed 34-byte check block:
    //         mov  r9d, 1
    //         cmp  r10, imm32(length)
    //         jae  fail            (+16: skip movzx/inc/cmp/je)
    //         movzx r11d, byte [r15]
    //         inc  r10
    //         cmp  r11, imm32(expected)
    //         je   done            (+3: skip the fail xor)
    //   fail: xor  r9d, r9d
    //   done:
    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x10]); // jae +16 -> fail
    bytes.extend([0x45, 0x0f, 0xb6, 0x1f]); // movzx r11d, byte [r15]
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0x49, 0x81, 0xfb]); // cmp r11, imm32
    bytes.extend(i32::from(expected).to_le_bytes());
    bytes.extend([0x74, 0x03]); // je +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_expected_byte_width(buffer_offset, buffer_length, read_offset, ok_offset)
    );
    Ok(bytes)
}

/// The fixed canonical LEB128 read loop + fail tail (see the decoder body).
fn wire_varint_read_loop_width() -> usize {
    81
}

fn append_wire_varint_read_loop(
    bytes: &mut Vec<u8>,
    buffer_length: usize,
) -> Result<(), Diagnostic> {
    let start = bytes.len();
    // Canonical unsigned LEB128:
    // - at most ten groups;
    // - a multi-group value's terminal payload is nonzero;
    // - the tenth payload is at most one (only bit 63 remains).
    //
    // rcx is the NEXT shift after each group, so terminal rcx == 7 means the
    // one-group case and rcx == 70 means the tenth group.
    bytes.extend([0x48, 0x83, 0xf9, 0x3f]); // cmp rcx, 63
    bytes.extend([0x77, 0x48]); // ja +72 -> fail
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x3f]); // jae +63 -> fail
    bytes.extend([0x45, 0x0f, 0xb6, 0x1f]); // movzx r11d, byte [r15]
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0x4d, 0x89, 0xd8]); // mov r8, r11
    bytes.extend([0x49, 0x83, 0xe0, 0x7f]); // and r8, 0x7f
    bytes.extend([0x49, 0xd3, 0xe0]); // shl r8, cl
    bytes.extend([0x4c, 0x09, 0xc0]); // or rax, r8
    bytes.extend([0x48, 0x83, 0xc1, 0x07]); // add rcx, 7
    bytes.extend([0x49, 0xf7, 0xc3, 0x80, 0x00, 0x00, 0x00]); // test r11, 0x80
    bytes.extend([0x75, 0xcd]); // jnz -51 -> loop
    bytes.extend([0x48, 0x83, 0xf9, 0x07]); // cmp rcx, 7
    bytes.extend([0x74, 0x18]); // je +24 -> done (one group)
    bytes.extend([0x49, 0xf7, 0xc3, 0x7f, 0x00, 0x00, 0x00]); // test r11,0x7f
    bytes.extend([0x74, 0x0c]); // jz +12 -> fail (non-minimal)
    bytes.extend([0x48, 0x83, 0xf9, 0x46]); // cmp rcx, 70
    bytes.extend([0x75, 0x09]); // jne +9 -> done
    bytes.extend([0x49, 0x83, 0xfb, 0x01]); // cmp r11, 1
    bytes.extend([0x76, 0x03]); // jbe +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // fail: xor r9d, r9d
    debug_assert_eq!(bytes.len() - start, wire_varint_read_loop_width());
    Ok(())
}

/// `mov r11, rax` + `and r11, 1` + `neg r11` + `shr rax, 1` + `xor rax, r11`.
fn wire_unzigzag_width() -> usize {
    16
}

pub fn read_wire_scalar_varint_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _target_offset: usize,
    _byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    // Prologue + success/value/shift init (10) + read loop + optional
    // unzigzag + target imm64 (10) + truncating store (7) + epilogue.
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 10
        + if range.is_some() { 35 } else { 0 }
        + 7
        + wire_decode_tail_width()
}

pub fn read_wire_scalar_varint_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R13,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn read_wire_scalar_varint_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// LEB128-read a runtime scalar at the cursor into the target place. The
/// loop's iteration count is data dependent but its EMITTED width is constant
/// (the widths invariant): truncation and overlong varints (a continuation
/// past shift 63) branch to the fail arm. Signed targets un-zigzag
/// (`(n >> 1) ^ -(n & 1)`) before the store; the store truncates to the field
/// width.
#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_scalar_varint(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 wire decoder cannot varint-decode {byte_size}-byte scalars yet"
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

    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x31, 0xc0]); // xor eax, eax (value)
    bytes.extend([0x31, 0xc9]); // xor ecx, ecx (shift)

    // Canonical LEB128 read loop (fixed width; see the shared emitter).
    let loop_start = bytes.len();
    append_wire_varint_read_loop(&mut bytes, buffer_length)?;
    debug_assert_eq!(bytes.len() - loop_start, wire_varint_read_loop_width());

    if zigzag {
        // unzigzag(n) = (n >> 1) ^ -(n & 1); r11 holds the mask.
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0x83, 0xe3, 0x01]); // and r11, 1
        bytes.extend([0x49, 0xf7, 0xdb]); // neg r11
        bytes.extend([0x48, 0xd1, 0xe8]); // shr rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }

    // r8 = the target base (imm64 relocated at
    // `wire_decode_varint_target_page_offset`), then the truncating store.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(target page)
    bytes.extend(0u64.to_le_bytes());
    let target_displacement = disp32(target_offset)?;
    if let Some(range) = range {
        // Establish the destination range BEFORE writing the constrained
        // place. Invalid hostile bytes clear this operation's sticky success
        // bit and branch over the store, preserving the prior valid value.
        //
        // mov r11,min; cmp rax,r11; j< fail
        bytes.extend([0x49, 0xbb]);
        bytes.extend((range.minimum as u64).to_le_bytes());
        bytes.extend([0x4c, 0x39, 0xd8]);
        bytes.extend([if range.signed { 0x7c } else { 0x72 }, 15]); // jl / jb
        // mov r11,max; cmp rax,r11; j<= store
        bytes.extend([0x49, 0xbb]);
        bytes.extend((range.maximum as u64).to_le_bytes());
        bytes.extend([0x4c, 0x39, 0xd8]);
        bytes.extend([
            if range.signed { 0x7e } else { 0x76 }, // jle / jbe
            5,
        ]);
        bytes.extend([0x45, 0x31, 0xc9]); // fail: xor r9d,r9d
        bytes.extend([0xeb, 0x07]); // skip the fixed-width target store
    }
    match byte_size {
        1 => bytes.extend([0x41, 0x88, 0x80]), // mov [r8+disp32], al
        4 => bytes.extend([0x41, 0x89, 0x80]), // mov [r8+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x80]), // mov [r8+disp32], rax
        _ => unreachable!("byte_size validated above"),
    }
    bytes.extend(target_displacement.to_le_bytes());

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

/// compact_binary v0 borrowed `&[u8]` decode (#43): read the byte-LENGTH varint
/// (the shared prologue + LEB128 loop leave r15 = &buffer[content start] and
/// rax = the length), bounds-check the content against the buffer, store the fat
/// `{ptr = r15, len = rax}` descriptor into the target, and advance the cursor
/// past the content. A content run past the buffer clears the sticky `ok`.
pub fn encode_read_wire_byte_slice(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    predicate_mask: u8,
) -> Result<Vec<u8>, Diagnostic> {
    // The region only picks the relocation symbol; the shape is identical.
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

    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1 (ok)
    bytes.extend([0x31, 0xc0]); // xor eax, eax (length)
    bytes.extend([0x31, 0xc9]); // xor ecx, ecx (shift)

    // Shared canonical LEB128 read loop: rax = length, r15 points at the
    // CONTENT (just past the length varint), and r10 is the cursor.
    append_wire_varint_read_loop(&mut bytes, buffer_length)?;

    // Bounds + advance (fixed 21 bytes): end = cursor + len; if end >
    // buffer_length clear ok; cursor = end.
    bytes.extend([0x4d, 0x89, 0xd0]); // mov r8, r10  (r8 = cursor)
    bytes.extend([0x49, 0x01, 0xc0]); // add r8, rax  (r8 = cursor + len = end)
    bytes.extend([0x49, 0x81, 0xf8]); // cmp r8, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x76, 0x03]); // jbe +3 (skip clear when end <= length)
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (content overruns -> clear ok)
    bytes.extend([0x4d, 0x89, 0xc2]); // mov r10, r8 (advance cursor to end)

    // Decode-boundary byte-domain validation over the just-decoded content
    // (ptr r15, len rax): every predicate in the mask checks the UNTRUSTED
    // bytes and clears the sticky ok flag (r9d) on violation -- the aarch64
    // twin's contract exactly.
    append_wire_byte_predicate_checks(&mut bytes, predicate_mask);

    // Store the descriptor: ptr = r15 (content start) @ +0, len = rax @ +8.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(target page)
    bytes.extend(0u64.to_le_bytes());
    bytes.extend([0x4d, 0x89, 0xb8]); // mov [r8+disp32], r15
    bytes.extend(disp32(target_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x89, 0x80]); // mov [r8+disp32], rax
    bytes.extend(disp32(target_offset + 8)?.to_le_bytes());

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_byte_slice_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            predicate_mask
        )
    );
    Ok(bytes)
}

/// Decode-boundary byte-domain validation blocks (one per predicate in the
/// mask, `ByteSequencePredicate::ALL` order -- the aarch64 twin's contract):
/// content ptr in r15, length in rax, sticky ok flag in r9d; rcx (walking
/// pointer), r11 (end bound), and r8 (byte scratch) are spent at this point
/// in the byte-slice sequence -- the target page claims r8 only AFTER these
/// checks. Widths via `wire_byte_predicate_checks_width` (which measures
/// this emitter -- it is pure, and a hand-summed constant for the ~90-entry
/// utf8 block would be pure drift risk).
pub(super) fn append_wire_byte_predicate_checks(bytes: &mut Vec<u8>, predicate_mask: u8) {
    use psi_language_semantics::byte_predicates::ByteSequencePredicate;
    for predicate in ByteSequencePredicate::in_mask(predicate_mask) {
        match predicate {
            ByteSequencePredicate::NonEmpty => {
                bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
                bytes.extend([0x75, 0x03]); // jnz +3 (nonzero length: ok)
                bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
            }
            ByteSequencePredicate::NoNul => {
                bytes.extend([0x4c, 0x89, 0xf9]); // mov rcx, r15 (p)
                bytes.extend([0x4d, 0x89, 0xfb]); // mov r11, r15
                bytes.extend([0x49, 0x01, 0xc3]); // add r11, rax (end)
                bytes.extend([0x4c, 0x39, 0xd9]); // loop: cmp rcx, r11
                bytes.extend([0x73, 0x0f]); // jae done (+15)
                bytes.extend([0x44, 0x0f, 0xb6, 0x01]); // movzx r8d, byte [rcx]
                bytes.extend([0x48, 0xff, 0xc1]); // inc rcx
                bytes.extend([0x45, 0x85, 0xc0]); // test r8d, r8d
                bytes.extend([0x75, 0xef]); // jnz loop (-17)
                bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (a NUL byte)
            }
            ByteSequencePredicate::AsciiOnly => {
                bytes.extend([0x4c, 0x89, 0xf9]); // mov rcx, r15
                bytes.extend([0x4d, 0x89, 0xfb]); // mov r11, r15
                bytes.extend([0x49, 0x01, 0xc3]); // add r11, rax
                bytes.extend([0x4c, 0x39, 0xd9]); // loop: cmp rcx, r11
                bytes.extend([0x73, 0x10]); // jae done (+16)
                bytes.extend([0x44, 0x0f, 0xb6, 0x01]); // movzx r8d, byte [rcx]
                bytes.extend([0x48, 0xff, 0xc1]); // inc rcx
                bytes.extend([0x41, 0xf6, 0xc0, 0x80]); // test r8b, 0x80
                bytes.extend([0x74, 0xee]); // jz loop (-18)
                bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (high bit set)
            }
            ByteSequencePredicate::ValidUtf8 => {
                append_wire_utf8_validation(bytes);
            }
        }
    }
}

/// UTF-8 validation over [r15, r15+rax): the aarch64 twin's decoded-scalar
/// walk in x86 idiom, dispatching on the LEAD before loading continuations
/// so ONE scratch register (r8) serves both roles. Lead classes: ASCII;
/// C2..DF one continuation; E0/ED/E1..EC,EE..EF two with E0 requiring
/// cont1 >= A0 (overlongs) and ED requiring cont1 < A0 (surrogates);
/// F0/F1..F3/F4 three with F0 requiring cont1 >= 90 and F4 requiring
/// cont1 < 90 (beyond U+10FFFF); 0x80..0xC1 and 0xF5.. invalid. Assembled
/// with a local two-pass label resolver; ALL label branches are rel32 for
/// uniform, safe distances.
fn append_wire_utf8_validation(bytes: &mut Vec<u8>) {
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum Label {
        Loop,
        Two,
        E0Block,
        EdBlock,
        ThreePlain,
        OneMore,
        F0Block,
        F4Block,
        FourPlain,
        TwoMore,
        Fail,
        Done,
    }
    enum Ins {
        Fixed(&'static [u8]),
        /// jcc rel32: (0x0f, opcode) pair.
        Jcc(u8, Label),
        Jmp(Label),
    }
    use Ins::*;
    use Label::*;
    const JB: u8 = 0x82; // unsigned <
    const JAE: u8 = 0x83; // unsigned >=
    const JNE: u8 = 0x85;

    // One continuation read with an UNSIGNED range check [low, high):
    // bounds, load into r8d, range compare. cmp r8d, imm32 (41 81 f8 + 4).
    fn continuation(
        program: &mut Vec<(Option<Label>, Ins)>,
        at: Option<Label>,
        low: u32,
        high: u32,
    ) {
        let cmp_imm = |value: u32| -> &'static [u8] {
            // Leaked tiny allocations keep Ins::Fixed 'static; bounded by the
            // fixed set of (low, high) pairs this validator uses.
            Box::leak(
                [0x41, 0x81, 0xf8]
                    .iter()
                    .copied()
                    .chain(value.to_le_bytes())
                    .collect::<Vec<u8>>()
                    .into_boxed_slice(),
            )
        };
        program.push((at, Fixed(&[0x4c, 0x39, 0xd9]))); // cmp rcx, r11
        program.push((None, Jcc(JAE, Fail))); // truncated
        program.push((None, Fixed(&[0x44, 0x0f, 0xb6, 0x01]))); // movzx r8d, [rcx]
        program.push((None, Fixed(&[0x48, 0xff, 0xc1]))); // inc rcx
        program.push((None, Fixed(cmp_imm(low))));
        program.push((None, Jcc(JB, Fail)));
        program.push((None, Fixed(cmp_imm(high))));
        program.push((None, Jcc(JAE, Fail)));
    }
    fn lead_cmp(value: u32) -> &'static [u8] {
        Box::leak(
            [0x41, 0x81, 0xf8]
                .iter()
                .copied()
                .chain(value.to_le_bytes())
                .collect::<Vec<u8>>()
                .into_boxed_slice(),
        )
    }

    let mut program: Vec<(Option<Label>, Ins)> = Vec::new();
    program.push((None, Fixed(&[0x4c, 0x89, 0xf9]))); // mov rcx, r15 (p)
    program.push((None, Fixed(&[0x4d, 0x89, 0xfb]))); // mov r11, r15
    program.push((None, Fixed(&[0x49, 0x01, 0xc3]))); // add r11, rax (end)
    program.push((Some(Loop), Fixed(&[0x4c, 0x39, 0xd9]))); // cmp rcx, r11
    program.push((None, Jcc(JAE, Done)));
    program.push((None, Fixed(&[0x44, 0x0f, 0xb6, 0x01]))); // movzx r8d, [rcx] (lead)
    program.push((None, Fixed(&[0x48, 0xff, 0xc1]))); // inc rcx
    program.push((None, Fixed(lead_cmp(0x80))));
    program.push((None, Jcc(JB, Loop))); // ASCII
    program.push((None, Fixed(lead_cmp(0xC2))));
    program.push((None, Jcc(JB, Fail))); // invalid lead 0x80..0xC1
    program.push((None, Fixed(lead_cmp(0xE0))));
    program.push((None, Jcc(JB, Two))); // C2..DF
    program.push((None, Jcc(JNE, Fail))); // placeholder replaced below
    program.pop(); // (structured dispatch below instead)
    // Dispatch E0 / ED / other-threes / F0 / F4 / other-fours / >= F5.
    program.push((None, Fixed(lead_cmp(0xE0 + 1)))); // cmp 0xE1
    program.push((None, Jcc(JB, E0Block))); // exactly 0xE0
    program.push((None, Fixed(lead_cmp(0xED))));
    program.push((None, Jcc(JB, ThreePlain))); // E1..EC
    program.push((None, Fixed(lead_cmp(0xED + 1)))); // cmp 0xEE
    program.push((None, Jcc(JB, EdBlock))); // exactly 0xED
    program.push((None, Fixed(lead_cmp(0xF0))));
    program.push((None, Jcc(JB, ThreePlain))); // EE..EF
    program.push((None, Fixed(lead_cmp(0xF0 + 1)))); // cmp 0xF1
    program.push((None, Jcc(JB, F0Block))); // exactly 0xF0
    program.push((None, Fixed(lead_cmp(0xF4))));
    program.push((None, Jcc(JB, FourPlain))); // F1..F3
    program.push((None, Fixed(lead_cmp(0xF4 + 1)))); // cmp 0xF5
    program.push((None, Jcc(JAE, Fail))); // F5..
    // exactly 0xF4 falls through:
    continuation(&mut program, Some(F4Block), 0x80, 0x90);
    program.push((None, Jmp(TwoMore)));
    continuation(&mut program, Some(F0Block), 0x90, 0xC0);
    program.push((None, Jmp(TwoMore)));
    continuation(&mut program, Some(FourPlain), 0x80, 0xC0);
    program.push((Some(TwoMore), Fixed(&[]))); // label carrier
    continuation(&mut program, None, 0x80, 0xC0);
    continuation(&mut program, None, 0x80, 0xC0);
    program.push((None, Jmp(Loop)));
    continuation(&mut program, Some(E0Block), 0xA0, 0xC0);
    program.push((None, Jmp(OneMore)));
    continuation(&mut program, Some(EdBlock), 0x80, 0xA0);
    program.push((None, Jmp(OneMore)));
    continuation(&mut program, Some(ThreePlain), 0x80, 0xC0);
    program.push((Some(OneMore), Fixed(&[]))); // label carrier
    continuation(&mut program, None, 0x80, 0xC0);
    program.push((None, Jmp(Loop)));
    continuation(&mut program, Some(Two), 0x80, 0xC0);
    program.push((None, Jmp(Loop)));
    program.push((Some(Fail), Fixed(&[0x45, 0x31, 0xc9]))); // xor r9d, r9d
    // Done = first instruction after the block.

    // Pass 1: byte positions (Jcc = 6 bytes, Jmp = 5, Fixed = len).
    let width_of = |instruction: &Ins| -> usize {
        match instruction {
            Fixed(word) => word.len(),
            Jcc(..) => 6,
            Jmp(..) => 5,
        }
    };
    let mut positions = std::collections::HashMap::new();
    let mut at = 0usize;
    for (label, instruction) in &program {
        if let Some(label) = label {
            positions.insert(*label, at);
        }
        at += width_of(instruction);
    }
    positions.insert(Done, at);
    // Pass 2: emit with resolved rel32 offsets (relative to the next
    // instruction's start).
    let mut at = 0usize;
    for (_, instruction) in &program {
        let end = at + width_of(instruction);
        match instruction {
            Fixed(word) => bytes.extend(*word),
            Jcc(opcode, target) => {
                bytes.extend([0x0f, *opcode]);
                bytes.extend(((positions[target] as i64 - end as i64) as i32).to_le_bytes());
            }
            Jmp(target) => {
                bytes.push(0xe9);
                bytes.extend(((positions[target] as i64 - end as i64) as i32).to_le_bytes());
            }
        }
        at = end;
    }
}

/// Bytes of [`append_wire_byte_predicate_checks`]: measured from the pure
/// emitter itself -- ONE source of truth (a hand-summed constant for the
/// ~90-entry utf8 block would be pure drift risk).
pub fn wire_byte_predicate_checks_width(predicate_mask: u8) -> usize {
    let mut scratch = Vec::new();
    append_wire_byte_predicate_checks(&mut scratch, predicate_mask);
    scratch.len()
}

pub fn read_wire_byte_slice_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _target_offset: usize,
    predicate_mask: u8,
) -> usize {
    // Prologue + success/value/shift init (10) + read loop + bounds&advance
    // (21) + the byte-predicate validation blocks + target imm64 (10) + ptr
    // store (7) + len store (7) + epilogue.
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + 21
        + wire_byte_predicate_checks_width(predicate_mask)
        + 10
        + 7
        + 7
        + wire_decode_tail_width()
}

pub fn read_wire_byte_slice_clobbers() -> RegisterSet {
    read_wire_scalar_varint_clobbers()
}

pub fn read_wire_byte_slice_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// Byte offset of the TARGET page mov inside the byte-slice decode (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_decode_byte_slice_target_page_offset(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    predicate_mask: u8,
) -> usize {
    // The validation blocks precede the target page mov.
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + 21
        + wire_byte_predicate_checks_width(predicate_mask)
}

/// Byte offset of the READ (cursor) page mov inside both wire decodes (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_decode_read_page_offset(_buffer_offset: usize) -> usize {
    17
}

/// Byte offset of the OK (sticky flag) page mov inside both wire decodes.
pub fn wire_decode_ok_page_offset(_buffer_offset: usize, _read_offset: usize) -> usize {
    37
}

/// Byte offset of the TARGET page mov inside the varint decode.
pub fn wire_decode_varint_target_page_offset(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    zigzag: bool,
) -> usize {
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
}

pub fn read_wire_nested_open_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _end_offset: usize,
) -> usize {
    // Prologue + end page mov (10) + length load (7) + success mov (6) +
    // length cmp/jbe/fail xor (11) + end add (3) + bound cmp/jbe/fail xor
    // (11) + end store (7) + epilogue.
    wire_decode_prologue_width() + 55 + wire_decode_tail_width()
}

pub fn read_wire_nested_open_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R13,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn read_wire_nested_open_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
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

    // r8 = the end-slot page (imm64 relocated at
    // `wire_decode_nested_end_page_offset`), rax = the LENGTH stored there.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(end page)
    bytes.extend(0u64.to_le_bytes());
    let end_displacement = disp32(end_offset)?;
    bytes.extend([0x49, 0x8b, 0x80]); // mov rax, [r8+disp32]
    bytes.extend(end_displacement.to_le_bytes());

    // ok &= length <= buffer length (a raw length past the buffer could wrap
    // the 64-bit end sum back inside the bound -- reject it before adding);
    // then end = cursor + length and ok &= end <= buffer length. The cursor
    // never exceeds the buffer length and the length just passed its own
    // check, so the sum cannot wrap.
    //          mov  r9d, 1
    //          cmp  rax, imm32(length)
    //          jbe  len_ok          (+3: skip the fail xor)
    //   fail1: xor  r9d, r9d
    //  len_ok: add  rax, r10
    //          cmp  rax, imm32(length)
    //          jbe  done            (+3: bound fits -- skip the fail xor)
    //   fail2: xor  r9d, r9d
    //   done:
    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x48, 0x3d]); // cmp rax, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x76, 0x03]); // jbe +3 -> len_ok
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
    bytes.extend([0x4c, 0x01, 0xd0]); // add rax, r10
    bytes.extend([0x48, 0x3d]); // cmp rax, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x76, 0x03]); // jbe +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

    bytes.extend([0x49, 0x89, 0x80]); // mov [r8+disp32], rax
    bytes.extend(end_displacement.to_le_bytes());

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

pub fn read_wire_nested_close_width(
    _buffer_offset: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _end_offset: usize,
) -> usize {
    // Prologue + end page mov (10) + end load (7) + success mov (6) +
    // cursor cmp (3) + je (2) + fail xor (3) + epilogue.
    wire_decode_prologue_width() + 31 + wire_decode_tail_width()
}

pub fn read_wire_nested_close_clobbers() -> RegisterSet {
    read_wire_nested_open_clobbers()
}

pub fn read_wire_nested_close_additional_machine_state() -> MachineStateSet {
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

    // r8 = the end-slot page, rax = the end bound stored there.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(end page)
    bytes.extend(0u64.to_le_bytes());
    let end_displacement = disp32(end_offset)?;
    bytes.extend([0x49, 0x8b, 0x80]); // mov rax, [r8+disp32]
    bytes.extend(end_displacement.to_le_bytes());

    // ok &= cursor == end:
    //         mov  r9d, 1
    //         cmp  r10, rax
    //         je   done            (+3: skip the fail xor)
    //   fail: xor  r9d, r9d
    //   done:
    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x49, 0x39, 0xc2]); // cmp r10, rax
    bytes.extend([0x74, 0x03]); // je +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_nested_close_width(buffer_offset, read_offset, ok_offset, end_offset)
    );
    Ok(bytes)
}

/// Byte offset of the END-slot page mov inside both nested decodes
/// (materialized right after the shared prologue). The repeated-element read
/// materializes its end page at the same position.
pub fn wire_decode_nested_end_page_offset(_buffer_offset: usize, _read_offset: usize) -> usize {
    wire_decode_prologue_width()
}

// ---- compact_binary v0 wire REPEATED fields (chapter 20) ----
//
// A repeated field packs LENGTH-delimited (tag + byte-length varint +
// back-to-back element varints). The element count is runtime-sized but
// bounded by the schema's declared maximum, so selection UNROLLS the maximum
// and each unrolled operation guards itself: the encode-side append runs only
// when its compile-time element index is below the FixedVec `length` slot's
// value; the decode-side read runs only while the cursor sits below the end
// bound the surrounding nested OPEN stored. Guarding keeps every emitted
// width compile-time-fixed (the widths invariant) while the wire bytes track
// the live count.

/// Guard block of the repeated scalar append: count page mov (10, relocated)
/// + count load (7) + index cmp (7) + jbe skip (2).
fn wire_repeated_append_guard_width() -> usize {
    26
}

pub fn append_wire_repeated_scalar_varint_width(
    _source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    _index: u64,
    _count_offset: usize,
    _out_offset: usize,
    _written_offset: usize,
) -> usize {
    // Prologue + guard + source imm64 (10) + sized load + optional zigzag +
    // emit loop + cursor store (7).
    wire_append_prologue_width()
        + wire_repeated_append_guard_width()
        + 10
        + wire_varint_source_load_width(byte_size)
        + if zigzag { wire_zigzag_width() } else { 0 }
        + wire_varint_emit_loop_width()
        + 7
}

pub fn append_wire_repeated_scalar_varint_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
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
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    index: u64,
    count_region: omega_target_operations::RuntimeStorageRegion,
    count_offset: usize,
    out_offset: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 wire encoder cannot varint-encode {byte_size}-byte scalars yet"
        )));
    }
    let index_imm = i32::try_from(index).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 wire encoder cannot guard repeated element index {index} yet"
        ))
    })?;
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

    // Guard: r9 = count (from the relocated count page); skip the whole
    // append when count <= index (unsigned). The skip lands past the cursor
    // store, so a skipped element changes nothing.
    let skip_distance = 10
        + wire_varint_source_load_width(byte_size)
        + if zigzag { wire_zigzag_width() } else { 0 }
        + wire_varint_emit_loop_width()
        + 7;
    let skip_rel8 =
        i8::try_from(skip_distance).expect("the guarded append body is well under the rel8 range");
    bytes.extend([0x49, 0xb9]); // mov r9, imm64(count page)
    bytes.extend(0u64.to_le_bytes());
    bytes.extend([0x4d, 0x8b, 0x89]); // mov r9, [r9+disp32]
    bytes.extend(disp32(count_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x81, 0xf9]); // cmp r9, imm32(index)
    bytes.extend(index_imm.to_le_bytes());
    bytes.extend([0x76, skip_rel8 as u8]); // jbe skip (count <= index)

    // The unguarded scalar-varint body (see `encode_append_wire_scalar_varint`):
    // r11 = source page (imm64 relocated), rax = the scalar.
    append_mov_reg_imm64(&mut bytes, Reg64::R11, 0);
    let displacement = disp32(source_offset)?;
    match (byte_size, zigzag) {
        (8, _) => bytes.extend([0x49, 0x8b, 0x83]), // mov rax, [r11+disp32]
        (4, false) => bytes.extend([0x41, 0x8b, 0x83]), // mov eax, [r11+disp32]
        (4, true) => bytes.extend([0x49, 0x63, 0x83]), // movsxd rax, dword [r11+disp32]
        (1, _) => bytes.extend([0x41, 0x0f, 0xb6, 0x83]), // movzx eax, byte [r11+disp32]
        _ => unreachable!("byte_size validated above"),
    }
    bytes.extend(displacement.to_le_bytes());

    if zigzag {
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
        bytes.extend([0x48, 0xc1, 0xe0, 0x01]); // shl rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }

    // The same fixed 40-byte LEB128 emit loop as the scalar varint.
    bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
    bytes.extend([0x49, 0x83, 0xe3, 0x7f]); // and r11, 0x7f
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x74, 0x12]); // je +18 -> last
    bytes.extend([0x49, 0x81, 0xcb, 0x80, 0x00, 0x00, 0x00]); // or r11, 0x80
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0xeb, 0xde]); // jmp -34 -> loop
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10

    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
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

/// Byte offset of the COUNT page mov inside the repeated append (right after
/// the shared prologue).
pub fn wire_append_repeated_count_page_offset(_out_offset: usize, _written_offset: usize) -> usize {
    wire_append_prologue_width()
}

/// Byte offset of the SOURCE page mov inside the repeated append (after the
/// guard block).
pub fn wire_append_repeated_source_page_offset(
    _out_offset: usize,
    _written_offset: usize,
    _count_offset: usize,
    _index: u64,
) -> usize {
    wire_append_prologue_width() + wire_repeated_append_guard_width()
}

/// Guard block of the repeated scalar read: end page mov (10, relocated) +
/// end load (7) + cursor cmp (3) + jae rel32 skip (6).
fn wire_repeated_read_guard_width() -> usize {
    26
}

/// Count bump of the repeated scalar read: count page mov (10, relocated) +
/// count load (7) + inc (3) + count store (7).
fn wire_repeated_read_count_bump_width() -> usize {
    27
}

#[allow(clippy::too_many_arguments)]
pub fn read_wire_repeated_scalar_varint_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _end_offset: usize,
    _count_offset: usize,
    _target_offset: usize,
    _byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    // Prologue + guard + success/value/shift init (10) + read loop + optional
    // unzigzag + target imm64 (10) + truncating store (7) + count bump +
    // epilogue.
    wire_decode_prologue_width()
        + wire_repeated_read_guard_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 10
        + if range.is_some() { 35 } else { 0 }
        + 7
        + wire_repeated_read_count_bump_width()
        + wire_decode_tail_width()
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
/// FixedVec `length` slot. A skipped read changes nothing -- the jump lands
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
    count_region: omega_target_operations::RuntimeStorageRegion,
    count_offset: usize,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 wire decoder cannot varint-decode {byte_size}-byte scalars yet"
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

    // Guard: r8 = the end-slot page (imm64 relocated at the nested end page
    // offset), rax = the absolute end bound stored there; skip everything
    // (including the epilogue) when cursor >= end.
    let skip_distance = 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 10
        + if range.is_some() { 35 } else { 0 }
        + 7
        + wire_repeated_read_count_bump_width()
        + wire_decode_tail_width();
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(end page)
    bytes.extend(0u64.to_le_bytes());
    let end_displacement = disp32(end_offset)?;
    bytes.extend([0x49, 0x8b, 0x80]); // mov rax, [r8+disp32]
    bytes.extend(end_displacement.to_le_bytes());
    bytes.extend([0x49, 0x39, 0xc2]); // cmp r10, rax
    bytes.extend([0x0f, 0x83]); // jae rel32 -> skip
    bytes.extend(
        i32::try_from(skip_distance)
            .expect("the guarded read body is well under the rel32 range")
            .to_le_bytes(),
    );

    // The unguarded scalar-varint body (see `encode_read_wire_scalar_varint`).
    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x31, 0xc0]); // xor eax, eax (value)
    bytes.extend([0x31, 0xc9]); // xor ecx, ecx (shift)

    let loop_start = bytes.len();
    append_wire_varint_read_loop(&mut bytes, buffer_length)?;
    debug_assert_eq!(bytes.len() - loop_start, wire_varint_read_loop_width());

    if zigzag {
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0x83, 0xe3, 0x01]); // and r11, 1
        bytes.extend([0x49, 0xf7, 0xdb]); // neg r11
        bytes.extend([0x48, 0xd1, 0xe8]); // shr rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }

    // r8 = the target page (imm64 relocated), then the truncating store.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(target page)
    bytes.extend(0u64.to_le_bytes());
    let target_displacement = disp32(target_offset)?;
    if let Some(range) = range {
        // Establish the element's declared interval before mutating that
        // array slot. A rejected element still consumes wire input and bumps
        // the decoded count, but preserves the prior valid element value.
        bytes.extend([0x49, 0xbb]); // mov r11, minimum
        bytes.extend((range.minimum as u64).to_le_bytes());
        bytes.extend([0x4c, 0x39, 0xd8]); // cmp rax, r11
        bytes.extend([if range.signed { 0x7c } else { 0x72 }, 15]); // jl / jb fail
        bytes.extend([0x49, 0xbb]); // mov r11, maximum
        bytes.extend((range.maximum as u64).to_le_bytes());
        bytes.extend([0x4c, 0x39, 0xd8]); // cmp rax, r11
        bytes.extend([if range.signed { 0x7e } else { 0x76 }, 5]); // jle / jbe store
        bytes.extend([0x45, 0x31, 0xc9]); // fail: xor r9d, r9d
        bytes.extend([0xeb, 0x07]); // skip fixed-width target store
    }
    match byte_size {
        1 => bytes.extend([0x41, 0x88, 0x80]), // mov [r8+disp32], al
        4 => bytes.extend([0x41, 0x89, 0x80]), // mov [r8+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x80]), // mov [r8+disp32], rax
        _ => unreachable!("byte_size validated above"),
    }
    bytes.extend(target_displacement.to_le_bytes());

    // Count bump: r11 = the count page (imm64 relocated), rcx = count + 1
    // (rcx is free -- the read loop's shift use ended above).
    bytes.extend([0x49, 0xbb]); // mov r11, imm64(count page)
    bytes.extend(0u64.to_le_bytes());
    let count_displacement = disp32(count_offset)?;
    bytes.extend([0x49, 0x8b, 0x8b]); // mov rcx, [r11+disp32]
    bytes.extend(count_displacement.to_le_bytes());
    bytes.extend([0x48, 0xff, 0xc1]); // inc rcx
    bytes.extend([0x49, 0x89, 0x8b]); // mov [r11+disp32], rcx
    bytes.extend(count_displacement.to_le_bytes());

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

/// Byte offset of the TARGET page mov inside the repeated read (after the
/// guard block and the read loop).
pub fn wire_decode_repeated_target_page_offset(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _end_offset: usize,
    zigzag: bool,
) -> usize {
    wire_decode_prologue_width()
        + wire_repeated_read_guard_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
}

/// Byte offset of the COUNT page mov inside the repeated read (after the
/// target store).
#[allow(clippy::too_many_arguments)]
pub fn wire_decode_repeated_count_page_offset(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    _target_offset: usize,
    _byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    wire_decode_repeated_target_page_offset(
        buffer_offset,
        buffer_length,
        read_offset,
        end_offset,
        zigzag,
    ) + 10
        + if range.is_some() { 35 } else { 0 }
        + 7
}
