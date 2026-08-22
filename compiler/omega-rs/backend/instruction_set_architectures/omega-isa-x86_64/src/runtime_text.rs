use super::host_calls::{
    HostCallPlan, append_add_rsp, append_sub_rsp, normalized_win64_file_io_layout,
    validate_composite_linux_syscall_plan, validate_normalized_win64_get_std_handle_plan,
};
use super::{
    Reg64, append_add_r10_imm32, append_add_r10_r11, append_add_r11_imm32, append_add_r11_rcx,
    append_add_rax_r11, append_imul_r11_imm32, append_input_delimiter_check, append_jcc_rel32,
    append_load_al_from_r15, append_load_r11_from_r15, append_load_r11_from_rax,
    append_load_r14_from_r14, append_load_r15_from_r15, append_load_rax_from_r14,
    append_load_rax_from_r15, append_load_rax_from_rcx, append_load_rcx_from_r15,
    append_load_rcx_from_rcx, append_load_unsigned_reg_from_r14, append_mov_r10_r14,
    append_mov_r11_rcx, append_mov_r14_imm64, append_mov_r15_imm64, append_mov_rcx_imm64,
    append_mov_rdi_r10, append_mov_rsi_rax, append_rep_movsb, append_store_r11_to_r15,
    append_store_r11_to_rax, append_store_r14_to_r15, append_store_r15_to_rax, disp32,
    element_scale, unsigned_load_width,
};
use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

pub const RUNTIME_TEXT_STORED_PLACE_APPEND_TARGET_IMM_OFFSET: usize = 10;
pub const RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET: usize = 33;
/// Like the non-pointee source offset, but the pointee variant inserts one extra
/// `mov r15, [r15+disp32]` (7 bytes) to dereference the runtime pointer before the
/// source-region `mov rcx, imm64`, pushing the source immediate from 33 to 40.
pub const RUNTIME_TEXT_STORED_PLACE_APPEND_POINTEE_SOURCE_IMM_OFFSET: usize = 40;

pub fn runtime_text_stored_place_append_width() -> usize {
    82
}

pub fn runtime_text_stored_place_append_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn runtime_text_stored_place_append_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// Appends a stored source string (a `{ptr,len}` descriptor in `source_region`)
/// to the end of a target string that lives in a fixed output `buffer`, updating
/// the target descriptor. r14=buffer base, r15=target region base, the source
/// region base is loaded into rcx. The copy itself is a `rep movsb`; the
/// enclosing callable frame preserves generated nonvolatile registers.
/// `buffer_offset` is unused (the append point is the
/// target's current length).
pub fn encode_runtime_text_stored_place_append(
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_place_append_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    append_mov_r15_imm64(&mut bytes, 0); // target region base (reloc @ +12)
    append_load_r11_from_r15(&mut bytes, target_offset + 8)?; // r11 = current length
    append_mov_r10_r14(&mut bytes); // r10 = buffer base
    append_add_r10_r11(&mut bytes); // r10 = dest = buffer + current length
    debug_assert_eq!(
        bytes.len(),
        RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET
    );
    append_mov_rcx_imm64(&mut bytes, 0); // source region base (reloc @ +2)
    append_load_rax_from_rcx(&mut bytes, source_offset)?; // rax = source pointer
    append_load_rcx_from_rcx(&mut bytes, source_offset + 8)?; // rcx = source length
    append_add_r11_rcx(&mut bytes); // r11 = new length = current + source
    append_store_r14_to_r15(&mut bytes, target_offset)?; // descriptor.ptr = buffer
    append_store_r11_to_r15(&mut bytes, target_offset + 8)?; // descriptor.len = new length
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    debug_assert_eq!(bytes.len(), runtime_text_stored_place_append_width());
    Ok(bytes)
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width() -> usize {
    89
}

/// Appends a stored source string to a target string whose `{ptr,len}` descriptor
/// is reached through a RUNTIME pointer: the descriptor lives at
/// `*(frame + pointer_byte_offset) + field_byte_offset`. Mirrors
/// `encode_runtime_text_stored_place_append`, but loads the descriptor base by
/// dereferencing the runtime pointer (one extra `mov r15,[r15+disp32]`) instead of
/// using a relocated target-region base. r14=materialized buffer base, r15=descriptor
/// address, rcx=source region base; the copy is a `rep movsb`, with generated
/// nonvolatile registers preserved by the enclosing callable frame.
/// The descriptor's `ptr` is overwritten to the buffer base and `len` grows by the
/// source length -- so a prior stale `ptr` (e.g. from WriteRuntimePointeeString) is
/// corrected here.
pub fn encode_runtime_text_stored_place_append_to_runtime_pointee(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_place_append_to_runtime_pointee_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ instruction start)
    append_mov_r15_imm64(&mut bytes, 0); // runtime-frame base (reloc @ +10 == TARGET offset)
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?; // r15 = runtime pointer
    append_load_r11_from_r15(&mut bytes, field_byte_offset + 8)?; // r11 = current length
    append_mov_r10_r14(&mut bytes); // r10 = buffer base
    append_add_r10_r11(&mut bytes); // r10 = dest = buffer + current length
    debug_assert_eq!(
        bytes.len(),
        RUNTIME_TEXT_STORED_PLACE_APPEND_POINTEE_SOURCE_IMM_OFFSET
    );
    append_mov_rcx_imm64(&mut bytes, 0); // source region base (reloc @ +40)
    append_load_rax_from_rcx(&mut bytes, source_offset)?; // rax = source pointer
    append_load_rcx_from_rcx(&mut bytes, source_offset + 8)?; // rcx = source length
    append_add_r11_rcx(&mut bytes); // r11 = new length = current + source
    append_store_r14_to_r15(&mut bytes, field_byte_offset)?; // descriptor.ptr = buffer
    append_store_r11_to_r15(&mut bytes, field_byte_offset + 8)?; // descriptor.len = new length
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    debug_assert_eq!(
        bytes.len(),
        runtime_text_stored_place_append_to_runtime_pointee_width()
    );
    Ok(bytes)
}

pub const RUNTIME_TEXT_STORED_SUFFIX_APPEND_SOURCE_IMM_OFFSET: usize = 10;
pub const RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET: usize = 55;

pub fn runtime_text_stored_suffix_append_width() -> usize {
    86
}

pub fn runtime_text_stored_suffix_append_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn runtime_text_stored_suffix_append_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// Writes a stored source string into `buffer + buffer_offset` and sets the
/// target descriptor to `{ buffer, source_len + length_delta }`. Used to build a
/// string whose first `length_delta` bytes are an already-present prefix.
pub fn encode_runtime_text_stored_suffix_append(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_suffix_append_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    append_mov_rcx_imm64(&mut bytes, 0); // source region base (reloc @ +12)
    append_load_rax_from_rcx(&mut bytes, source_offset)?; // rax = source pointer
    append_load_rcx_from_rcx(&mut bytes, source_offset + 8)?; // rcx = source length
    append_mov_r11_rcx(&mut bytes); // r11 = saved source length
    append_mov_r10_r14(&mut bytes); // r10 = buffer base
    append_add_r10_imm32(&mut bytes, buffer_offset)?; // r10 = dest = buffer + buffer_offset
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    debug_assert_eq!(
        bytes.len(),
        RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET
    );
    append_mov_r15_imm64(&mut bytes, 0); // target region base (reloc @ +2)
    append_store_r14_to_r15(&mut bytes, target_offset)?; // descriptor.ptr = buffer
    append_add_r11_imm32(&mut bytes, length_delta)?; // r11 = source_len + length_delta
    append_store_r11_to_r15(&mut bytes, target_offset + 8)?; // descriptor.len
    debug_assert_eq!(bytes.len(), runtime_text_stored_suffix_append_width());
    Ok(bytes)
}

pub fn runtime_text_literal_compare_width(literal: &[u8]) -> usize {
    10 + literal.len() * 15 + 36
}

// Write a literal's bytes into a runtime text buffer at a fixed byte offset
// (the first segment of a concatenation). r15 = buffer (reloc @ +2); store each
// literal byte at [r15 + byte_offset + i].
pub fn runtime_text_literal_segment_write_width(literal: &[u8]) -> usize {
    10 + literal.len() * 8
}

pub fn runtime_text_literal_segment_write_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R15])
}

pub fn runtime_text_literal_segment_write_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
}

pub fn encode_runtime_text_literal_segment_write(
    byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_segment_write_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    for (i, byte) in literal.iter().enumerate() {
        let disp = disp32(byte_offset + i)?;
        bytes.extend([0x41, 0xc6, 0x87]); // mov byte [r15 + disp32], imm8
        bytes.extend(disp.to_le_bytes());
        bytes.push(*byte);
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_text_literal_segment_write_width(literal)
    );
    Ok(bytes)
}

// Append a literal to a runtime text buffer, growing the {ptr,len} descriptor.
// r15 = buffer (reloc @ +2); r14 = descriptor base (reloc @ +12 via offset 10);
// rax = current len; store literal bytes at [r15 + len + i]; then
// descriptor.ptr = buffer, descriptor.len += literal.len.
pub const RUNTIME_TEXT_LITERAL_APPEND_TARGET_IMM_OFFSET: usize = 10;

pub fn runtime_text_literal_append_width(literal: &[u8]) -> usize {
    // mov r15,imm64 (10) + mov r14,imm64 (10) + mov rax,[r14+len] (7) = 27
    // + per byte: mov cl,imm8 (2) + mov [r15+rax],cl (4) + inc rax (3) = 9
    // + mov [r14+ptr],r15 (7) + mov [r14+len],rax (7) = 14
    27 + literal.len() * 9 + 14
}

pub fn runtime_text_literal_append_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn runtime_text_literal_append_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn encode_runtime_text_literal_append(
    target_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let ptr_disp = disp32(target_offset)?;
    let len_disp = disp32(target_offset + 8)?;
    let lit_len = i32::try_from(literal.len()).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot append literal of length `{}` yet",
            literal.len()
        ))
    })?;
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    append_mov_r14_imm64(&mut bytes, 0); // descriptor base (reloc @ +12)
    // rax = current length.
    bytes.extend([0x49, 0x8b, 0x86]); // mov rax, [r14 + len_disp]
    bytes.extend(len_disp.to_le_bytes());
    // append bytes at buffer[rax]; rax advances per byte.
    for byte in literal {
        bytes.extend([0xb1, *byte]); // mov cl, imm8
        bytes.extend([0x41, 0x88, 0x0c, 0x07]); // mov [r15+rax], cl
        bytes.extend([0x48, 0xff, 0xc0]); // inc rax
    }
    // descriptor.ptr = buffer (r15).
    bytes.extend([0x4d, 0x89, 0xbe]); // mov [r14 + ptr_disp], r15
    bytes.extend(ptr_disp.to_le_bytes());
    // descriptor.len = original_len + literal.len.  rax currently = len + lit.len
    // (advanced once per byte), so just store rax.
    let _ = lit_len;
    bytes.extend([0x49, 0x89, 0x86]); // mov [r14 + len_disp], rax
    bytes.extend(len_disp.to_le_bytes());
    debug_assert_eq!(bytes.len(), runtime_text_literal_append_width(literal));
    Ok(bytes)
}

pub fn runtime_text_literal_append_to_runtime_pointee_width(literal: &[u8]) -> usize {
    // Like the non-pointee literal append (41 + len*9) plus one extra
    // `mov r14, [r14 + disp32]` (7) to dereference the runtime pointer.
    48 + literal.len() * 9
}

/// Appends a compile-time literal to a target string whose `{ptr,len}` descriptor
/// is reached through a RUNTIME pointer (`*(frame + pointer_byte_offset) +
/// field_byte_offset`). Mirrors `encode_runtime_text_literal_append`, dereferencing
/// the runtime pointer into r14 first. r15=materialized buffer base. The descriptor
/// `ptr` is overwritten to the buffer base and `len` grows by the literal length.
pub fn encode_runtime_text_literal_append_to_runtime_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let ptr_disp = disp32(field_byte_offset)?;
    let len_disp = disp32(field_byte_offset + 8)?;
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_to_runtime_pointee_width(
        literal,
    ));
    append_mov_r15_imm64(&mut bytes, 0); // buffer base (reloc @ instruction start)
    append_mov_r14_imm64(&mut bytes, 0); // runtime-frame base (reloc @ +10 == TARGET offset)
    append_load_r14_from_r14(&mut bytes, pointer_byte_offset)?; // r14 = runtime pointer
    // rax = current length.
    bytes.extend([0x49, 0x8b, 0x86]); // mov rax, [r14 + len_disp]
    bytes.extend(len_disp.to_le_bytes());
    // append bytes at buffer[rax]; rax advances per byte.
    for byte in literal {
        bytes.extend([0xb1, *byte]); // mov cl, imm8
        bytes.extend([0x41, 0x88, 0x0c, 0x07]); // mov [r15+rax], cl
        bytes.extend([0x48, 0xff, 0xc0]); // inc rax
    }
    // descriptor.ptr = buffer (r15).
    bytes.extend([0x4d, 0x89, 0xbe]); // mov [r14 + ptr_disp], r15
    bytes.extend(ptr_disp.to_le_bytes());
    // descriptor.len = original_len + literal.len (rax advanced once per byte).
    bytes.extend([0x49, 0x89, 0x86]); // mov [r14 + len_disp], rax
    bytes.extend(len_disp.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_text_literal_append_to_runtime_pointee_width(literal)
    );
    Ok(bytes)
}

/// The target-region `mov r15, imm64` is the second instruction (after the
/// 10-byte buffer `mov r14, imm64`), so its relocated immediate sits at offset
/// 10 (the relocation planner adds the +2 imm position itself).
pub const RUNTIME_TEXT_BUFFER_MATERIALIZE_TARGET_IMM_OFFSET: usize = 10;

pub fn runtime_text_buffer_materialize_width() -> usize {
    // mov r14,imm64(10) + mov r15,imm64(10) + load rax,[r15+t](7) + load rcx,[r15+t+8](7)
    // + mov r11,rcx(3) + mov r10,r14(3) + mov rsi,rax(3) + mov rdi,r10(3)
    // + rep movsb(2) + store r14(7) + store r11(7). The callable frame already
    // preserves RSI/RDI, so body-local push/pop traffic would only exceed the
    // ordinary StatePlan's stack-pointer ceiling.
    62
}

/// Exact may-write set of the direct text-buffer materializer below. RSI and
/// RDI are restored by the enclosing callable frame, so this body sequence can
/// use them without transiently mutating RSP.
pub fn runtime_text_buffer_materialize_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn runtime_text_buffer_materialize_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
}

/// Materializes a fresh writable text buffer for an in-place concat: copies the
/// current `{ptr,len}` descriptor at `target_offset` (in the relocated target
/// region) into the relocated `buffer`, then repoints the descriptor at the
/// buffer (ptr=buffer, len unchanged). A later append then grows the copy in
/// place without disturbing the original literal/source the descriptor named.
pub fn encode_runtime_text_buffer_materialize(target_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_buffer_materialize_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ instruction start)
    append_mov_r15_imm64(&mut bytes, 0); // target region base (reloc @ +10)
    append_load_rax_from_r15(&mut bytes, target_offset)?; // rax = source pointer
    append_load_rcx_from_r15(&mut bytes, target_offset + 8)?; // rcx = source length
    append_mov_r11_rcx(&mut bytes); // r11 = saved length
    append_mov_r10_r14(&mut bytes); // r10 = dest = buffer base
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_store_r14_to_r15(&mut bytes, target_offset)?; // descriptor.ptr = buffer
    append_store_r11_to_r15(&mut bytes, target_offset + 8)?; // descriptor.len = original length
    debug_assert_eq!(bytes.len(), runtime_text_buffer_materialize_width());
    Ok(bytes)
}

pub fn runtime_text_buffer_materialize_to_runtime_pointee_width() -> usize {
    runtime_text_buffer_materialize_width() + 7
}

pub fn runtime_text_buffer_materialize_to_runtime_pointee_register_writes() -> RegisterSet {
    runtime_text_buffer_materialize_register_writes()
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_buffer_materialize_to_runtime_pointee_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ instruction start)
    append_mov_r15_imm64(&mut bytes, 0); // runtime-frame base (reloc @ +10)
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?;
    append_load_rax_from_r15(&mut bytes, field_byte_offset)?;
    append_load_rcx_from_r15(&mut bytes, field_byte_offset + 8)?;
    append_mov_r11_rcx(&mut bytes);
    append_mov_r10_r14(&mut bytes);
    append_mov_rsi_rax(&mut bytes);
    append_mov_rdi_r10(&mut bytes);
    append_rep_movsb(&mut bytes);
    append_store_r14_to_r15(&mut bytes, field_byte_offset)?;
    append_store_r11_to_r15(&mut bytes, field_byte_offset + 8)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_text_buffer_materialize_to_runtime_pointee_width()
    );
    Ok(bytes)
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
    index_byte_size: usize,
) -> usize {
    frame_indexed_string_prefix_width(index_byte_size) + 55
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_buffer_imm_offset(
    index_byte_size: usize,
) -> usize {
    frame_indexed_string_prefix_width(index_byte_size) + 3
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes() -> RegisterSet {
    runtime_text_buffer_materialize_register_writes()
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_text_buffer_materialize_to_runtime_frame_indexed_width(index_byte_size),
    );
    append_frame_indexed_element_address_into_rax(
        &mut bytes,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
    )?;
    bytes.extend([0x49, 0x89, 0xc7]); // mov r15, rax (indexed descriptor base)
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ prefix + 3)
    append_load_rax_from_r15(&mut bytes, field_byte_offset)?;
    append_load_rcx_from_r15(&mut bytes, field_byte_offset + 8)?;
    append_mov_r11_rcx(&mut bytes);
    append_mov_r10_r14(&mut bytes);
    append_mov_rsi_rax(&mut bytes);
    append_mov_rdi_r10(&mut bytes);
    append_rep_movsb(&mut bytes);
    append_store_r14_to_r15(&mut bytes, field_byte_offset)?;
    append_store_r11_to_r15(&mut bytes, field_byte_offset + 8)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_text_buffer_materialize_to_runtime_frame_indexed_width(index_byte_size)
    );
    Ok(bytes)
}

#[cfg(test)]
mod text_buffer_materialize_place_tests {
    use super::*;

    #[test]
    fn pointee_materialize_width_matches_emission() {
        let bytes = encode_runtime_text_buffer_materialize_to_runtime_pointee(24, 8)
            .expect("pointee text-buffer materialization");
        assert_eq!(
            bytes.len(),
            runtime_text_buffer_materialize_to_runtime_pointee_width()
        );
    }

    #[test]
    fn frame_indexed_materialize_width_and_buffer_site_follow_index_width() {
        for index_byte_size in [1usize, 2, 4, 8] {
            let bytes = encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
                24,
                40,
                index_byte_size,
                16,
                8,
            )
            .expect("frame-indexed text-buffer materialization");
            let buffer_site =
                runtime_text_buffer_materialize_to_runtime_frame_indexed_buffer_imm_offset(
                    index_byte_size,
                );
            assert_eq!(
                bytes.len(),
                runtime_text_buffer_materialize_to_runtime_frame_indexed_width(index_byte_size)
            );
            assert_eq!(&bytes[buffer_site..buffer_site + 2], &[0x49, 0xbe]);
        }
    }

    #[test]
    fn frame_indexed_stored_append_width_and_relocation_sites_follow_index_width() {
        for index_byte_size in [1usize, 2, 4, 8] {
            let bytes = encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
                56,
                24,
                40,
                index_byte_size,
                32,
                8,
            )
            .expect("frame-indexed stored-text append");
            let buffer_site =
                runtime_text_stored_place_append_to_runtime_frame_indexed_buffer_imm_offset(
                    index_byte_size,
                );
            let source_site =
                runtime_text_stored_place_append_to_runtime_frame_indexed_source_imm_offset(
                    index_byte_size,
                );
            assert_eq!(
                bytes.len(),
                runtime_text_stored_place_append_to_runtime_frame_indexed_width(index_byte_size)
            );
            assert_eq!(&bytes[buffer_site..buffer_site + 2], &[0x49, 0xbe]);
            assert_eq!(&bytes[source_site..source_site + 2], &[0x48, 0xb9]);
        }
    }
}

pub fn runtime_text_literal_compare_branch_next_offset(byte_index: usize) -> usize {
    10 + byte_index * 15 + 15
}

/// Exact register writes of the literal-buffer guard encoder below. Every
/// path materializes the buffer in r15 and loads the compared/delimiter byte
/// through AL before its flag-setting comparisons.
pub fn runtime_text_literal_compare_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86R15])
}

pub fn runtime_text_literal_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn encode_runtime_text_literal_compare(
    literal: &[u8],
    failure_branch_distances: impl ExactSizeIterator<Item = isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    if literal.len() != failure_branch_distances.len() {
        return Err(Diagnostic::error(format!(
            "X86_64 runtime text guard expected {} branch distance(s), got {}",
            literal.len(),
            failure_branch_distances.len()
        )));
    }

    let mut bytes = Vec::with_capacity(runtime_text_literal_compare_width(literal));
    append_mov_r15_imm64(&mut bytes, 0);
    for (byte_index, (expected_byte, failure_branch_distance)) in
        literal.iter().zip(failure_branch_distances).enumerate()
    {
        append_load_al_from_r15(&mut bytes, byte_index)?;
        bytes.extend([0x3c, *expected_byte]); // cmp al, imm8
        append_jcc_rel32(&mut bytes, 0x85, failure_branch_distance)?; // jne
    }
    append_input_delimiter_check(
        &mut bytes,
        literal.len(),
        delimiter_failure_branch_distance - 4,
    )?;
    Ok(bytes)
}

// Compare a stored String (descriptor {ptr,len} at source storage) against a
// data-section literal of known length.
//
// The lowering wraps this compare as: write the optimistic result (text_ok=1) ->
// COMPARE -> write the failure result (text_ok=0). On a MATCH we must branch
// PAST the trailing "write 0" (keeping the optimistic 1); on a MISMATCH we fall
// through into it. So MATCH jumps to the external distance ("next guarded effect
// end") and MISMATCH falls through. Every internal match path funnels through a
// single terminal `jmp rel32` so emission only needs one branch offset.
//
// r15 = literal buffer (reloc @ instruction start +2); r14 = source base (reloc);
// rax = stored.ptr; r9 = stored.len; r8 = index; cl = scratch byte.
//
// Layout: [setup + compare loop + trailing delimiter check] ... fail: (fall
// through) ; match: jmp rel32(external)   <- terminal 5 bytes; rel32 end == width.
pub fn encode_runtime_text_storage_compare_bytes(
    source_offset: usize,
    literal_len: usize,
    match_branch_distance: isize,
    negated: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let literal_len_i = i32::try_from(literal_len).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot compare literal of length `{literal_len}` yet"
        ))
    })?;
    let mut bytes = Vec::new();
    let mut fail_fixups: Vec<usize> = Vec::new();
    let mut success_fixups: Vec<usize> = Vec::new();

    // r15 = literal base (reloc@+2); r14 = source base (reloc).
    append_mov_r15_imm64(&mut bytes, 0);
    append_mov_r14_imm64(&mut bytes, 0);
    append_load_rax_from_r14(&mut bytes, source_offset, 8)?; // rax = stored.ptr
    bytes.extend([0x4d, 0x8b, 0x8e]); // mov r9, [r14 + disp32]  (stored.len)
    bytes.extend(disp32(source_offset + 8)?.to_le_bytes());

    let mut jcc_fail = |bytes: &mut Vec<u8>, opcode: u8| {
        bytes.push(0x0f);
        bytes.push(opcode);
        fail_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    };
    // stored.len < literal_len  => not equal.
    bytes.extend([0x49, 0x81, 0xf9]); // cmp r9, imm32
    bytes.extend(literal_len_i.to_le_bytes());
    jcc_fail(&mut bytes, 0x82); // jb fail
    bytes.extend([0x4d, 0x31, 0xc0]); // xor r8, r8 (index = 0)

    let loop_start = bytes.len();
    bytes.extend([0x49, 0x81, 0xf8]); // cmp r8, imm32 (literal_len)
    bytes.extend(literal_len_i.to_le_bytes());
    let to_trailing = {
        bytes.extend([0x0f, 0x83]); // jae rel32 -> trailing check
        let at = bytes.len();
        bytes.extend([0, 0, 0, 0]);
        at
    };
    bytes.extend([0x42, 0x8a, 0x0c, 0x00]); // mov cl, [rax+r8]
    bytes.extend([0x43, 0x3a, 0x0c, 0x07]); // cmp cl, [r15+r8]
    jcc_fail(&mut bytes, 0x85); // jne fail
    bytes.extend([0x49, 0xff, 0xc0]); // inc r8
    {
        bytes.push(0xe9); // jmp loop_start
        let at = bytes.len();
        bytes.extend([0, 0, 0, 0]);
        bytes[at..at + 4]
            .copy_from_slice(&((loop_start as isize - (at as isize + 4)) as i32).to_le_bytes());
    }

    // trailing: if stored.len == literal_len -> success; else stored[len] must
    // be a line delimiter for equality (input had a trailing terminator).
    let trailing = bytes.len();
    bytes[to_trailing..to_trailing + 4]
        .copy_from_slice(&((trailing as isize - (to_trailing as isize + 4)) as i32).to_le_bytes());
    bytes.extend([0x49, 0x81, 0xf9]); // cmp r9, imm32 (literal_len)
    bytes.extend(literal_len_i.to_le_bytes());
    {
        bytes.extend([0x0f, 0x84]); // je success
        success_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    }
    bytes.extend([0x42, 0x8a, 0x0c, 0x00]); // mov cl, [rax+r8] (stored[literal_len])
    let mut je_success = |bytes: &mut Vec<u8>, imm: u8| {
        bytes.extend([0x80, 0xf9, imm]); // cmp cl, imm8
        bytes.extend([0x0f, 0x84]); // je success
        success_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    };
    je_success(&mut bytes, 0x0a); // '\n'
    je_success(&mut bytes, 0x0d); // '\r'
    je_success(&mut bytes, 0x00); // '\0'
    {
        bytes.push(0xe9); // jmp fail (no delimiter -> not equal)
        fail_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    }

    // Exit trampolines. The FIRST falls through to the instruction end (the
    // following "write text_ok = 0"); the SECOND jmps the external "next
    // guarded effect end" distance, skipping that write. `negated` (a `!=`
    // compare) swaps which OUTCOME routes where: `==` sends match outcomes
    // external; `!=` sends MISMATCH outcomes external -- the flag was ignored
    // and `!=` behaved as `==` (the frame-slot text-comparison writer's
    // preset-1/compare/write-0 pattern kept the 1 for equal strings). Same
    // byte layout either way; only the fixup routing differs.
    let (end_fixups, external_fixups) = if negated {
        (success_fixups, fail_fixups)
    } else {
        (fail_fixups, success_fixups)
    };
    let mismatch = bytes.len();
    bytes.push(0xe9);
    let mismatch_jmp_at = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    for fixup in &end_fixups {
        bytes[*fixup..*fixup + 4]
            .copy_from_slice(&((mismatch as isize - (*fixup as isize + 4)) as i32).to_le_bytes());
    }

    let matched = bytes.len();
    for fixup in &external_fixups {
        bytes[*fixup..*fixup + 4]
            .copy_from_slice(&((matched as isize - (*fixup as isize + 4)) as i32).to_le_bytes());
    }
    bytes.push(0xe9); // jmp match target (rel32)
    let match_jmp_at = bytes.len();
    bytes.extend((match_branch_distance as i32).to_le_bytes());

    let width = bytes.len();
    // mismatch path jumps to the instruction end (the trailing write-0).
    bytes[mismatch_jmp_at..mismatch_jmp_at + 4]
        .copy_from_slice(&((width as isize - (mismatch_jmp_at as isize + 4)) as i32).to_le_bytes());
    debug_assert_eq!(
        match_jmp_at + 4,
        width,
        "match jmp must terminate the instruction"
    );

    Ok(bytes)
}

/// Exact register writes of the descriptor-vs-literal content comparison.
/// The emitted loop owns both relocated bases, pointer/length/index state,
/// and CL as its byte scratch.
pub fn runtime_text_storage_compare_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn runtime_text_storage_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// Byte offset (within a `CompareRuntimeTextStorage`) of the rel32 displacement
/// end of the terminal failure `jmp` -- i.e. the instruction width. Emission
/// anchors the failure branch distance here.
pub fn runtime_text_storage_compare_failure_branch_offset(literal_len: usize) -> usize {
    runtime_text_storage_compare_width_x86(literal_len)
}

pub fn runtime_text_storage_compare_width_x86(literal_len: usize) -> usize {
    // Encode once with placeholder distance to recover the authoritative width.
    encode_runtime_text_storage_compare_bytes(0, literal_len, 0, false)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

// --- Frame-indexed String descriptor write + literal append ---
//
// The String descriptor lives at `*(frame+descriptor_offset) + index*elem +
// field` (a slice element reached through the slice's data pointer). Both
// encoders share an address-computation prefix whose selected-width index load
// leaves the element address in rax:
//   mov r14,imm64(frame) (10, reloc@+2) ; mov rax,[r14+descriptor] (7)
//   load-zx r11,[r14+index] ; imul r11,r11,elem (7) ; add rax,r11 (3)
// The second relocated immediate follows this width-aware prefix.
fn frame_indexed_string_prefix_width(index_byte_size: usize) -> usize {
    27 + unsigned_load_width(index_byte_size)
}

fn append_frame_indexed_element_address_into_rax(
    bytes: &mut Vec<u8>,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
) -> Result<(), Diagnostic> {
    append_mov_r14_imm64(bytes, 0); // frame base (reloc @ +2)
    append_load_rax_from_r14(bytes, descriptor_offset, 8)?; // rax = slice data ptr
    append_load_unsigned_reg_from_r14(bytes, Reg64::R11, index_offset, index_byte_size)?;
    append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
    append_add_rax_r11(bytes); // rax = element address
    debug_assert_eq!(
        bytes.len(),
        frame_indexed_string_prefix_width(index_byte_size)
    );
    Ok(())
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_width(
    index_byte_size: usize,
) -> usize {
    frame_indexed_string_prefix_width(index_byte_size) + 75
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_buffer_imm_offset(
    index_byte_size: usize,
) -> usize {
    frame_indexed_string_prefix_width(index_byte_size) + 3
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_source_imm_offset(
    index_byte_size: usize,
) -> usize {
    frame_indexed_string_prefix_width(index_byte_size) + 26
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes() -> RegisterSet {
    runtime_text_stored_place_append_register_writes()
}

/// Appends a stored source descriptor to a text descriptor inside a
/// runtime-indexed frame slice. The shared prefix resolves the indexed element
/// into rax; r15 then retains that descriptor base while r14 holds the relocated
/// output buffer and rcx is reused for the relocated source descriptor.
pub fn encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_text_stored_place_append_to_runtime_frame_indexed_width(index_byte_size),
    );
    append_frame_indexed_element_address_into_rax(
        &mut bytes,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
    )?;
    bytes.extend([0x49, 0x89, 0xc7]); // mov r15, rax (indexed descriptor base)
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ prefix + 3)
    append_load_r11_from_r15(&mut bytes, field_byte_offset + 8)?;
    append_mov_r10_r14(&mut bytes);
    append_add_r10_r11(&mut bytes);
    debug_assert_eq!(
        bytes.len(),
        runtime_text_stored_place_append_to_runtime_frame_indexed_source_imm_offset(
            index_byte_size,
        )
    );
    append_mov_rcx_imm64(&mut bytes, 0); // source region base
    append_load_rax_from_rcx(&mut bytes, source_offset)?;
    append_load_rcx_from_rcx(&mut bytes, source_offset + 8)?;
    append_add_r11_rcx(&mut bytes);
    append_store_r14_to_r15(&mut bytes, field_byte_offset)?;
    append_store_r11_to_r15(&mut bytes, field_byte_offset + 8)?;
    append_mov_rsi_rax(&mut bytes);
    append_mov_rdi_r10(&mut bytes);
    append_rep_movsb(&mut bytes);
    debug_assert_eq!(
        bytes.len(),
        runtime_text_stored_place_append_to_runtime_frame_indexed_width(index_byte_size)
    );
    Ok(bytes)
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    index_byte_size: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    // width-aware prefix + mov r15,imm64 buffer (10)
    // + mov r11,[rax+field+8] len (7)
    // + per byte: mov cl,imm8 (2) + mov [r15+r11],cl (4) + inc r11 (3) = 9
    // + store r15->[rax+field] ptr (7) + store r11->[rax+field+8] len (7)
    frame_indexed_string_prefix_width(index_byte_size) + 17 + literal.len() * 9 + 14
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub const RUNTIME_TEXT_INDEXED_LITERAL_APPEND_BUFFER_IMM_OFFSET: usize = 34;

pub fn encode_runtime_text_literal_append_to_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_to_runtime_frame_indexed_width(
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        literal,
    ));
    append_frame_indexed_element_address_into_rax(
        &mut bytes,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
    )?;
    // r15 = buffer (reloc @ prefix+2); r11 = current len from the indexed descriptor.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_r11_from_rax(&mut bytes, field_byte_offset + 8)?;
    // append bytes at buffer[len]; r11 advances per byte.
    for byte in literal {
        bytes.extend([0xb1, *byte]); // mov cl, imm8
        bytes.extend([0x43, 0x88, 0x0c, 0x1f]); // mov [r15+r11], cl
        bytes.extend([0x49, 0xff, 0xc3]); // inc r11
    }
    // descriptor.ptr = buffer (r15); descriptor.len = r11 (already grown).
    append_store_r15_to_rax(&mut bytes, field_byte_offset)?;
    append_store_r11_to_rax(&mut bytes, field_byte_offset + 8)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_text_literal_append_to_runtime_frame_indexed_width(
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal
        )
    );
    Ok(bytes)
}

// --- Machine-indexed String descriptor write ---
//
// Writes {ptr,len} into a machine-owned array element `machine[base + index*elem
// + field]` (the array is inline, so no pointer deref -- unlike the frame slice
// variant). The index lives in a runtime-frame slot. Fixed prefix leaves the
// element address in r15:
//   mov r15,imm64(machine) (10, reloc@+2) ; mov r14,imm64(frame) (10, reloc@+12)
//   mov r11,[r14+index] (7) ; imul r11,r11,elem (7) ; add r15,r11 (3)
// so the runtime-frame reloc imm is at offset 12 and the literal reloc at 39.
const MACHINE_INDEXED_STRING_PREFIX_WIDTH: usize = 37;
pub const MACHINE_INDEXED_STRING_FRAME_IMM_OFFSET: usize = 10;
pub const MACHINE_INDEXED_STRING_DATA_IMM_OFFSET: usize = MACHINE_INDEXED_STRING_PREFIX_WIDTH;

// --- Runtime text line read (Windows stdin via GetStdHandle + ReadFile) ---
//
// Self-contained instruction reading ONE logical line. Stdin is read one byte
// at a time (a bulk ReadFile would consume bytes belonging to the next
// read_line); the loop calls ReadFile with count=1 until a \n/\r/\0 delimiter,
// EOF (0 bytes), or capacity, then stores {ptr, len} at the target descriptor.
//
// Win64 callee-saved r13/r14/r15 survive the ReadFile call:
//   r14 = buffer base   r13 = stdin handle   r15 = line length / write index
//
// Branch displacements are resolved by post-patching recorded label positions,
// so the four relocation offsets are read back from the encoder rather than
// hand-computed; see `runtime_text_line_read_relocation_offsets`.

/// Byte offsets (within the instruction) of the four relocations the planner
/// must patch: buffer imm64, GetStdHandle call rel32, ReadFile call rel32,
/// and the target-descriptor imm64. Computed by encoding once with a dummy
/// target so the layout is authoritative (no hand-maintained constants).
pub struct RuntimeTextLineReadLayout {
    pub get_std_handle_call_offset: usize,
    pub read_file_call_offset: usize,
    pub target_imm_offset: usize,
    pub width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeTextReadTarget {
    StringDescriptor,
    BoundedByteCarrier,
    RawFixedArray,
}

fn build_runtime_text_line_read(
    target_offset: usize,
    capacity: u32,
    target: RuntimeTextReadTarget,
) -> Result<(Vec<u8>, RuntimeTextLineReadLayout), Diagnostic> {
    validate_normalized_win64_get_std_handle_plan(HostCallPlan::CompatibilityOracle)?;
    let file_layout = normalized_win64_file_io_layout(HostCallPlan::CompatibilityOracle)?;
    let target_ptr_disp = disp32(target_offset)?;
    let target_len_disp = disp32(target_offset + 8)?;
    // Owned carrier: r14 must point at the inline bytes (`region + target_offset +
    // pointer_size`), so the imm64 relocates to the carrier's own region and an
    // `add` advances past the leading 8-byte length word.
    let direct_bytes_disp = disp32(match target {
        RuntimeTextReadTarget::BoundedByteCarrier => target_offset + 8,
        RuntimeTextReadTarget::RawFixedArray => target_offset,
        RuntimeTextReadTarget::StringDescriptor => 0,
    })?;
    let mut bytes = Vec::with_capacity(128);

    // r14 = read buffer (imm64 at +2 relocated to the buffer data symbol, OR to
    // the carrier's own region for an owned `[u8; N]` target).
    bytes.extend([0x49, 0xbe]);
    bytes.extend(0u64.to_le_bytes());
    if target != RuntimeTextReadTarget::StringDescriptor {
        // Add the inline destination offset. A carrier skips its length word;
        // a raw fixed array starts at the place itself.
        bytes.extend([0x49, 0x81, 0xc6]);
        bytes.extend(direct_bytes_disp.to_le_bytes());
    }
    append_sub_rsp(&mut bytes, file_layout.reserve);
    // mov ecx, -10 (STD_INPUT_HANDLE).
    bytes.push(0xb9);
    bytes.extend((-10i32).to_le_bytes());
    // call GetStdHandle (rel32).
    bytes.push(0xe8);
    let get_std_handle_call_offset = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    // r13 = handle; r15 = 0.
    bytes.extend([0x49, 0x89, 0xc5]); // mov r13, rax
    bytes.extend([0x4d, 0x31, 0xff]); // xor r15, r15

    let loop_start = bytes.len();
    bytes.extend([0x4c, 0x89, 0xe9]); // mov rcx, r13 (handle)
    bytes.extend([0x4b, 0x8d, 0x14, 0x3e]); // lea rdx, [r14+r15]
    bytes.extend([0x41, 0xb8, 0x01, 0x00, 0x00, 0x00]); // mov r8d, 1
    bytes.extend([0x4c, 0x8d, 0x4c, 0x24, file_layout.transferred_disp]);
    bytes.extend([
        0x48,
        0xc7,
        0x44,
        0x24,
        file_layout.overlapped_disp,
        0,
        0,
        0,
        0,
    ]);
    bytes.push(0xe8); // call ReadFile (rel32)
    let read_file_call_offset = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x8b, 0x44, 0x24, file_layout.transferred_disp]);

    // Forward jumps to `done`, patched after `done` is known.
    let mut done_fixups: Vec<usize> = Vec::new();
    let mut jcc_done = |bytes: &mut Vec<u8>, opcode: u8| {
        bytes.push(0x0f);
        bytes.push(opcode);
        done_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    };
    bytes.extend([0x85, 0xc0]); // test eax, eax
    jcc_done(&mut bytes, 0x84); // je done (EOF)
    bytes.extend([0x43, 0x8a, 0x04, 0x3e]); // mov al, [r14+r15] (byte read)
    // A '\n'/'\r' delimiter terminates the line only once content is present
    // (r15 > 0); a LEADING one is skipped (loop back without accepting it). This
    // makes CRLF a single terminator -- the '\n' trailing a '\r'-ended line, and
    // a bare Enter, no longer surface as a phantom empty line to the next
    // read_line. Per delimiter: cmp al,d; jne over; test r15,r15; jnz done;
    // jmp loop_start; over:
    for delim in [0x0au8, 0x0du8] {
        bytes.extend([0x3c, delim]); // cmp al, delim
        bytes.push(0x75); // jne over (skip the eol-handling block)
        let jne_over = bytes.len();
        bytes.push(0x00);
        bytes.extend([0x4d, 0x85, 0xff]); // test r15, r15
        jcc_done(&mut bytes, 0x85); // jnz done (content present -> finish line)
        bytes.push(0xe9); // jmp loop_start (leading delimiter: skip, read next)
        let jmp_loop = bytes.len();
        bytes.extend([0, 0, 0, 0]);
        let rel = loop_start as isize - (jmp_loop as isize + 4);
        bytes[jmp_loop..jmp_loop + 4].copy_from_slice(&(rel as i32).to_le_bytes());
        let over = bytes.len();
        bytes[jne_over] = (over - (jne_over + 1)) as u8;
    }
    bytes.extend([0x3c, 0x00]); // cmp al, 0
    jcc_done(&mut bytes, 0x84); // a NUL always terminates (EOF sentinel)
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15 (accept the byte)
    // cmp r15, capacity ; jb loop  (keep reading while length < capacity, else
    // fall through to done so we never overrun the buffer).
    bytes.extend([0x49, 0x81, 0xff]); // cmp r15, imm32
    bytes.extend(capacity.to_le_bytes());
    bytes.extend([0x0f, 0x82]); // jb rel32
    let loop_jmp_disp = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    {
        let rel = loop_start as isize - (loop_jmp_disp as isize + 4);
        bytes[loop_jmp_disp..loop_jmp_disp + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }

    // done:
    let done = bytes.len();
    for fixup in done_fixups {
        let rel = done as isize - (fixup as isize + 4);
        bytes[fixup..fixup + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }
    append_add_rsp(&mut bytes, file_layout.reserve);

    let target_mov_offset = if target == RuntimeTextReadTarget::BoundedByteCarrier {
        // Owned carrier: the bytes are already in place (r14 read straight into the
        // inline storage). Write only the length at `[r14 - 8]` (= region +
        // target_offset, the leading len word). No `{ptr, len}` descriptor, hence
        // no second relocation.
        bytes.extend([0x4d, 0x89, 0x7e, 0xf8]); // mov [r14-8], r15
        0
    } else if target == RuntimeTextReadTarget::RawFixedArray {
        // Bytes already landed in disposable scratch. No descriptor or length
        // word exists, so no epilogue store or second relocation is needed.
        0
    } else {
        // r13 = target descriptor base (imm64 relocated). The relocation planner
        // anchors at the instruction start and adds the +2 immediate offset itself,
        // so record the start.
        let target_mov_offset = bytes.len();
        bytes.extend([0x49, 0xbd]);
        bytes.extend(0u64.to_le_bytes());
        // mov [r13+target_offset], r14  (descriptor.ptr = buffer).
        bytes.extend([0x4d, 0x89, 0xb5]);
        bytes.extend(target_ptr_disp.to_le_bytes());
        // mov [r13+target_offset+8], r15 (descriptor.len = line length).
        bytes.extend([0x4d, 0x89, 0xbd]);
        bytes.extend(target_len_disp.to_le_bytes());
        target_mov_offset
    };

    let width = bytes.len();
    Ok((
        bytes,
        RuntimeTextLineReadLayout {
            get_std_handle_call_offset,
            read_file_call_offset,
            target_imm_offset: target_mov_offset,
            width,
        },
    ))
}

fn runtime_text_line_read_layout_for(target: RuntimeTextReadTarget) -> RuntimeTextLineReadLayout {
    // Capacity/target do not affect the layout (all immediates are fixed width),
    // so encode once with placeholders to recover the authoritative offsets.
    build_runtime_text_line_read(0, 1, target)
        .expect("runtime text line read layout encodes")
        .1
}

fn runtime_text_line_read_layout() -> RuntimeTextLineReadLayout {
    runtime_text_line_read_layout_for(RuntimeTextReadTarget::StringDescriptor)
}

pub fn runtime_text_line_read_width(_byte_capacity: usize) -> usize {
    runtime_text_line_read_layout().width
}

pub fn runtime_text_line_read_get_std_handle_call_offset() -> usize {
    runtime_text_line_read_layout().get_std_handle_call_offset
}

pub fn runtime_text_line_read_read_file_call_offset() -> usize {
    runtime_text_line_read_layout().read_file_call_offset
}

pub fn runtime_text_line_read_target_imm_offset() -> usize {
    runtime_text_line_read_layout().target_imm_offset
}

/// Owned `[u8; N]` carrier read encodes a wider prologue (the `add r14` past the
/// length word) and a shorter epilogue (a single `len` store, no `{ptr, len}`
/// descriptor), so its import-call offsets and width differ from the String path.
pub fn runtime_text_line_read_carrier_width(_byte_capacity: usize) -> usize {
    runtime_text_line_read_layout_for(RuntimeTextReadTarget::BoundedByteCarrier).width
}

pub fn runtime_text_line_read_carrier_get_std_handle_call_offset() -> usize {
    runtime_text_line_read_layout_for(RuntimeTextReadTarget::BoundedByteCarrier)
        .get_std_handle_call_offset
}

pub fn runtime_text_line_read_carrier_read_file_call_offset() -> usize {
    runtime_text_line_read_layout_for(RuntimeTextReadTarget::BoundedByteCarrier)
        .read_file_call_offset
}

pub fn runtime_text_line_read_fixed_array_width(_byte_capacity: usize) -> usize {
    runtime_text_line_read_layout_for(RuntimeTextReadTarget::RawFixedArray).width
}

pub fn runtime_text_line_read_fixed_array_get_std_handle_call_offset() -> usize {
    runtime_text_line_read_layout_for(RuntimeTextReadTarget::RawFixedArray)
        .get_std_handle_call_offset
}

pub fn runtime_text_line_read_fixed_array_read_file_call_offset() -> usize {
    runtime_text_line_read_layout_for(RuntimeTextReadTarget::RawFixedArray).read_file_call_offset
}

/// x86_64 Linux line read via the `read(2)` syscall (no GetStdHandle/ReadFile imports).
/// Byte-at-a-time read from stdin (fd 0) into the relocated buffer (r14), tracking the
/// line length in r15, with the same CRLF/NUL terminator handling as the win32 import
/// path, then store the {pointer, length} String descriptor into the target region.
pub fn encode_runtime_text_line_read_syscall(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    validate_composite_linux_syscall_plan(
        parameter_registers,
        result_register,
        number_register,
        supervisor_call,
    )?;
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read_syscall(
        target_offset,
        capacity,
        number,
        RuntimeTextReadTarget::StringDescriptor,
    )?
    .0)
}

/// Linux `read(2)` line read into an owned `[u8; N]` carrier: stdin bytes land in
/// the carrier's inline storage and the line length is written to its leading
/// length word; no `{ptr, len}` descriptor.
pub fn encode_runtime_text_line_read_syscall_carrier(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    validate_composite_linux_syscall_plan(
        parameter_registers,
        result_register,
        number_register,
        supervisor_call,
    )?;
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read_syscall(
        target_offset,
        capacity,
        number,
        RuntimeTextReadTarget::BoundedByteCarrier,
    )?
    .0)
}

pub fn encode_runtime_text_line_read_syscall_fixed_array(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    validate_composite_linux_syscall_plan(
        parameter_registers,
        result_register,
        number_register,
        supervisor_call,
    )?;
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read_syscall(
        target_offset,
        capacity,
        number,
        RuntimeTextReadTarget::RawFixedArray,
    )?
    .0)
}

fn build_runtime_text_line_read_syscall(
    target_offset: usize,
    capacity: u32,
    number: u32,
    target: RuntimeTextReadTarget,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    let target_ptr_disp = disp32(target_offset)?;
    let target_len_disp = disp32(target_offset + 8)?;
    let direct_bytes_disp = disp32(match target {
        RuntimeTextReadTarget::BoundedByteCarrier => target_offset + 8,
        RuntimeTextReadTarget::RawFixedArray => target_offset,
        RuntimeTextReadTarget::StringDescriptor => 0,
    })?;
    let mut bytes = Vec::with_capacity(128);

    // r14 = read buffer (imm64 at +2 relocated to the buffer data symbol, OR to the
    // carrier's own region for an owned `[u8; N]` target); r15 = length.
    bytes.extend([0x49, 0xbe]);
    bytes.extend(0u64.to_le_bytes());
    if target != RuntimeTextReadTarget::StringDescriptor {
        // add r14, target_offset + pointer_size -> r14 = carrier inline bytes.
        bytes.extend([0x49, 0x81, 0xc6]);
        bytes.extend(direct_bytes_disp.to_le_bytes());
    }
    bytes.extend([0x4d, 0x31, 0xff]); // xor r15, r15

    let loop_start = bytes.len();
    bytes.extend([0x31, 0xff]); // xor edi, edi (fd = 0, stdin)
    bytes.extend([0x4b, 0x8d, 0x34, 0x3e]); // lea rsi, [r14+r15] (buffer + length)
    bytes.extend([0xba, 0x01, 0x00, 0x00, 0x00]); // mov edx, 1 (read one byte)
    bytes.push(0xb8); // mov eax, read-syscall-number
    bytes.extend(number.to_le_bytes());
    bytes.extend([0x0f, 0x05]); // syscall
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax (rax = bytes read / -errno)

    // Forward jumps to `done`, patched after `done` is known.
    let mut done_fixups: Vec<usize> = Vec::new();
    let mut jcc_done = |bytes: &mut Vec<u8>, opcode: u8| {
        bytes.push(0x0f);
        bytes.push(opcode);
        done_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    };
    jcc_done(&mut bytes, 0x8e); // jle done (read returned 0 (EOF) or < 0 (error))
    bytes.extend([0x43, 0x8a, 0x04, 0x3e]); // mov al, [r14+r15] (byte read)
    // A '\n'/'\r' delimiter terminates the line only once content is present
    // (r15 > 0); a LEADING one is skipped (loop back without accepting it), so CRLF
    // is a single terminator. Mirrors the win32 import path's terminator handling.
    for delim in [0x0au8, 0x0du8] {
        bytes.extend([0x3c, delim]); // cmp al, delim
        bytes.push(0x75); // jne over
        let jne_over = bytes.len();
        bytes.push(0x00);
        bytes.extend([0x4d, 0x85, 0xff]); // test r15, r15
        jcc_done(&mut bytes, 0x85); // jnz done (content present -> finish line)
        bytes.push(0xe9); // jmp loop_start (leading delimiter: skip, read next)
        let jmp_loop = bytes.len();
        bytes.extend([0, 0, 0, 0]);
        let rel = loop_start as isize - (jmp_loop as isize + 4);
        bytes[jmp_loop..jmp_loop + 4].copy_from_slice(&(rel as i32).to_le_bytes());
        let over = bytes.len();
        bytes[jne_over] = (over - (jne_over + 1)) as u8;
    }
    bytes.extend([0x3c, 0x00]); // cmp al, 0
    jcc_done(&mut bytes, 0x84); // a NUL always terminates (EOF sentinel)
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15 (accept the byte)
    bytes.extend([0x49, 0x81, 0xff]); // cmp r15, imm32
    bytes.extend(capacity.to_le_bytes());
    bytes.extend([0x0f, 0x82]); // jb rel32 -> loop_start (keep reading while < capacity)
    let loop_jmp_disp = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    {
        let rel = loop_start as isize - (loop_jmp_disp as isize + 4);
        bytes[loop_jmp_disp..loop_jmp_disp + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }

    // done:
    let done = bytes.len();
    for fixup in done_fixups {
        let rel = done as isize - (fixup as isize + 4);
        bytes[fixup..fixup + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }
    let target_mov_offset = if target == RuntimeTextReadTarget::BoundedByteCarrier {
        // Owned carrier: the bytes are already in place; write only the length at
        // `[r14 - 8]` (the leading len word). No `{ptr, len}` descriptor.
        bytes.extend([0x4d, 0x89, 0x7e, 0xf8]); // mov [r14-8], r15
        0
    } else if target == RuntimeTextReadTarget::RawFixedArray {
        0
    } else {
        // mov r13, imm64(target) (relocated at +2); store the descriptor.
        let target_mov_offset = bytes.len();
        bytes.extend([0x49, 0xbd]);
        bytes.extend(0u64.to_le_bytes());
        bytes.extend([0x4d, 0x89, 0xb5]); // mov [r13+target_offset], r14 (descriptor.ptr)
        bytes.extend(target_ptr_disp.to_le_bytes());
        bytes.extend([0x4d, 0x89, 0xbd]); // mov [r13+target_offset+8], r15 (descriptor.len)
        bytes.extend(target_len_disp.to_le_bytes());
        target_mov_offset
    };

    Ok((bytes, target_mov_offset))
}

fn runtime_text_line_read_syscall_layout_for(target: RuntimeTextReadTarget) -> (usize, usize) {
    // Capacity/number/target are all fixed-width immediates, so they do not affect the
    // layout; encode once with placeholders to recover the width + target imm offset.
    let (bytes, target_mov_offset) = build_runtime_text_line_read_syscall(0, 1, 0, target)
        .expect("runtime text line read syscall layout encodes");
    (bytes.len(), target_mov_offset)
}

fn runtime_text_line_read_syscall_layout() -> (usize, usize) {
    runtime_text_line_read_syscall_layout_for(RuntimeTextReadTarget::StringDescriptor)
}

pub fn runtime_text_line_read_syscall_width() -> usize {
    runtime_text_line_read_syscall_layout().0
}

pub fn runtime_text_line_read_syscall_target_imm_offset() -> usize {
    runtime_text_line_read_syscall_layout().1
}

/// Owned carrier syscall read: wider prologue (`add r14`), shorter epilogue (a
/// single `len` store), so its width differs from the String descriptor path.
pub fn runtime_text_line_read_syscall_carrier_width() -> usize {
    runtime_text_line_read_syscall_layout_for(RuntimeTextReadTarget::BoundedByteCarrier).0
}

pub fn runtime_text_line_read_syscall_fixed_array_width() -> usize {
    runtime_text_line_read_syscall_layout_for(RuntimeTextReadTarget::RawFixedArray).0
}

pub fn encode_runtime_text_line_read(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read(
        target_offset,
        capacity,
        RuntimeTextReadTarget::StringDescriptor,
    )?
    .0)
}

/// Read a stdin line into an owned `[u8; N]` carrier: stdin bytes land directly in
/// the carrier's inline storage (`region + target_offset + pointer_size`) and the
/// line length is written to the carrier's leading length word (`target_offset`).
pub fn encode_runtime_text_line_read_carrier(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read(
        target_offset,
        capacity,
        RuntimeTextReadTarget::BoundedByteCarrier,
    )?
    .0)
}

pub fn encode_runtime_text_line_read_fixed_array(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read(
        target_offset,
        capacity,
        RuntimeTextReadTarget::RawFixedArray,
    )?
    .0)
}

// Append a source carrier's content onto a target carrier (concat builder source
// segment, after the first literal initialized the target). r15 = machine
// storage base (reloc @ +2). rax = target running len; rcx = source len (rep
// count); rsi = source bytes (source + 8); rdi = target bytes + running len; copy
// rcx bytes; store new len = target_len + source_len. Fixed width (no per-byte
// loop), one relocation (the base).
pub fn runtime_machine_bounded_buffer_source_append_width(source_in_frame: bool) -> usize {
    // mov r15,imm64 (10) + mov rax,[r15+t] (7) + mov rcx,[base+s] (7)
    // + lea rsi,[base+s+8] (7) + lea rdi,[r15+t+8] (7) + add rdi,rax (3)
    // + rep movsb (2) + add rax,rcx (3) + mov [r15+t],rax (7) = 53.
    // A frame-local source adds `mov r14, imm64(frame)` (10) for the source base.
    if source_in_frame { 63 } else { 53 }
}

pub fn encode_runtime_machine_bounded_buffer_source_append(
    target_byte_offset: usize,
    source_byte_offset: usize,
    source_in_frame: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let target = disp32(target_byte_offset)?;
    let target_bytes = disp32(target_byte_offset + 8)?;
    let source = disp32(source_byte_offset)?;
    let source_bytes = disp32(source_byte_offset + 8)?;
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_source_append_width(
        source_in_frame,
    ));
    append_mov_r15_imm64(&mut bytes, 0); // machine storage base (target; reloc @ +2)
    // The source carrier is read off r15 (machine) by default; a `let`-local source
    // loads the runtime frame base into r14 (a second relocation @ +12) and reads
    // from there. The two source instructions differ only in their base register.
    let (source_len_modrm, source_bytes_modrm) = if source_in_frame {
        append_mov_r14_imm64(&mut bytes, 0); // frame base (reloc @ +12)
        (0x8eu8, 0xb6u8) // mov rcx,[r14+s] ; lea rsi,[r14+s+8]
    } else {
        (0x8fu8, 0xb7u8) // mov rcx,[r15+s] ; lea rsi,[r15+s+8]
    };
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15 + target]   (target running len)
    bytes.extend(target.to_le_bytes());
    bytes.extend([0x49, 0x8b, source_len_modrm]); // mov rcx, [base + source] (source len)
    bytes.extend(source.to_le_bytes());
    bytes.extend([0x49, 0x8d, 0xbf]); // lea rdi, [r15 + target+8] (target bytes base)
    bytes.extend(target_bytes.to_le_bytes());
    bytes.extend([0x48, 0x01, 0xc7]); // add rdi, rax  (target bytes + running len)
    // new len = target_len + source_len -- MUST precede `rep movsb`, which
    // decrements rcx to 0 as it copies; computing it after would always add 0.
    bytes.extend([0x48, 0x01, 0xc8]); // add rax, rcx  (rax = target_len + source_len)
    bytes.extend([0x49, 0x8d, source_bytes_modrm]); // lea rsi, [base + source+8] (source bytes)
    bytes.extend(source_bytes.to_le_bytes());
    bytes.extend([0xf3, 0xa4]); // rep movsb  (copy rcx bytes; consumes rcx)
    bytes.extend([0x49, 0x89, 0x87]); // mov [r15 + target], rax  (store new len)
    bytes.extend(target.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_source_append_width(source_in_frame)
    );
    Ok(bytes)
}

// Append a string LITERAL onto a target carrier at its running length (a later
// concat segment, e.g. the trailing `" =="`). r15 = machine storage base (reloc
// @ +2). rax = target running len; rdi = target bytes + running len; the literal
// bytes are written as immediates at `[rdi + i]`; store new len = old + lit.len.
// One relocation (the base); fixed width (no per-byte loop -- the bytes are
// unrolled immediate stores).
pub fn runtime_machine_bounded_buffer_literal_append_width(literal: &[u8]) -> usize {
    // mov r15,imm64 (10) + mov rax,[r15+t] (7) + lea rdi,[r15+t+8] (7)
    // + add rdi,rax (3) + per byte: mov byte [rdi+disp8],imm8 (4)
    // + add rax,imm32 (`48 05`+imm32 = 6) + mov [r15+t],rax (7) = 40 + 4*len
    40 + 4 * literal.len()
}

pub fn encode_runtime_machine_bounded_buffer_literal_append(
    target_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let target = disp32(target_byte_offset)?;
    let target_bytes = disp32(target_byte_offset + 8)?;
    let literal_bytes = literal;
    let literal_len = u32::try_from(literal_bytes.len()).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 encoder cannot append a carrier literal of {} bytes",
            literal_bytes.len()
        ))
    })?;
    let mut bytes =
        Vec::with_capacity(runtime_machine_bounded_buffer_literal_append_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // machine storage base (reloc @ +2)
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15 + target]   (target running len)
    bytes.extend(target.to_le_bytes());
    bytes.extend([0x49, 0x8d, 0xbf]); // lea rdi, [r15 + target+8] (target bytes base)
    bytes.extend(target_bytes.to_le_bytes());
    bytes.extend([0x48, 0x01, 0xc7]); // add rdi, rax  (dest = target bytes + running len)
    for (index, byte) in literal_bytes.iter().enumerate() {
        let disp = u8::try_from(index).map_err(|_| {
            Diagnostic::error(
                "X86_64 encoder cannot append a carrier literal longer than 127 bytes".to_string(),
            )
        })?;
        bytes.extend([0xc6, 0x47, disp, *byte]); // mov byte [rdi + disp8], imm8
    }
    bytes.extend([0x48, 0x05]); // add rax, imm32  (new len = old + literal length)
    bytes.extend(literal_len.to_le_bytes());
    bytes.extend([0x49, 0x89, 0x87]); // mov [r15 + target], rax  (store new len)
    bytes.extend(target.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_literal_append_width(literal)
    );
    Ok(bytes)
}
