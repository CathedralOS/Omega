use omega_calling_conventions::{HostCapability, HostOperation, HostOperationKey};
use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{
    InstructionOperandLike, RuntimeValueOperandHandle, RuntimeValueOperandSource,
    StateGuardOperator,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64RelocationSiteKind {
    Absolute64,
    Relative32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64RelocationSite {
    pub operand_index: Option<usize>,
    pub byte_offset: usize,
    pub byte_width: usize,
    pub kind: X86_64RelocationSiteKind,
}

pub fn return_width() -> usize {
    1
}

pub fn encode_return_bytes() -> [u8; 1] {
    [0xc3]
}

pub fn return_register_integer_write_width() -> usize {
    5
}

pub fn runtime_storage_copy_to_return_register_width(byte_offset: usize, byte_size: usize) -> usize {
    // mov r15,imm64(region base, relocated) (10) + load into eax/rax (7; the
    // sign-extending movsx forms for 1/2-byte operands carry an 0F prefix, 8).
    let _ = byte_offset;
    let load_width = if matches!(byte_size, 1 | 2) { 8 } else { 7 };
    10 + load_width
}

pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_width() -> usize {
    // mov r14,imm64(frame) (10) + mov r11,[r14+idx] (7) + imul r11,r11,elem (7)
    // + add r14,r11 (3) + add r14,base+field (7) + mov r15,imm64(frame) (10)
    // + mov [r15+target],r14 (7)
    51
}

/// Relocation imm offset (pre-`+2`) of the frame base loaded for the target slot
/// store in `encode_runtime_frame_base_indexed_address_to_runtime_frame_write`.
pub const FRAME_BASE_INDEXED_ADDRESS_TARGET_FRAME_IMM_OFFSET: usize = 34;

pub fn runtime_pointee_string_write_width(_field_byte_offset: usize, _byte_length: usize) -> usize {
    // mov r14,imm64(literal) (10) + mov r15,imm64(frame) (10)
    // + mov r15,[r15+ptr] (7) + mov [r15+field],r14 (7)
    // + mov r14,len (10) + mov [r15+field+8],r14 (7)
    51
}

/// Writes a `{ptr,len}` string descriptor through a pointer stored in the frame:
/// `*(frame[pointer_byte_offset]) + field_byte_offset = { literal, byte_length }`.
/// The literal `mov` is emitted first so its relocation lands at the instruction
/// start (matching the shared relocation contract); the frame base relocation
/// follows at offset 10.
pub fn encode_runtime_pointee_string_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_string_write_width(
        field_byte_offset,
        byte_length,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // string literal pointer (reloc @ +2)
    append_mov_r15_imm64(&mut bytes, 0); // frame base (reloc @ +12)
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?; // r15 = stored pointer
    append_store_r14_to_r15(&mut bytes, field_byte_offset)?; // descriptor.ptr = literal
    append_mov_r14_imm64(&mut bytes, byte_length as u64);
    append_store_r14_to_r15(&mut bytes, field_byte_offset + 8)?; // descriptor.len
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_string_write_width(field_byte_offset, byte_length)
    );
    Ok(bytes)
}

pub const RUNTIME_TEXT_STORED_PLACE_APPEND_TARGET_IMM_OFFSET: usize = 10;
pub const RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET: usize = 33;
/// Like the non-pointee source offset, but the pointee variant inserts one extra
/// `mov r15, [r15+disp32]` (7 bytes) to dereference the runtime pointer before the
/// source-region `mov rcx, imm64`, pushing the source immediate from 33 to 40.
pub const RUNTIME_TEXT_STORED_PLACE_APPEND_POINTEE_SOURCE_IMM_OFFSET: usize = 40;

pub fn runtime_text_stored_place_append_width() -> usize {
    86
}

/// Appends a stored source string (a `{ptr,len}` descriptor in `source_region`)
/// to the end of a target string that lives in a fixed output `buffer`, updating
/// the target descriptor. r14=buffer base, r15=target region base, the source
/// region base is loaded into rcx. The copy itself is a `rep movsb` (rsi/rdi are
/// preserved around it). `buffer_offset` is unused (the append point is the
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
    append_push_rsi_rdi(&mut bytes);
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_pop_rdi_rsi(&mut bytes);
    debug_assert_eq!(bytes.len(), runtime_text_stored_place_append_width());
    Ok(bytes)
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width() -> usize {
    93
}

/// Appends a stored source string to a target string whose `{ptr,len}` descriptor
/// is reached through a RUNTIME pointer: the descriptor lives at
/// `*(frame + pointer_byte_offset) + field_byte_offset`. Mirrors
/// `encode_runtime_text_stored_place_append`, but loads the descriptor base by
/// dereferencing the runtime pointer (one extra `mov r15,[r15+disp32]`) instead of
/// using a relocated target-region base. r14=materialized buffer base, r15=descriptor
/// address, rcx=source region base; the copy is a `rep movsb` (rsi/rdi preserved).
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
    append_push_rsi_rdi(&mut bytes);
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_pop_rdi_rsi(&mut bytes);
    debug_assert_eq!(
        bytes.len(),
        runtime_text_stored_place_append_to_runtime_pointee_width()
    );
    Ok(bytes)
}

pub const RUNTIME_TEXT_STORED_SUFFIX_APPEND_SOURCE_IMM_OFFSET: usize = 10;
pub const RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET: usize = 59;

pub fn runtime_text_stored_suffix_append_width() -> usize {
    90
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
    append_push_rsi_rdi(&mut bytes);
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_pop_rdi_rsi(&mut bytes);
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

pub fn runtime_frame_fixed_indexed_address_to_runtime_frame_write_width() -> usize {
    // mov r14,imm64(frame) (10) + mov r15,[r14+desc] (7) + add r15,const (7)
    // + mov [r14+target],r15 (7)
    31
}

/// Computes the address of a descriptor-based element at a constant index
/// (`*(frame[descriptor]) + element_index*elem + field`) and stores that pointer
/// into the runtime-frame slot at `target_offset`. The frame base is loaded once
/// (r14) and reused for the store, so a single `mov r14,imm64` relocation suffices.
/// Computes the address of a field reached through a RUNTIME pointer stored in the
/// frame (`*(frame[pointer_byte_offset]) + field_byte_offset`) and stores that
/// pointer into the runtime-frame slot at `target_offset` -- the lowering of
/// `ptr.field.as_[mut_]slice()`'s descriptor pointer where `ptr` is a `&mut`
/// reference parameter. Same shape as the fixed-indexed-address write but the
/// displacement is just the field offset (no element index*size).
pub fn runtime_pointee_address_to_runtime_frame_write_width() -> usize {
    // mov r14,imm64(frame) (10) + mov r15,[r14+ptr] (7) + add r15,const (7)
    // + mov [r14+target],r15 (7)
    31
}

pub fn encode_runtime_pointee_address_to_runtime_frame_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_address_to_runtime_frame_write_width());
    append_mov_r14_imm64(&mut bytes, 0); // frame base (reloc @ +2)
    append_load_r15_from_r14(&mut bytes, pointer_byte_offset)?; // r15 = runtime pointer value
    append_add_r15_imm32(&mut bytes, field_byte_offset)?; // r15 = pointer + field
    append_store_r15_to_r14(&mut bytes, target_offset)?; // frame[target] = address
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_address_to_runtime_frame_write_width()
    );
    Ok(bytes)
}

pub fn encode_runtime_frame_fixed_indexed_address_to_runtime_frame_write(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes =
        Vec::with_capacity(runtime_frame_fixed_indexed_address_to_runtime_frame_write_width());
    append_mov_r14_imm64(&mut bytes, 0); // frame base (reloc @ +2)
    append_load_r15_from_r14(&mut bytes, descriptor_offset)?; // r15 = data pointer
    let displacement = element_index
        .checked_mul(element_byte_size)
        .and_then(|scaled| scaled.checked_add(field_byte_offset))
        .ok_or_else(|| Diagnostic::error("X86_64 fixed indexed address offset overflow"))?;
    append_add_r15_imm32(&mut bytes, displacement)?; // r15 = element address
    append_store_r15_to_r14(&mut bytes, target_offset)?; // frame[target] = address
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_fixed_indexed_address_to_runtime_frame_write_width()
    );
    Ok(bytes)
}

/// Computes the address of an inline frame-base-indexed element
/// (`frame + base + index*elem + field`) and stores that pointer into the
/// runtime-frame slot at `target_offset` -- the lowering of `arr.as_slice()` /
/// `as_mut_slice()` into a `{ptr,len}` descriptor's pointer field. The frame
/// base is loaded twice (source address in r14, target base in r15); both
/// `mov r*,imm64` immediates are relocated to the runtime-frame symbol.
pub fn encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes =
        Vec::with_capacity(runtime_frame_base_indexed_address_to_runtime_frame_write_width());
    append_mov_r14_imm64(&mut bytes, 0); // frame base (reloc @ +2)
    append_load_r11_from_r14(&mut bytes, index_offset)?; // r11 = index
    append_imul_r11_imm32(&mut bytes, element_scale(element_byte_size)?);
    append_add_r14_r11(&mut bytes); // r14 = frame + index*elem
    append_add_r14_imm32(&mut bytes, base_byte_offset + field_byte_offset)?; // + base + field
    debug_assert_eq!(
        bytes.len(),
        FRAME_BASE_INDEXED_ADDRESS_TARGET_FRAME_IMM_OFFSET
    );
    append_mov_r15_imm64(&mut bytes, 0); // frame base for the target slot (reloc @ +2)
    append_store_r14_to_r15(&mut bytes, target_offset)?; // frame[target] = element address
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_base_indexed_address_to_runtime_frame_write_width()
    );
    Ok(bytes)
}

pub fn encode_return_register_integer_write_bytes(
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot write {byte_size}-byte return integers yet"
        )));
    }
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot write return integer `{value}` yet"
        ))
    })?;
    let mut bytes = Vec::with_capacity(return_register_integer_write_width());
    bytes.push(0xb8); // mov eax, imm32
    bytes.extend(value.to_le_bytes());
    Ok(bytes)
}

/// Load a runtime-storage scalar into the return register (eax/rax) so a
/// NON-CONSTANT terminal value (a local read, a field read-back) becomes the
/// process exit code. The `mov r15, imm64=0` (imm at instruction start + 2) is
/// relocated to the storage region's data symbol by the relocation planner,
/// exactly like a dispatch guard's storage load. Narrow operands use the
/// sign-extending movsx forms so a negative i8/i16 terminal survives the
/// widening read.
pub fn encode_runtime_storage_copy_to_return_register_bytes(
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_to_return_register_width(
        byte_offset,
        byte_size,
    ));
    append_mov_r15_imm64(&mut bytes, 0);
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x0f, 0xbe, 0x87]), // movsx eax, byte [r15 + disp32]
        2 => bytes.extend([0x41, 0x0f, 0xbf, 0x87]), // movsx eax, word [r15 + disp32]
        4 => bytes.extend([0x41, 0x8b, 0x87]),       // mov eax, [r15 + disp32]
        8 => bytes.extend([0x49, 0x8b, 0x87]),       // mov rax, [r15 + disp32]
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot copy {byte_size}-byte terminal values to the return register yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_to_return_register_width(byte_offset, byte_size)
    );
    Ok(bytes)
}

pub fn dispatch_loop_enter_width() -> usize {
    6
}

pub fn dispatch_case_enter_width() -> usize {
    13
}

pub fn dispatch_state_write_width() -> usize {
    11
}

pub fn dispatch_case_leave_width() -> usize {
    5
}

pub fn encode_dispatch_loop_enter_bytes(entry_dispatch_index: u32) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_loop_enter_width());
    append_mov_r12d_imm32(&mut bytes, entry_dispatch_index)?;
    Ok(bytes)
}

pub fn encode_dispatch_case_enter_bytes(
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_case_enter_width());
    append_cmp_r12d_imm32(&mut bytes, dispatch_index)?;
    append_jcc_rel32(&mut bytes, 0x85, skip_byte_distance - 9)?; // jne
    Ok(bytes)
}

pub fn encode_dispatch_state_write_bytes(
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_state_write_width());
    append_mov_r12d_imm32(&mut bytes, dispatch_index)?;
    append_jmp_rel32(&mut bytes, case_leave_byte_distance - 7)?;
    Ok(bytes)
}

pub fn encode_dispatch_case_leave_bytes(loop_byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_case_leave_width());
    append_jmp_rel32(&mut bytes, loop_byte_distance - 5)?;
    Ok(bytes)
}

pub fn dispatch_guard_compare_static_width(is_float: bool, byte_size: usize) -> usize {
    // mov r15, imm64 (10) + load r10, [r15+disp32] (7; 8 for the 0x66-prefixed
    // 2-byte form) + mov r11, imm64 (10) + compare + jcc rel32 (6). Integer
    // compare is `cmp r10,r11` (3; 4 with the 0x66 prefix); float is
    // movq/movd + movq/movd + ucomisd/ucomiss.
    let load_width = if !is_float && byte_size == 2 { 8 } else { 7 };
    10 + load_width + 10 + runtime_float_or_integer_compare_width(is_float, byte_size) + 6
}

fn runtime_float_or_integer_compare_width(is_float: bool, byte_size: usize) -> usize {
    if is_float {
        // f64: movq(5)+movq(5)+ucomisd(4). f32: movd(5)+movd(5)+ucomiss(3) — the
        // single-precision SSE compare drops the 0x66 prefix, so it is 1 byte shorter.
        if byte_size == 4 { 13 } else { 14 }
    } else if byte_size == 2 {
        // 16-bit `cmp r10w,r11w` carries the 0x66 operand-size prefix.
        4
    } else {
        3
    }
}

/// Compare the bits already in r10 (left) and r11 (right) as `byte_size`-wide IEEE
/// floats via the SSE unit. For an 8-byte operand: `movq` into xmm0/xmm1 + `ucomisd`
/// (double precision). For a 4-byte operand: `movd` the low dword + `ucomiss` (single
/// precision). `ucomis*` sets CF/ZF exactly like an unsigned integer `cmp` (and PF on
/// unordered/NaN, which the unsigned failure branches ignore — a documented first-cut
/// limitation), so the same unsigned/equal failure-jcc conditions apply.
fn append_float_compare_r10_r11(bytes: &mut Vec<u8>, byte_size: usize) {
    if byte_size == 4 {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xcb]); // movd xmm1, r11d
        bytes.extend([0x0f, 0x2e, 0xc1]); // ucomiss xmm0, xmm1
    } else {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xcb]); // movq xmm1, r11
        bytes.extend([0x66, 0x0f, 0x2e, 0xc1]); // ucomisd xmm0, xmm1
    }
}

/// Narrow a guard's float `expected_value` (stored as f64 bits) to the operand's
/// width: for a 4-byte float operand the comparison runs in single precision, so the
/// immediate must be the f32 bit pattern. Exact for any value representable in f32
/// (which a constant compared against an f32 field always is).
fn float_compare_expected_bits(expected_value: i64, byte_size: usize) -> u64 {
    if byte_size == 4 {
        u64::from((f64::from_bits(expected_value as u64) as f32).to_bits())
    } else {
        expected_value as u64
    }
}

pub fn encode_dispatch_guard_compare_static_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
        )));
    }
    let mut bytes = Vec::with_capacity(dispatch_guard_compare_static_width(is_float, byte_size));
    // Storage base; the imm64 (at instruction start + 2) is relocated to the
    // guard's storage-region data symbol by the relocation planner.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_reg_from_r15(&mut bytes, Reg64::R10, byte_offset, byte_size)?;
    let expected_bits = if is_float {
        float_compare_expected_bits(expected_value, byte_size)
    } else {
        expected_value as u64
    };
    append_mov_reg_imm64(&mut bytes, Reg64::R11, expected_bits);
    if is_float {
        append_float_compare_r10_r11(&mut bytes, byte_size);
    } else {
        append_cmp_r10_r11(&mut bytes, byte_size)?;
    }
    // `skip_byte_distance` is anchored at the instruction's rel32 field start
    // (`current.offset + byte_width - 4`, now architecture-aware in the branch-
    // distance helper). The jcc rel is measured from the field's end, 4 bytes
    // later, so the relative target is `skip_byte_distance - 4`.
    append_failure_branch(&mut bytes, operator, skip_byte_distance - 4)?;
    debug_assert_eq!(bytes.len(), dispatch_guard_compare_static_width(is_float, byte_size));
    Ok(bytes)
}

pub fn host_call_sequence_width<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> usize {
    encode_host_call_sequence(operation_key, operands)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

pub fn host_call_data_relocation_site<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    operand_index: usize,
) -> Option<X86_64RelocationSite> {
    host_call_relocation_sites(operation_key, operands)
        .into_iter()
        .find(|site| {
            site.operand_index == Some(operand_index)
                && site.kind == X86_64RelocationSiteKind::Absolute64
        })
}

/// A `mov <arg-reg>, imm64` is 10 bytes (2-byte REX.W+B8 prefix, then the imm64), and
/// for both an immediate/data-address argument (`mov arg, imm64`) and a runtime-storage
/// argument (whose first instruction is `mov r15, imm64=0` for the relocated region base)
/// the relocated imm64 sits at the argument's start + 2.
pub const SYSCALL_ARG_MOV_WIDTH: usize = 10;

/// Byte width of marshalling a single syscall argument into its register. Simple
/// arguments (immediate, byte-length, data-address) are a direct `mov arg, imm64`;
/// runtime-storage arguments stage the value through r15/rax (see `encode_syscall_sequence`).
fn syscall_arg_operand_width<T: InstructionOperandLike>(operand: &T) -> usize {
    if operand.runtime_pointee_string_pointer().is_some()
        || operand.runtime_pointee_string_length().is_some()
    {
        // mov r15,imm64 (10) + mov r15,[r15+off] (7) + mov rax,[r15+disp] (7) + mov arg,rax (3)
        SYSCALL_ARG_MOV_WIDTH + 7 + 7 + 3
    } else if operand.runtime_string_pointer().is_some()
        || operand.runtime_string_length().is_some()
        || operand.runtime_scalar_integer().is_some()
    {
        // mov r15,imm64 (10) + mov rax,[r15+disp] (7) + mov arg,rax (3)
        SYSCALL_ARG_MOV_WIDTH + 7 + 3
    } else {
        // mov arg,imm64
        SYSCALL_ARG_MOV_WIDTH
    }
}

/// Byte offset (within the syscall sequence) of the relocated imm64 for the argument at
/// `operand_index`: the sum of the widths of all preceding arguments, plus the 2-byte
/// prefix before the imm64. Applies to both data-address and runtime-storage arguments,
/// whose relocated `mov`/`mov r15` is always the argument's first instruction.
pub fn syscall_data_relocation_byte_offset<T: InstructionOperandLike>(
    operands: &[T],
    operand_index: usize,
) -> usize {
    operands
        .iter()
        .take(operand_index)
        .map(syscall_arg_operand_width)
        .sum::<usize>()
        + 2
}

/// Total byte width of a Linux syscall sequence: each argument's marshalling, plus
/// `mov rax, imm64` (the syscall number) and the 2-byte `syscall`.
pub fn syscall_sequence_width<T: InstructionOperandLike>(operands: &[T]) -> usize {
    operands.iter().map(syscall_arg_operand_width).sum::<usize>() + SYSCALL_ARG_MOV_WIDTH + 2
}

/// x86_64 Linux (System V) syscall sequence: marshal each argument into the syscall
/// argument registers in order (RDI, RSI, RDX, R10, R8, R9), load the syscall number
/// into RAX, then `syscall` (0F 05).
///
/// Simple arguments emit a direct `mov arg, imm64` (data-address arguments use imm64=0
/// fixed up by an Absolute64 relocation). Runtime-storage arguments (a String descriptor
/// in a statically-allocated frame/machine/data region) stage through r15 and rax: load
/// the relocated region base into r15, read the pointer/length field (descriptor layout:
/// pointer at +0, length at +8) into rax, then `mov arg, rax`. r15 is used as the scratch
/// base because it is neither a syscall argument register nor clobbered by `syscall`.
pub fn encode_syscall_sequence<T: InstructionOperandLike>(
    operands: &[T],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(syscall_sequence_width(operands));
    for (index, operand) in operands.iter().enumerate() {
        if let Some((_, byte_offset)) = operand.runtime_pointee_string_pointer() {
            append_mov_r15_imm64(&mut bytes, 0); // relocated region base
            append_load_r15_from_r15(&mut bytes, byte_offset)?; // r15 = &descriptor
            append_load_rax_from_r15(&mut bytes, 0)?; // rax = descriptor.pointer
            append_mov_syscall_arg_from_rax(&mut bytes, index)?;
        } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_length() {
            append_mov_r15_imm64(&mut bytes, 0);
            append_load_r15_from_r15(&mut bytes, byte_offset)?;
            append_load_rax_from_r15(&mut bytes, 8)?; // rax = descriptor.length
            append_mov_syscall_arg_from_rax(&mut bytes, index)?;
        } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
            append_mov_r15_imm64(&mut bytes, 0);
            append_load_rax_from_r15(&mut bytes, byte_offset)?; // rax = descriptor.pointer
            append_mov_syscall_arg_from_rax(&mut bytes, index)?;
        } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
            append_mov_r15_imm64(&mut bytes, 0);
            append_load_rax_from_r15(&mut bytes, byte_offset + 8)?; // rax = descriptor.length
            append_mov_syscall_arg_from_rax(&mut bytes, index)?;
        } else if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
            append_mov_r15_imm64(&mut bytes, 0); // relocated region base
            append_load_rax_from_r15(&mut bytes, byte_offset)?; // rax = scalar value
            append_mov_syscall_arg_from_rax(&mut bytes, index)?;
        } else {
            let opcode = syscall_arg_mov_imm64_opcode(index)?;
            let value = if let Some(value) = operand.immediate_integer() {
                value as u64
            } else if let Some(value) = operand.byte_length() {
                value as u64
            } else if operand.data_address().is_some() {
                0 // relocated to the data symbol's address
            } else {
                return Err(Diagnostic::error(
                    "X86_64 syscall encoder cannot marshal this argument yet (expected \
                     immediate, byte-length, data-address, or runtime-storage)",
                ));
            };
            bytes.extend(opcode);
            bytes.extend(value.to_le_bytes());
        }
    }
    append_mov_rax_imm64(&mut bytes, u64::from(syscall_number));
    bytes.extend([0x0f, 0x05]); // syscall
    debug_assert_eq!(bytes.len(), syscall_sequence_width(operands));
    Ok(bytes)
}

/// The `mov <syscall-arg-reg>, imm64` opcode prefix (REX.W + B8+rd) for syscall
/// argument `index`, mapping to RDI, RSI, RDX, R10, R8, R9 in order.
fn syscall_arg_mov_imm64_opcode(index: usize) -> Result<[u8; 2], Diagnostic> {
    Ok(match index {
        0 => [0x48, 0xbf], // mov rdi, imm64
        1 => [0x48, 0xbe], // mov rsi, imm64
        2 => [0x48, 0xba], // mov rdx, imm64
        3 => [0x49, 0xba], // mov r10, imm64
        4 => [0x49, 0xb8], // mov r8,  imm64
        5 => [0x49, 0xb9], // mov r9,  imm64
        _ => {
            return Err(Diagnostic::error(
                "X86_64 syscall encoder supports at most 6 arguments",
            ));
        }
    })
}

/// `mov <syscall-arg-reg>, rax` (opcode 89 /r, source rax = reg field 0), for staging a
/// runtime-storage value computed in rax into syscall argument `index`'s register.
fn append_mov_syscall_arg_from_rax(bytes: &mut Vec<u8>, index: usize) -> Result<(), Diagnostic> {
    bytes.extend(match index {
        0 => [0x48, 0x89, 0xc7], // mov rdi, rax
        1 => [0x48, 0x89, 0xc6], // mov rsi, rax
        2 => [0x48, 0x89, 0xc2], // mov rdx, rax
        3 => [0x49, 0x89, 0xc2], // mov r10, rax
        4 => [0x49, 0x89, 0xc0], // mov r8,  rax
        5 => [0x49, 0x89, 0xc1], // mov r9,  rax
        _ => {
            return Err(Diagnostic::error(
                "X86_64 syscall encoder supports at most 6 arguments",
            ));
        }
    });
    Ok(())
}

pub fn host_call_external_relocation_site<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Option<X86_64RelocationSite> {
    host_call_relocation_sites(operation_key, operands)
        .into_iter()
        .find(|site| site.kind == X86_64RelocationSiteKind::Relative32)
}

pub fn encode_host_call_sequence<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match (operation_key.capability, operation_key.operation) {
        (
            HostCapability::Stdin | HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::GetStdHandle,
        ) => encode_get_std_handle(operands),
        (
            HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::Write | HostOperation::WriteFile,
        ) => encode_file_operation(operation_key, operands),
        (HostCapability::Stdin, HostOperation::ReadFile) => {
            encode_file_operation(operation_key, operands)
        }
        (HostCapability::Process, HostOperation::ExitProcess) => encode_exit_process(operands),
        _ => Err(Diagnostic::error(format!(
            "X86_64 host operation {}.{} is not implemented",
            operation_key.capability_name(),
            operation_key.operation_name()
        ))),
    }
}

fn encode_get_std_handle<T: InstructionOperandLike>(operands: &[T]) -> Result<Vec<u8>, Diagnostic> {
    let handle_kind = immediate_i32(operands, 0, "GetStdHandle handle kind")?;
    let mut bytes = Vec::with_capacity(18);
    bytes.extend([0x48, 0x83, 0xec, 0x28]); // sub rsp, 40
    bytes.push(0xb9); // mov ecx, imm32
    bytes.extend(handle_kind.to_le_bytes());
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32
    bytes.extend([0x48, 0x83, 0xc4, 0x28]); // add rsp, 40
    Ok(bytes)
}

fn encode_file_operation<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    let (pointer_index, length_index) = file_pointer_and_length_indices(operands)?;
    if operands.len() <= length_index {
        return Err(Diagnostic::error(
            "cannot encode X86_64 file operation: missing pointer/length operands",
        ));
    }

    let mut bytes = Vec::new();
    bytes.extend([0x48, 0x83, 0xec, 0x38]); // sub rsp, 56
    if pointer_index == 1 {
        let handle = immediate_i32(operands, 0, "file handle")?;
        bytes.push(0xb9); // mov ecx, imm32
        bytes.extend(handle.to_le_bytes());
    } else {
        bytes.extend([0x48, 0x89, 0xc1]); // mov rcx, rax
    }
    append_file_pointer_operand(&mut bytes, &operands[pointer_index])?;
    if operation_key.capability == HostCapability::Stdin
        && operation_key.operation == HostOperation::ReadFile
    {
        bytes.extend([0xc6, 0x02, 0]); // mov byte ptr [rdx], 0
    }
    append_file_length_operand(&mut bytes, &operands[length_index])?;
    bytes.extend([0x4c, 0x8d, 0x4c, 0x24, 0x28]); // lea r9, [rsp + 40]
    bytes.extend([0x48, 0xc7, 0x44, 0x24, 0x20, 0, 0, 0, 0]); // qword [rsp+32] = 0
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32
    bytes.extend([0x48, 0x83, 0xc4, 0x38]); // add rsp, 56
    Ok(bytes)
}

fn append_file_pointer_operand<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
) -> Result<(), Diagnostic> {
    if operand.data_address().is_some() {
        append_mov_rdx_imm64(bytes, 0);
        Ok(())
    } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
        append_mov_r10_imm64(bytes, 0);
        append_load_rdx_from_r10(bytes, byte_offset)?;
        Ok(())
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_pointer() {
        append_mov_r10_imm64(bytes, 0);
        append_load_r10_from_r10(bytes, byte_offset)?;
        append_load_rdx_from_r10(bytes, 0)?;
        Ok(())
    } else {
        Err(Diagnostic::error(
            "cannot encode X86_64 file operation: pointer operand is unsupported",
        ))
    }
}

fn append_file_length_operand<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
) -> Result<(), Diagnostic> {
    if let Some(value) = operand.byte_length() {
        let value = u32::try_from(value).map_err(|_| {
            Diagnostic::error(format!(
                "cannot encode X86_64 file operation: byte length {value} does not fit u32"
            ))
        })?;
        bytes.extend([0x41, 0xb8]); // mov r8d, imm32
        bytes.extend(value.to_le_bytes());
        Ok(())
    } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
        append_mov_r10_imm64(bytes, 0);
        append_load_r8_from_r10(bytes, byte_offset + 8)?;
        Ok(())
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_length() {
        append_mov_r10_imm64(bytes, 0);
        append_load_r10_from_r10(bytes, byte_offset)?;
        append_load_r8_from_r10(bytes, 8)?;
        Ok(())
    } else {
        Err(Diagnostic::error(
            "cannot encode X86_64 file operation: length operand is unsupported",
        ))
    }
}

/// Marshalling width of the exit-code argument: a constant is `mov ecx, imm32` (5 bytes),
/// a runtime-storage scalar is `mov r15, imm64=0` (10, relocated to the region base) +
/// `mov rcx, [r15+disp32]` (7).
fn exit_process_exit_code_width<T: InstructionOperandLike>(operands: &[T]) -> usize {
    if operands
        .first()
        .is_some_and(|operand| operand.runtime_scalar_integer().is_some())
    {
        17
    } else {
        5
    }
}

fn encode_exit_process<T: InstructionOperandLike>(operands: &[T]) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(13 + exit_process_exit_code_width(operands));
    bytes.extend([0x48, 0x83, 0xec, 0x28]); // sub rsp, 40
    match operands.first() {
        Some(operand) if operand.runtime_scalar_integer().is_some() => {
            let (_, byte_offset, _) = operand.runtime_scalar_integer().unwrap();
            // mov r15, imm64=0 (relocated to the exit code's storage-region base), then
            // mov rcx, [r15 + byte_offset]. ExitProcess reads ecx (the low 32 bits).
            append_mov_r15_imm64(&mut bytes, 0);
            append_load_rcx_from_r15(&mut bytes, byte_offset)?;
        }
        _ => {
            let exit_code = immediate_i32(operands, 0, "ExitProcess exit code")?;
            bytes.push(0xb9); // mov ecx, imm32
            bytes.extend(exit_code.to_le_bytes());
        }
    }
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32
    bytes.extend([0x48, 0x83, 0xc4, 0x28]); // add rsp, 40
    debug_assert_eq!(bytes.len(), 13 + exit_process_exit_code_width(operands));
    Ok(bytes)
}

fn host_call_relocation_sites<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    match (operation_key.capability, operation_key.operation) {
        (
            HostCapability::Stdin | HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::GetStdHandle,
        ) => {
            vec![X86_64RelocationSite {
                operand_index: None,
                byte_offset: 10,
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            }]
        }
        (HostCapability::Process, HostOperation::ExitProcess) => {
            // Layout: sub rsp,40 (4) + exit-code marshalling + call rel32.
            let exit_code_width = exit_process_exit_code_width(operands);
            let mut sites = Vec::new();
            if operands
                .first()
                .is_some_and(|operand| operand.runtime_scalar_integer().is_some())
            {
                // The relocated region-base imm64 sits inside `mov r15, imm64` at
                // (sub rsp = 4) + 2.
                sites.push(X86_64RelocationSite {
                    operand_index: Some(0),
                    byte_offset: 4 + 2,
                    byte_width: 8,
                    kind: X86_64RelocationSiteKind::Absolute64,
                });
            }
            // `call rel32`: skip sub rsp (4), the exit-code marshalling, and the call
            // opcode (1).
            sites.push(X86_64RelocationSite {
                operand_index: None,
                byte_offset: 4 + exit_code_width + 1,
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            });
            sites
        }
        (
            HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::Write | HostOperation::WriteFile,
        )
        | (HostCapability::Stdin, HostOperation::ReadFile) => {
            let mut sites = Vec::new();
            let Ok((pointer_index, length_index)) = file_pointer_and_length_indices(operands)
            else {
                return sites;
            };
            let mut cursor = if pointer_index == 1 { 9 } else { 7 };

            if operands.get(pointer_index).is_some_and(|operand| {
                operand.data_address().is_some()
                    || operand.runtime_string_pointer().is_some()
                    || operand.runtime_pointee_string_pointer().is_some()
            }) {
                sites.push(X86_64RelocationSite {
                    operand_index: Some(pointer_index),
                    byte_offset: cursor + 2,
                    byte_width: 8,
                    kind: X86_64RelocationSiteKind::Absolute64,
                });
            }
            cursor += file_pointer_operand_width(operands.get(pointer_index));
            if operation_key.capability == HostCapability::Stdin
                && operation_key.operation == HostOperation::ReadFile
            {
                cursor += 3;
            }

            if operands.get(length_index).is_some_and(|operand| {
                operand.runtime_string_length().is_some()
                    || operand.runtime_pointee_string_length().is_some()
            }) {
                sites.push(X86_64RelocationSite {
                    operand_index: Some(length_index),
                    byte_offset: cursor + 2,
                    byte_width: 8,
                    kind: X86_64RelocationSiteKind::Absolute64,
                });
            }
            cursor += file_length_operand_width(operands.get(length_index));
            cursor += 15; // lea r9 + qword null + call opcode

            sites.push(X86_64RelocationSite {
                operand_index: None,
                byte_offset: cursor,
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            });
            sites
        }
        _ => Vec::new(),
    }
}

fn file_pointer_and_length_indices<T: InstructionOperandLike>(
    operands: &[T],
) -> Result<(usize, usize), Diagnostic> {
    match operands.first() {
        Some(operand) if operand.immediate_integer().is_some() => Ok((1, 2)),
        Some(operand)
            if operand.data_address().is_some()
                || operand.runtime_string_pointer().is_some()
                || operand.runtime_pointee_string_pointer().is_some() =>
        {
            Ok((0, 1))
        }
        _ => Err(Diagnostic::error(
            "cannot encode X86_64 file operation: unsupported operand shape",
        )),
    }
}

fn file_pointer_operand_width<T: InstructionOperandLike>(operand: Option<&T>) -> usize {
    match operand {
        Some(operand) if operand.data_address().is_some() => 10,
        Some(operand) if operand.runtime_string_pointer().is_some() => 17,
        Some(operand) if operand.runtime_pointee_string_pointer().is_some() => 24,
        _ => 0,
    }
}

fn file_length_operand_width<T: InstructionOperandLike>(operand: Option<&T>) -> usize {
    match operand {
        Some(operand) if operand.byte_length().is_some() => 6,
        Some(operand) if operand.runtime_string_length().is_some() => 17,
        Some(operand) if operand.runtime_pointee_string_length().is_some() => 24,
        _ => 0,
    }
}

pub fn runtime_text_literal_compare_width(literal: &str) -> usize {
    10 + literal.len() * 15 + 36
}

// Write a literal's bytes into a runtime text buffer at a fixed byte offset
// (the first segment of a concatenation). r15 = buffer (reloc @ +2); store each
// literal byte at [r15 + byte_offset + i].
pub fn runtime_text_literal_segment_write_width(literal: &str) -> usize {
    10 + literal.len() * 8
}

pub fn encode_runtime_text_literal_segment_write(
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_segment_write_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    for (i, byte) in literal.as_bytes().iter().enumerate() {
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

pub fn runtime_text_literal_append_width(literal: &str) -> usize {
    // mov r15,imm64 (10) + mov r14,imm64 (10) + mov rax,[r14+len] (7) = 27
    // + per byte: mov cl,imm8 (2) + mov [r15+rax],cl (4) + inc rax (3) = 9
    // + mov [r14+ptr],r15 (7) + mov [r14+len],rax (7) = 14
    27 + literal.len() * 9 + 14
}

pub fn encode_runtime_text_literal_append(
    target_offset: usize,
    literal: &str,
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
    for byte in literal.as_bytes() {
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

pub fn runtime_text_literal_append_to_runtime_pointee_width(literal: &str) -> usize {
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
    literal: &str,
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
    for byte in literal.as_bytes() {
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
    // + mov r11,rcx(3) + mov r10,r14(3) + push rsi;push rdi(2) + mov rsi,rax(3)
    // + mov rdi,r10(3) + rep movsb(2) + pop rdi;pop rsi(2) + store r14(7) + store r11(7)
    66
}

/// Materializes a fresh writable text buffer for an in-place concat: copies the
/// current `{ptr,len}` descriptor at `target_offset` (in the relocated target
/// region) into the relocated `buffer`, then repoints the descriptor at the
/// buffer (ptr=buffer, len unchanged). A later append then grows the copy in
/// place without disturbing the original literal/source the descriptor named.
pub fn encode_runtime_text_buffer_materialize(
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_buffer_materialize_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ instruction start)
    append_mov_r15_imm64(&mut bytes, 0); // target region base (reloc @ +10)
    append_load_rax_from_r15(&mut bytes, target_offset)?; // rax = source pointer
    append_load_rcx_from_r15(&mut bytes, target_offset + 8)?; // rcx = source length
    append_mov_r11_rcx(&mut bytes); // r11 = saved length
    append_mov_r10_r14(&mut bytes); // r10 = dest = buffer base
    append_push_rsi_rdi(&mut bytes);
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_pop_rdi_rsi(&mut bytes);
    append_store_r14_to_r15(&mut bytes, target_offset)?; // descriptor.ptr = buffer
    append_store_r11_to_r15(&mut bytes, target_offset + 8)?; // descriptor.len = original length
    debug_assert_eq!(bytes.len(), runtime_text_buffer_materialize_width());
    Ok(bytes)
}

pub fn runtime_text_literal_compare_branch_next_offset(byte_index: usize) -> usize {
    10 + byte_index * 15 + 15
}

pub fn encode_runtime_text_literal_compare(
    literal: &str,
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
    for (byte_index, (expected_byte, failure_branch_distance)) in literal
        .as_bytes()
        .iter()
        .zip(failure_branch_distances)
        .enumerate()
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
    _branch_when_equal: bool,
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

    // MISMATCH trampoline: fall through to the instruction end (the following
    // "write text_ok = 0"). `fail_fixups` are the internal mismatch branches; we
    // route them just before the terminal match jmp and then let them reach the
    // end via a short jmp.
    let mismatch = bytes.len();
    bytes.push(0xe9);
    let mismatch_jmp_at = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    for fixup in &fail_fixups {
        bytes[*fixup..*fixup + 4]
            .copy_from_slice(&((mismatch as isize - (*fixup as isize + 4)) as i32).to_le_bytes());
    }

    // MATCH trampoline: single jmp to the external "next guarded effect end"
    // distance, skipping the trailing "write 0". Its rel32 ends at the
    // instruction width, which is the offset emission anchors the distance to.
    let matched = bytes.len();
    for fixup in &success_fixups {
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
// encoders share a fixed 34-byte address-computation prefix that leaves the
// element address in rax:
//   mov r14,imm64(frame) (10, reloc@+2) ; mov rax,[r14+descriptor] (7)
//   mov r11,[r14+index] (7) ; imul r11,r11,elem (7) ; add rax,r11 (3)
// so the second relocated immediate (data/buffer) always sits at offset 36.
const FRAME_INDEXED_STRING_PREFIX_WIDTH: usize = 34;
pub const RUNTIME_FRAME_INDEXED_STRING_DATA_IMM_OFFSET: usize = FRAME_INDEXED_STRING_PREFIX_WIDTH;

fn append_frame_indexed_element_address_into_rax(
    bytes: &mut Vec<u8>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
) -> Result<(), Diagnostic> {
    append_mov_r14_imm64(bytes, 0); // frame base (reloc @ +2)
    append_load_rax_from_r14(bytes, descriptor_offset, 8)?; // rax = slice data ptr
    append_load_r11_from_r14(bytes, index_offset)?; // r11 = index
    append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
    append_add_rax_r11(bytes); // rax = element address
    debug_assert_eq!(bytes.len(), FRAME_INDEXED_STRING_PREFIX_WIDTH);
    Ok(())
}

pub fn runtime_frame_indexed_string_write_width(
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_length: usize,
) -> usize {
    // prefix (34) + mov r15,imm64 (10) + store r15 (7) + mov r11,imm64 (10) + store r11 (7)
    FRAME_INDEXED_STRING_PREFIX_WIDTH + 34
}

pub fn encode_runtime_frame_indexed_string_write(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_string_write_width(
        element_byte_size,
        field_byte_offset,
        byte_length,
    ));
    append_frame_indexed_element_address_into_rax(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
    )?;
    // r15 = string literal data ptr (reloc @ prefix+2); store {ptr, len}.
    append_mov_r15_imm64(&mut bytes, 0);
    append_store_r15_to_rax(&mut bytes, field_byte_offset)?;
    append_mov_reg_imm64(&mut bytes, Reg64::R11, byte_length as u64);
    append_store_r11_to_rax(&mut bytes, field_byte_offset + 8)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_indexed_string_write_width(element_byte_size, field_byte_offset, byte_length)
    );
    Ok(bytes)
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    _element_byte_size: usize,
    _field_byte_offset: usize,
    literal: &str,
) -> usize {
    // prefix (34) + mov r15,imm64 buffer (10) + mov r11,[rax+field+8] len (7)
    // + per byte: mov cl,imm8 (2) + mov [r15+r11],cl (4) + inc r11 (3) = 9
    // + store r15->[rax+field] ptr (7) + store r11->[rax+field+8] len (7)
    FRAME_INDEXED_STRING_PREFIX_WIDTH + 17 + literal.len() * 9 + 14
}

pub fn encode_runtime_text_literal_append_to_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_to_runtime_frame_indexed_width(
        element_byte_size,
        field_byte_offset,
        literal,
    ));
    append_frame_indexed_element_address_into_rax(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
    )?;
    // r15 = buffer (reloc @ prefix+2); r11 = current len from the indexed descriptor.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_r11_from_rax(&mut bytes, field_byte_offset + 8)?;
    // append bytes at buffer[len]; r11 advances per byte.
    for byte in literal.as_bytes() {
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

pub fn runtime_machine_indexed_string_write_width(
    _base_byte_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_length: usize,
) -> usize {
    // prefix (37) + mov r14,imm64 literal (10) + store r14 (7)
    // + mov r14,imm64 len (10) + store r14 (7)
    MACHINE_INDEXED_STRING_PREFIX_WIDTH + 34
}

pub fn encode_runtime_machine_indexed_string_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_indexed_string_write_width(
        base_byte_offset,
        element_byte_size,
        field_byte_offset,
        byte_length,
    ));
    // r15 = machine base (reloc @ +2); r14 = frame base (reloc @ +12).
    append_mov_r15_imm64(&mut bytes, 0);
    append_mov_r14_imm64(&mut bytes, 0);
    append_load_r11_from_r14(&mut bytes, index_offset)?; // r11 = index
    append_imul_r11_imm32(&mut bytes, element_scale(element_byte_size)?);
    append_add_r15_r11(&mut bytes); // r15 = machine_base + index*elem
    debug_assert_eq!(bytes.len(), MACHINE_INDEXED_STRING_PREFIX_WIDTH);
    let store_offset = base_byte_offset + field_byte_offset;
    // descriptor.ptr = string literal (r14, reloc @ prefix+2).
    append_mov_r14_imm64(&mut bytes, 0);
    append_store_r14_to_r15(&mut bytes, store_offset)?;
    // descriptor.len = byte_length.
    append_mov_r14_imm64(&mut bytes, byte_length as u64);
    append_store_r14_to_r15(&mut bytes, store_offset + 8)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_indexed_string_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_length
        )
    );
    Ok(bytes)
}

pub fn runtime_value_compare_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    // cmp (3; 4 with the 0x66 prefix at 2-byte width) + jcc rel32 (6).
    let compare_width = if byte_size == 2 { 4 } else { 3 };
    runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + compare_width
        + 6
}

pub fn encode_runtime_value_compare(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_value_compare_width(
        runtime_value_operands,
        byte_size,
        left,
        right,
    ));
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R11, right)?;
    append_cmp_r10_r11(&mut bytes, byte_size)?;
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_value_compare_width(runtime_value_operands, byte_size, left, right)
    );
    Ok(bytes)
}

pub fn runtime_storage_compare_width(
    _left_offset: usize,
    _right_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> usize {
    // mov r15,imm64(left base) + load r10,[r15+left] + mov r15,imm64(right base)
    // + load r11,[r15+right] + compare + jcc rel32. Integer compare = cmp (3;
    // 4 with the 0x66 prefix for 2-byte operands, whose loads are also 8 not 7);
    // float = movq/movd+movq/movd+ucomisd/ucomiss.
    let load_width = if !is_float && byte_size == 2 { 8 } else { 7 };
    10 + load_width
        + 10
        + load_width
        + runtime_float_or_integer_compare_width(is_float, byte_size)
        + 6
}

pub fn encode_runtime_storage_compare_bytes(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_compare_width(
        left_offset,
        right_offset,
        byte_size,
        is_float,
    ));
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_reg_from_r15(&mut bytes, Reg64::R10, left_offset, byte_size)?;
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_reg_from_r15(&mut bytes, Reg64::R11, right_offset, byte_size)?;
    if is_float {
        append_float_compare_r10_r11(&mut bytes, byte_size);
    } else {
        append_cmp_r10_r11(&mut bytes, byte_size)?;
    }
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_compare_width(left_offset, right_offset, byte_size, is_float)
    );
    Ok(bytes)
}

pub fn runtime_storage_value_compare_width(_byte_offset: usize, byte_size: usize) -> usize {
    // mov r15,imm64(storage base) + load r10,[r15+offset] + mov r11,imm64
    // + cmp r10,r11 + jcc rel32. 2-byte operands add the 0x66 prefix to the
    // load (8) and the compare (4).
    if byte_size == 2 {
        10 + 8 + 10 + 4 + 6
    } else {
        10 + 7 + 10 + 3 + 6
    }
}

pub fn encode_runtime_storage_value_compare_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_value_compare_width(byte_offset, byte_size));
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_reg_from_r15(&mut bytes, Reg64::R10, byte_offset, byte_size)?;
    append_mov_reg_imm64(&mut bytes, Reg64::R11, expected_value as u64);
    append_cmp_r10_r11(&mut bytes, byte_size)?;
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_value_compare_width(byte_offset, byte_size)
    );
    Ok(bytes)
}

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
    let mut bytes = Vec::with_capacity(runtime_machine_integer_write_width(byte_offset, byte_size));
    append_mov_r15_imm64(&mut bytes, 0);
    append_mov_rax_imm64(&mut bytes, value as u64);
    append_store_rax_to_r15(&mut bytes, byte_offset, byte_size)?;
    Ok(bytes)
}

pub fn runtime_machine_indexed_integer_write_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    _element_byte_size: usize,
    _byte_size: usize,
) -> usize {
    // mov r15,imm64 (10) [+ mov r10,imm64 (10) for RuntimeFrame index]
    // + mov rax,[base+index_off] (7) + imul rax,rax,imm32 (7)
    // + add r15,rax (3) + mov rax,imm64 (10) + store [r15+disp] (7).
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => 54,
        omega_target_operations::RuntimeStorageRegion::Machine => 44,
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
    let store_displacement = base_byte_offset + field_byte_offset;
    let mut bytes = Vec::with_capacity(runtime_machine_indexed_integer_write_width(
        index_region,
        element_byte_size,
        byte_size,
    ));
    // r15 = machine storage base (imm64 at +2 relocated to the machine symbol).
    append_mov_r15_imm64(&mut bytes, 0);
    // rax = index, loaded from the runtime frame or from machine storage.
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            // r10 = runtime-frame base (imm64 at +12 relocated to the frame symbol).
            append_mov_r10_imm64(&mut bytes, 0);
            append_load_rax_from_r10(&mut bytes, index_offset)?;
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            append_load_rax_from_r15(&mut bytes, index_offset)?;
        }
    }
    // rax = index * element_byte_size; r15 = machine base + scaled index.
    append_imul_rax_imm32(&mut bytes, element_scale);
    append_add_r15_rax(&mut bytes);
    // Store the value at [r15 + base + field]. rax is free again after the add.
    append_mov_rax_imm64(&mut bytes, value as u64);
    append_store_rax_to_r15(&mut bytes, store_displacement, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_indexed_integer_write_width(index_region, element_byte_size, byte_size)
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
    let mut bytes = Vec::with_capacity(runtime_pointee_integer_write_width(
        field_byte_offset,
        byte_size,
    ));
    // r15 = frame base (imm64 at +2 relocated to the frame symbol); then load the
    // stored pointer in place and store the value through it.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?;
    append_mov_rax_imm64(&mut bytes, value as u64);
    append_store_rax_to_r15(&mut bytes, field_byte_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_integer_write_width(field_byte_offset, byte_size)
    );
    Ok(bytes)
}

pub fn runtime_frame_indexed_integer_write_width(
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
) -> usize {
    // mov r14,imm64 (10) + mov r15,[r14+desc] (7) + mov r11,[r14+idx] (7)
    // + imul r11,r11,elem (7) + add r15,r11 (3) + mov rax,imm64 (10) + store [r15+field] (7)
    51
}

pub fn encode_runtime_frame_indexed_integer_write(
    descriptor_offset: usize,
    index_offset: usize,
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
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_integer_write_width(
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    // r14 = frame base (imm64 at +2 relocated to the frame symbol). The descriptor
    // holds the slice data pointer; r15 = data ptr + index*element.
    append_mov_r14_imm64(&mut bytes, 0);
    append_load_r15_from_r14(&mut bytes, descriptor_offset)?;
    append_load_r11_from_r14(&mut bytes, index_offset)?;
    append_imul_r11_imm32(&mut bytes, element_scale(element_byte_size)?);
    append_add_r15_r11(&mut bytes);
    append_mov_rax_imm64(&mut bytes, value as u64);
    append_store_rax_to_r15(&mut bytes, field_byte_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_indexed_integer_write_width(element_byte_size, field_byte_offset, byte_size)
    );
    Ok(bytes)
}

pub fn runtime_frame_base_indexed_integer_write_width(
    _base_byte_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
) -> usize {
    // mov r14,imm64 (10) + mov r11,[r14+idx] (7) + imul r11,r11,elem (7)
    // + mov r15,r14 (3) + add r15,r11 (3) + mov rax,imm64 (10) + store [r15+base+field] (7)
    47
}

pub fn encode_runtime_frame_base_indexed_integer_write(
    base_byte_offset: usize,
    index_offset: usize,
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
    let store_displacement = base_byte_offset + field_byte_offset;
    let mut bytes = Vec::with_capacity(runtime_frame_base_indexed_integer_write_width(
        base_byte_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    // r14 = frame base (imm64 at +2 relocated to the frame symbol). The array base
    // lives inline in the frame at base_byte_offset; r15 = frame base + index*element.
    append_mov_r14_imm64(&mut bytes, 0);
    append_load_r11_from_r14(&mut bytes, index_offset)?;
    append_imul_r11_imm32(&mut bytes, element_scale(element_byte_size)?);
    append_mov_r15_r14(&mut bytes);
    append_add_r15_r11(&mut bytes);
    append_mov_rax_imm64(&mut bytes, value as u64);
    append_store_rax_to_r15(&mut bytes, store_displacement, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_size
        )
    );
    Ok(bytes)
}

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

fn build_runtime_text_line_read(
    target_offset: usize,
    capacity: u32,
) -> Result<(Vec<u8>, RuntimeTextLineReadLayout), Diagnostic> {
    let target_ptr_disp = disp32(target_offset)?;
    let target_len_disp = disp32(target_offset + 8)?;
    let mut bytes = Vec::with_capacity(128);

    // r14 = buffer (imm64 at +2 relocated to the buffer data symbol).
    bytes.extend([0x49, 0xbe]);
    bytes.extend(0u64.to_le_bytes());
    // sub rsp, 56.
    bytes.extend([0x48, 0x83, 0xec, 0x38]);
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
    bytes.extend([0x4c, 0x8d, 0x4c, 0x24, 0x28]); // lea r9, [rsp+40]
    bytes.extend([0x48, 0xc7, 0x44, 0x24, 0x20, 0, 0, 0, 0]); // mov qword [rsp+32], 0
    bytes.push(0xe8); // call ReadFile (rel32)
    let read_file_call_offset = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x8b, 0x44, 0x24, 0x28]); // mov eax, [rsp+40]  (bytesRead)

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
    // add rsp, 56.
    bytes.extend([0x48, 0x83, 0xc4, 0x38]);
    // r12 = target descriptor base (imm64 relocated). Use r12? no -- r12 is the
    // dispatch-state register; use r13 (handle no longer needed).
    // mov r13, imm64(target). The relocation planner anchors at the instruction
    // start and adds the +2 immediate offset itself, so record the start.
    let target_mov_offset = bytes.len();
    bytes.extend([0x49, 0xbd]);
    bytes.extend(0u64.to_le_bytes());
    // mov [r13+target_offset], r14  (descriptor.ptr = buffer).
    bytes.extend([0x4d, 0x89, 0xb5]);
    bytes.extend(target_ptr_disp.to_le_bytes());
    // mov [r13+target_offset+8], r15 (descriptor.len = line length).
    bytes.extend([0x4d, 0x89, 0xbd]);
    bytes.extend(target_len_disp.to_le_bytes());

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

fn runtime_text_line_read_layout() -> RuntimeTextLineReadLayout {
    // Capacity/target do not affect the layout (all immediates are fixed width),
    // so encode once with placeholders to recover the authoritative offsets.
    build_runtime_text_line_read(0, 1)
        .expect("runtime text line read layout encodes")
        .1
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

/// x86_64 Linux line read via the `read(2)` syscall (no GetStdHandle/ReadFile imports).
/// Byte-at-a-time read from stdin (fd 0) into the relocated buffer (r14), tracking the
/// line length in r15, with the same CRLF/NUL terminator handling as the win32 import
/// path, then store the {pointer, length} String descriptor into the target region.
pub fn encode_runtime_text_line_read_syscall(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read_syscall(target_offset, capacity, number)?.0)
}

fn build_runtime_text_line_read_syscall(
    target_offset: usize,
    capacity: u32,
    number: u32,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    let target_ptr_disp = disp32(target_offset)?;
    let target_len_disp = disp32(target_offset + 8)?;
    let mut bytes = Vec::with_capacity(128);

    // r14 = buffer (imm64 at +2 relocated to the buffer data symbol); r15 = length.
    bytes.extend([0x49, 0xbe]);
    bytes.extend(0u64.to_le_bytes());
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
    // mov r13, imm64(target) (relocated at +2); store the descriptor.
    let target_mov_offset = bytes.len();
    bytes.extend([0x49, 0xbd]);
    bytes.extend(0u64.to_le_bytes());
    bytes.extend([0x4d, 0x89, 0xb5]); // mov [r13+target_offset], r14 (descriptor.ptr)
    bytes.extend(target_ptr_disp.to_le_bytes());
    bytes.extend([0x4d, 0x89, 0xbd]); // mov [r13+target_offset+8], r15 (descriptor.len)
    bytes.extend(target_len_disp.to_le_bytes());

    Ok((bytes, target_mov_offset))
}

fn runtime_text_line_read_syscall_layout() -> (usize, usize) {
    // Capacity/number/target are all fixed-width immediates, so they do not affect the
    // layout; encode once with placeholders to recover the width + target imm offset.
    let (bytes, target_mov_offset) = build_runtime_text_line_read_syscall(0, 1, 0)
        .expect("runtime text line read syscall layout encodes");
    (bytes.len(), target_mov_offset)
}

pub fn runtime_text_line_read_syscall_width() -> usize {
    runtime_text_line_read_syscall_layout().0
}

pub fn runtime_text_line_read_syscall_target_imm_offset() -> usize {
    runtime_text_line_read_syscall_layout().1
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
    Ok(build_runtime_text_line_read(target_offset, capacity)?.0)
}

// ---- compact_binary v0 wire-encode appends (chapter 20, decision 10) ----
//
// Both operations share one cursor convention: the caller's `written` slot
// holds the running byte count, so every append loads it, stores through a
// moving pointer (`out base + out offset + cursor`), and writes the advanced
// cursor back. Register use: r15 = moving out pointer, r14 = written page,
// r10 = cursor, rax = runtime scalar, r11 = byte/zigzag scratch.
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

/// Byte offset of the WRITTEN page mov inside both wire appends (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_append_written_page_offset(_out_offset: usize) -> usize {
    17
}

/// Byte offset of the SOURCE page mov inside the varint append.
pub fn wire_append_varint_source_page_offset(
    _out_offset: usize,
    _written_offset: usize,
) -> usize {
    37
}

pub fn runtime_machine_string_write_width(_byte_length: usize) -> usize {
    44
}

pub fn runtime_frame_string_write_width(byte_length: usize) -> usize {
    runtime_machine_string_write_width(byte_length)
}

pub fn encode_runtime_machine_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_string_write_width(byte_length));
    append_mov_r14_imm64(&mut bytes, 0);
    append_mov_r15_imm64(&mut bytes, 0);
    append_store_r14_to_r15(&mut bytes, byte_offset)?;
    append_mov_rax_imm64(&mut bytes, byte_length as u64);
    append_store_rax_to_r15(&mut bytes, byte_offset + 8, 8)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_string_write_width(byte_length));
    append_mov_r14_imm64(&mut bytes, 0);
    append_mov_r15_imm64(&mut bytes, 0);
    append_store_r14_to_r15(&mut bytes, byte_offset)?;
    append_mov_rax_imm64(&mut bytes, byte_length as u64);
    append_store_rax_to_r15(&mut bytes, byte_offset + 8, 8)?;
    Ok(bytes)
}

pub fn runtime_storage_address_to_runtime_frame_write_width() -> usize {
    34
}

pub fn encode_runtime_storage_address_to_runtime_frame_write(
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_address_to_runtime_frame_write_width());
    append_mov_r14_imm64(&mut bytes, 0);
    append_add_r14_imm32(&mut bytes, source_offset)?;
    append_mov_r15_imm64(&mut bytes, 0);
    append_store_r14_to_r15(&mut bytes, target_offset)?;
    Ok(bytes)
}

/// Bytes inserted between the left and right operand evaluations of a binary
/// write on x86_64: a single `push r10` that preserves the left result while the
/// right operand is evaluated (both accumulate in r10). Relocation planning adds
/// this to the right operand's start offset.
pub const BINARY_RIGHT_OPERAND_PUSH_WIDTH: usize = 2;

pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
) -> usize {
    // 10 (mov r14,imm64) + left + push r10 (2) + right + mov r11,r10 (3)
    // + pop r10 (2) + operation + store.
    10 + runtime_value_operand_width(runtime_value_operands, left)
        + 2
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3
        + 2
        + runtime_binary_operation_or_float_width(operator, byte_size, is_float)
        + 7.max(store_width(byte_size))
}

/// Width of the in-register operation step, dispatching to the SSE float op when
/// the write is floating-point.
fn runtime_binary_operation_or_float_width(
    operator: StateGuardOperator,
    byte_size: usize,
    is_float: bool,
) -> usize {
    if is_float {
        runtime_float_binary_operation_width()
    } else {
        runtime_binary_operation_width(operator, byte_size)
    }
}

pub fn encode_runtime_storage_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
        is_float,
    ));
    // Hold the target base in r14, not r15: evaluating the operands below
    // reloads r15 with each source base, which would otherwise clobber the
    // target pointer before the store. r14 is untouched by operand evaluation.
    // `mov r14, imm64` and `mov r15, imm64` are both 10 bytes with the relocated
    // immediate at +2, so the target relocation offset is unchanged.
    append_mov_r14_imm64(&mut bytes, 0);
    // Each operand's evaluation accumulates in r10, so the right operand would
    // clobber the left result. Stash left on the stack across the right eval.
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_push_r10(&mut bytes);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, right)?;
    append_mov_reg_reg(&mut bytes, Reg64::R11, Reg64::R10); // right -> r11
    append_pop_r10(&mut bytes); // restore left -> r10
    if is_float {
        append_runtime_float_binary_operation(&mut bytes, operator, byte_size)?;
    } else {
        append_runtime_binary_operation(
            &mut bytes,
            operator,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )?;
    }
    append_store_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    Ok(bytes)
}

/// Bytes of the in-register conversion step for a numeric `as` cast (the source
/// bits are already in r10; the result is left in r10 for the store).
fn runtime_convert_operation_width(
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> usize {
    match (source_is_float, target_is_float) {
        // movq/movd xmm0,r10 (5) + cvttsd2si/cvttss2si r10,xmm0 (5)
        (true, false) => 10,
        // cvtsi2sd/ss xmm0,r10 (5) + movq/movd r10,xmm0 (5)
        (false, true) => 10,
        (true, true) => {
            if source_byte_size == target_byte_size {
                0 // f64->f64: bits already in r10
            } else {
                14 // movq/movd (5) + cvtsd2ss/cvtss2sd (4) + movd/movq (5)
            }
        }
        (false, false) => {
            // Sign-extend a narrow signed source when widening; otherwise the
            // load already zero-extended and the store truncates.
            if target_byte_size > source_byte_size && source_signed && source_byte_size == 4 {
                3 // movsxd r10, r10d
            } else {
                0
            }
        }
    }
}

/// Append the in-register conversion (see [`runtime_convert_operation_width`]).
fn append_runtime_convert_operation(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) {
    match (source_is_float, target_is_float) {
        (true, false) => {
            // float -> int: move bits into xmm0, truncating-convert to r10.
            if source_byte_size > 4 {
                bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
                bytes.extend([0xf2, 0x4c, 0x0f, 0x2c, 0xd0]); // cvttsd2si r10, xmm0
            } else {
                bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
                bytes.extend([0xf3, 0x4c, 0x0f, 0x2c, 0xd0]); // cvttss2si r10, xmm0
            }
        }
        (false, true) => {
            // int -> float: convert r10 (signed) into xmm0, move bits back to r10.
            if source_byte_size > 4 {
                if target_byte_size > 4 {
                    bytes.extend([0xf2, 0x49, 0x0f, 0x2a, 0xc2]); // cvtsi2sd xmm0, r10
                } else {
                    bytes.extend([0xf3, 0x49, 0x0f, 0x2a, 0xc2]); // cvtsi2ss xmm0, r10
                }
            } else if target_byte_size > 4 {
                bytes.extend([0xf2, 0x41, 0x0f, 0x2a, 0xc2]); // cvtsi2sd xmm0, r10d
            } else {
                bytes.extend([0xf3, 0x41, 0x0f, 0x2a, 0xc2]); // cvtsi2ss xmm0, r10d
            }
            if target_byte_size > 4 {
                bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
            } else {
                bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
            }
        }
        (true, true) => {
            if source_byte_size == target_byte_size {
                // f64 -> f64: nothing to do.
            } else if source_byte_size > target_byte_size {
                bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
                bytes.extend([0xf2, 0x0f, 0x5a, 0xc0]); // cvtsd2ss xmm0, xmm0
                bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
            } else {
                bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
                bytes.extend([0xf3, 0x0f, 0x5a, 0xc0]); // cvtss2sd xmm0, xmm0
                bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
            }
        }
        (false, false) => {
            if target_byte_size > source_byte_size && source_signed && source_byte_size == 4 {
                bytes.extend([0x4d, 0x63, 0xd2]); // movsxd r10, r10d
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_convert_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> usize {
    // mov r14,imm64(target base) (10) + source operand load + convert + store.
    10 + runtime_value_operand_width(runtime_value_operands, source)
        + runtime_convert_operation_width(
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
        )
        + store_width(target_byte_size)
}

/// `target = source as T`: hold the target base in r14 (untouched by operand
/// evaluation, which reloads r15), evaluate the source operand into r10, convert
/// it in place between integer/float representations, and store the result.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_convert(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_convert_width(
        runtime_value_operands,
        source,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // target base (imm64 @ +2 relocated)
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, source)?;
    append_runtime_convert_operation(
        &mut bytes,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
    );
    append_store_r10_to_r14(&mut bytes, target_offset, target_byte_size)?;
    Ok(bytes)
}

/// Address-computation prefix before the value operands in a pointee binary
/// write: `mov r14,imm64(frame)` (10) + `mov r14,[r14+ptr]` (7) -- r14 then holds
/// the dereferenced runtime pointer (the target base) across operand evaluation.
pub fn runtime_pointee_binary_operand_start_width() -> usize {
    17
}

pub fn runtime_pointee_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    // 17 (frame base + deref ptr) + left + push r10 (2) + right + mov r11,r10 (3)
    // + pop r10 (2) + operation + store.
    runtime_pointee_binary_operand_start_width()
        + runtime_value_operand_width(runtime_value_operands, left)
        + 2
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3
        + 2
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

/// `*(frame[pointer_byte_offset]) + field_byte_offset = left OP right`, where the
/// operands resolve against the runtime frame. The dereferenced target pointer is
/// held in r14 (untouched by operand evaluation, which reloads r15/r10/r11).
pub fn encode_runtime_pointee_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // frame base (imm64 @ +2 relocated)
    append_load_r14_from_r14(&mut bytes, pointer_byte_offset)?; // r14 = runtime pointer (target base)
    debug_assert_eq!(bytes.len(), runtime_pointee_binary_operand_start_width());
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_push_r10(&mut bytes);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, right)?;
    append_mov_reg_reg(&mut bytes, Reg64::R11, Reg64::R10); // right -> r11
    append_pop_r10(&mut bytes); // restore left -> r10
    append_runtime_binary_operation(
        &mut bytes,
        operator,
        runtime_binary_operation_byte_size(runtime_value_operands, operator, left, right, byte_size),
    )?;
    append_store_r10_to_r14(&mut bytes, field_byte_offset, byte_size)?;
    Ok(bytes)
}

/// Length of the address-computation prefix that precedes the value operands in
/// a frame-base-indexed binary write: `mov r14,imm64(frame)` (10) +
/// `mov r15,[r14+idx]` (7) + `imul r15,r15,elem` (7) + `add r14,r15` (3).
pub fn runtime_frame_base_indexed_binary_left_operand_offset() -> usize {
    27
}

pub fn runtime_frame_base_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_frame_base_indexed_binary_left_operand_offset()
        + runtime_value_operand_width(runtime_value_operands, left)
        + 2 // push r10
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3 // mov r11, r10
        + 2 // pop r10
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let store_displacement = base_byte_offset + field_byte_offset;
    let mut bytes = Vec::with_capacity(runtime_frame_base_indexed_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
    ));
    // r14 = frame base + index*element (target address held across operand
    // evaluation, which freely clobbers r15/r10/r11 but never r14).
    append_mov_r14_imm64(&mut bytes, 0); // imm64 at +2 relocated to the frame symbol
    append_load_r15_from_r14(&mut bytes, index_offset)?;
    append_imul_r15_imm32(&mut bytes, element_scale(element_byte_size)?);
    append_add_r14_r15(&mut bytes);
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_base_indexed_binary_left_operand_offset()
    );
    // Stash the left result across the right operand's evaluation (both accumulate
    // in r10). r14 (target address) survives push/pop and operand evaluation.
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_push_r10(&mut bytes);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, right)?;
    append_mov_reg_reg(&mut bytes, Reg64::R11, Reg64::R10); // right -> r11
    append_pop_r10(&mut bytes); // restore left -> r10
    append_runtime_binary_operation(
        &mut bytes,
        operator,
        runtime_binary_operation_byte_size(runtime_value_operands, operator, left, right, byte_size),
    )?;
    append_store_r10_to_r14(&mut bytes, store_displacement, byte_size)?;
    Ok(bytes)
}

pub fn runtime_storage_copy_width(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    20 + runtime_storage_copy_chunk_count(source_offset, target_offset, byte_count) * 14
}

pub fn encode_runtime_storage_copy(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_width(
        source_offset,
        target_offset,
        byte_count,
    ));
    append_mov_r14_imm64(&mut bytes, 0);
    append_mov_r15_imm64(&mut bytes, 0);
    for_each_runtime_copy_chunk(
        source_offset,
        target_offset,
        byte_count,
        |offset, chunk_size| {
            append_load_rax_from_r14(&mut bytes, source_offset + offset, chunk_size)?;
            append_store_rax_to_r15(&mut bytes, target_offset + offset, chunk_size)?;
            Ok(())
        },
    )?;
    Ok(bytes)
}

pub fn runtime_storage_copy_to_runtime_pointee_width(
    source_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    // mov r14,imm64(source) (10) + mov r15,imm64(frame) (10)
    // + mov r15,[r15+ptr] (7) + per-chunk load/store (14 each).
    27 + runtime_storage_copy_chunk_count(source_offset, field_byte_offset, byte_count) * 14
}

/// Copies `byte_count` bytes from the source storage region (`source_offset`)
/// into the memory pointed at by a frame pointer slot
/// (`*(frame[pointer_byte_offset]) + field_byte_offset`). r14 holds the source
/// base (relocated to the source region), r15 the dereferenced target pointer
/// (loaded after relocating the frame base).
pub fn encode_runtime_storage_copy_to_runtime_pointee(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_to_runtime_pointee_width(
        source_offset,
        field_byte_offset,
        byte_count,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // source base (reloc @ +2)
    append_mov_r15_imm64(&mut bytes, 0); // frame base (reloc @ +12)
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?; // r15 = target pointer
    // The chunk planner aligns on source/target offsets; use field_byte_offset as
    // the target base so chunking matches the relocation-free store displacements.
    for_each_runtime_copy_chunk(
        source_offset,
        field_byte_offset,
        byte_count,
        |offset, chunk_size| {
            append_load_rax_from_r14(&mut bytes, source_offset + offset, chunk_size)?;
            append_store_rax_to_r15(&mut bytes, field_byte_offset + offset, chunk_size)?;
            Ok(())
        },
    )?;
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    // mov r14,imm64(frame) + mov r14,[r14+ptr] + mov r15,imm64(frame)
    // + per-chunk load/store.
    27 + runtime_storage_copy_chunk_count(field_byte_offset, target_offset, byte_count) * 14
}

/// Copies `byte_count` bytes from memory pointed at by a frame pointer slot
/// (`*(frame[pointer_byte_offset]) + field_byte_offset`) into `frame[target_offset]`.
pub fn encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    append_mov_r14_imm64(&mut bytes, 0); // frame base for source pointer (reloc @ +2)
    append_load_r14_from_r14(&mut bytes, pointer_byte_offset)?; // r14 = source pointer
    append_mov_r15_imm64(&mut bytes, 0); // frame base for target slot (reloc at instruction +17)
    for_each_runtime_copy_chunk(
        field_byte_offset,
        target_offset,
        byte_count,
        |offset, chunk_size| {
            append_load_rax_from_r14(&mut bytes, field_byte_offset + offset, chunk_size)?;
            append_store_rax_to_r15(&mut bytes, target_offset + offset, chunk_size)?;
            Ok(())
        },
    )?;
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
    _element_index: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
    byte_count: usize,
) -> usize {
    // mov r15,imm64 (10) + mov r14,[r15+desc] (7) + per chunk: load rax (7) + store rax (7).
    17 + runtime_storage_copy_chunk_count(0, 0, byte_count) * 14
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("X86_64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    let mut bytes =
        Vec::with_capacity(runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ));
    // r15 = frame base (imm64 at +2 relocated). r14 = slice data pointer from the
    // descriptor; copy source [r14 + source_offset] -> target [r15 + target_offset].
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_r14_from_r15(&mut bytes, descriptor_offset)?;
    for_each_runtime_copy_chunk(
        source_offset,
        target_offset,
        byte_count,
        |offset, chunk_size| {
            append_load_rax_from_r14(&mut bytes, source_offset + offset, chunk_size)?;
            append_store_rax_to_r15(&mut bytes, target_offset + offset, chunk_size)?;
            Ok(())
        },
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count
        )
    );
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(source_field_byte_offset);
    // mov r15,imm64(frame) + mov r14,[r15+desc] + mov r15,[r15+ptr] + per-chunk load/store.
    24 + runtime_storage_copy_chunk_count(source_offset, target_field_byte_offset, byte_count) * 14
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(source_field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("X86_64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    let mut bytes = Vec::with_capacity(
        runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
            element_index,
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
        ),
    );
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_r14_from_r15(&mut bytes, descriptor_offset)?;
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?;
    for_each_runtime_copy_chunk(
        source_offset,
        target_field_byte_offset,
        byte_count,
        |offset, chunk_size| {
            append_load_rax_from_r14(&mut bytes, source_offset + offset, chunk_size)?;
            append_store_rax_to_r15(&mut bytes, target_field_byte_offset + offset, chunk_size)?;
            Ok(())
        },
    )?;
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_width(
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
    byte_count: usize,
) -> usize {
    // mov r15,imm64 (10) + mov r14,[r15+desc] (7) + mov r11,[r15+idx] (7)
    // + imul r11,r11,elem (7) + add r14,r11 (3) + per chunk: load rax (7) + store rax (7).
    34 + runtime_storage_copy_chunk_count(0, 0, byte_count) * 14
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
    _element_byte_size: usize,
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    // mov r15,imm64(frame) (10) + mov r14,[r15+desc] (7) + mov r11d,[r15+idx] (7)
    // + imul r11,r11,elem (7) + add r14,r11 (3) + mov r15,[r15+ptr] (7)
    // + per chunk: load rax (7) + store rax (7).
    41 + runtime_storage_copy_chunk_count(
        source_field_byte_offset,
        target_field_byte_offset,
        byte_count,
    ) * 14
}

/// Copies `byte_count` bytes from a runtime-frame slice element field
/// (`*(frame[descriptor]) + index*elem + source_field`, index read from
/// `frame[index_offset]`) through a `&mut` reference into its pointee field
/// (`*(frame[pointer]) + target_field`). Runtime-index sibling of
/// `encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee`.
pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
        ),
    );
    append_mov_r15_imm64(&mut bytes, 0); // r15 = frame base (reloc @ +2)
    append_load_r14_from_r15(&mut bytes, descriptor_offset)?; // r14 = descriptor.ptr
    append_load_index_r11_from_r15(&mut bytes, index_offset)?; // r11 = index (32-bit)
    append_imul_r11_imm32(&mut bytes, element_scale(element_byte_size)?);
    append_add_r14_r11(&mut bytes); // r14 = element base
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?; // r15 = target pointer (frame base consumed last)
    for_each_runtime_copy_chunk(
        source_field_byte_offset,
        target_field_byte_offset,
        byte_count,
        |offset, chunk_size| {
            append_load_rax_from_r14(&mut bytes, source_field_byte_offset + offset, chunk_size)?;
            append_store_rax_to_r15(&mut bytes, target_field_byte_offset + offset, chunk_size)?;
            Ok(())
        },
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
        )
    );
    Ok(bytes)
}

/// Copies `byte_count` bytes from a runtime-frame slice element field
/// (`*(frame[descriptor]) + index*elem + field`, where `index` is read from
/// `frame[index_offset]`) into `frame[target_offset]`. The runtime-index sibling
/// of `encode_runtime_storage_copy_from_runtime_frame_fixed_indexed`.
pub fn encode_runtime_storage_copy_from_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_from_runtime_frame_indexed_width(
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    ));
    // r15 = frame base (reloc @ +2). r14 = slice data pointer + index*element, so
    // the copy source is [r14 + field + offset]; target is [r15 + target + offset].
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_r14_from_r15(&mut bytes, descriptor_offset)?;
    append_load_index_r11_from_r15(&mut bytes, index_offset)?;
    append_imul_r11_imm32(&mut bytes, element_scale(element_byte_size)?);
    append_add_r14_r11(&mut bytes);
    for_each_runtime_copy_chunk(
        field_byte_offset,
        target_offset,
        byte_count,
        |offset, chunk_size| {
            append_load_rax_from_r14(&mut bytes, field_byte_offset + offset, chunk_size)?;
            append_store_rax_to_r15(&mut bytes, target_offset + offset, chunk_size)?;
            Ok(())
        },
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_from_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count
        )
    );
    Ok(bytes)
}

pub fn runtime_value_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    if runtime_value_operands.immediate_integer(operand).is_some() {
        10
    } else if let Some((_, _, byte_size)) = runtime_value_operands.storage(operand) {
        10 + load_width(byte_size)
    } else if runtime_value_operands.pointee(operand).is_some() {
        // mov r15,imm64 (10) + mov rax,[r15+ptr_off] (7) + load dest,[rax+field] (7)
        24
    } else if runtime_value_operands.frame_indexed(operand).is_some() {
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7) + mov r11,[r15+idx] (7)
        // + imul r11,r11,elem (7) + add rax,r11 (3) + load dest,[rax+field] (7)
        41
    } else if runtime_value_operands.frame_base_indexed(operand).is_some() {
        // mov r15,imm64 (10) + mov r11,[r15+idx] (7) + imul r11,r11,elem (7)
        // + mov rax,r15 (3) + add rax,r11 (3) + load dest,[rax+base+field] (7)
        37
    } else if runtime_value_operands
        .frame_fixed_indexed(operand)
        .is_some()
    {
        // Constant element index folds into the load displacement, so the shape
        // matches the pointee case: mov r15,imm64 (10) + mov rax,[r15+desc] (7)
        // + load dest,[rax+const] (7).
        24
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        let operation_width = if runtime_value_operands.binary_is_float(operand) {
            // Float operands: the SSE op (movq xmm<-r, op, movq r<-xmm) is a fixed
            // width regardless of operator. MUST match the emission below or the
            // recorded relocation offsets drift (silent runtime segfault).
            runtime_float_binary_operation_width()
        } else {
            // Nested binary operands do not carry their result width; assume the
            // 64-bit form (correct for i64 and for non-negative i32 division).
            runtime_binary_operation_width(operator, 8)
        };
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + operation_width
            // push r10 (2) + mov r11,r10 (3) + pop r10 (2) + mov dest,r10 (3)
            + 10
    } else if let Some((source, src_bytes, tgt_bytes, src_float, tgt_float, src_signed)) =
        runtime_value_operands.convert(operand)
    {
        // Load source into r10, convert it in place, then mov dest,r10 (3). MUST
        // match the emission below or relocation offsets drift (runtime segfault).
        runtime_value_operand_width(runtime_value_operands, source)
            + runtime_convert_operation_width(src_bytes, tgt_bytes, src_float, tgt_float, src_signed)
            + 3
    } else {
        0
    }
}

fn append_runtime_value_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination: Reg64,
    operand: RuntimeValueOperandHandle,
) -> Result<(), Diagnostic> {
    if let Some(value) = runtime_value_operands.immediate_integer(operand) {
        append_mov_reg_imm64(bytes, destination, value as u64);
        Ok(())
    } else if let Some((_, byte_offset, byte_size)) = runtime_value_operands.storage(operand) {
        append_mov_r15_imm64(bytes, 0);
        append_load_reg_from_r15(bytes, destination, byte_offset, byte_size)
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        runtime_value_operands.pointee(operand)
    {
        // r15 = frame base (relocated). rax = the stored pointer; load through it.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, pointer_byte_offset)?;
        append_load_reg_from_rax(bytes, destination, field_byte_offset, byte_size)
    } else if let Some((
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_indexed(operand)
    {
        // r15 = frame base (relocated). rax = slice data pointer from the descriptor;
        // r11 = index; rax += index*element + ... then load [rax + field].
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 8)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_add_rax_r11(bytes);
        append_load_reg_from_rax(bytes, destination, field_byte_offset, byte_size)
    } else if let Some((
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_base_indexed(operand)
    {
        // r15 = frame base (relocated). The base lives inline in the frame at
        // base_byte_offset; rax = frame base, then add scaled index + base + field.
        append_mov_r15_imm64(bytes, 0);
        append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 8)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_mov_rax_r15(bytes);
        append_add_rax_r11(bytes);
        append_load_reg_from_rax(
            bytes,
            destination,
            base_byte_offset + field_byte_offset,
            byte_size,
        )
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_fixed_indexed(operand)
    {
        // Descriptor-based access with a constant element index: r15 = frame base
        // (relocated), rax = the slice data pointer, then load through it at the
        // constant displacement `element_index*element + field`.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        let displacement = element_index
            .checked_mul(element_byte_size)
            .and_then(|scaled| scaled.checked_add(field_byte_offset))
            .ok_or_else(|| {
                Diagnostic::error("X86_64 fixed indexed value operand offset overflow")
            })?;
        append_load_reg_from_rax(bytes, destination, displacement, byte_size)
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        // Every comparison/operation accumulates its result in r10, so evaluating
        // the right operand clobbers the left result. Stash left on the stack
        // across the right evaluation, then combine.
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, left)?;
        append_push_r10(bytes);
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, right)?;
        append_mov_reg_reg(bytes, Reg64::R11, Reg64::R10); // right -> r11
        append_pop_r10(bytes); // restore left -> r10
        if runtime_value_operands.binary_is_float(operand) {
            // Float operands carry their IEEE bits in r10/r11; do the SSE op on the
            // bits (addsd/...) rather than an integer add over them. Default f64
            // width (8); f32 value-operand arithmetic is a further gap. MUST match
            // runtime_float_binary_operation_width() used in the width fn above.
            append_runtime_float_binary_operation(bytes, operator, 8)?;
        } else {
            // Comparisons use the operand width; other nested binaries do not carry
            // their result width, so assume 64-bit (matches runtime_value_operand_
            // width above for relocation consistency).
            append_runtime_binary_operation(
                bytes,
                operator,
                runtime_binary_operation_byte_size(runtime_value_operands, operator, left, right, 8),
            )?;
        }
        append_mov_reg_reg(bytes, destination, Reg64::R10);
        Ok(())
    } else if let Some((source, src_bytes, tgt_bytes, src_float, tgt_float, src_signed)) =
        runtime_value_operands.convert(operand)
    {
        // Load the cast's source into r10, convert it in place (cvttsd2si /
        // cvtsi2sd / cvtsd2ss / movsxd), then move the result to `destination`.
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, source)?;
        append_runtime_convert_operation(
            bytes,
            src_bytes,
            tgt_bytes,
            src_float,
            tgt_float,
            src_signed,
        );
        append_mov_reg_reg(bytes, destination, Reg64::R10);
        Ok(())
    } else {
        Err(Diagnostic::error(
            "X86_64 runtime value operand is not implemented yet",
        ))
    }
}

/// Value width of a runtime operand, looking through nested binary operands.
/// `None` for immediates (which carry no width). Used to size comparisons, whose
/// result type (bool) does not reflect the compared operands' width.
fn runtime_value_operand_value_byte_size(
    operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> Option<usize> {
    if let Some((_, _, byte_size)) = operands.storage(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, byte_size)) = operands.pointee(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_base_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_fixed_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((left, _, right)) = operands.binary(operand) {
        return runtime_value_operand_value_byte_size(operands, left)
            .or_else(|| runtime_value_operand_value_byte_size(operands, right));
    }
    if let Some((_, _, target_byte_size, _, _, _)) = operands.convert(operand) {
        return Some(target_byte_size);
    }
    None
}

/// Width to compare two operands at: the first operand with a known width, else
/// the i32 default. (`a OP b` requires `a` and `b` to share a type, so either
/// operand's width is the comparison width.)
fn runtime_binary_compare_byte_size(
    operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_value_byte_size(operands, left)
        .or_else(|| runtime_value_operand_value_byte_size(operands, right))
        .unwrap_or(4)
}

fn is_comparison_operator(operator: StateGuardOperator) -> bool {
    matches!(
        operator,
        StateGuardOperator::Equal
            | StateGuardOperator::NotEqual
            | StateGuardOperator::Greater
            | StateGuardOperator::GreaterOrEqual
            | StateGuardOperator::Less
            | StateGuardOperator::LessOrEqual
            | StateGuardOperator::GreaterUnsigned
            | StateGuardOperator::GreaterOrEqualUnsigned
            | StateGuardOperator::LessUnsigned
            | StateGuardOperator::LessOrEqualUnsigned
    )
}

/// Width to pass to `append_runtime_binary_operation`. Comparisons produce a
/// `bool`, so the target width is not the compared-operands' width — derive it
/// from the operands instead. All other operations share the target's width.
fn runtime_binary_operation_byte_size(
    operands: &impl RuntimeValueOperandSource,
    operator: StateGuardOperator,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    target_byte_size: usize,
) -> usize {
    if is_comparison_operator(operator) {
        runtime_binary_compare_byte_size(operands, left, right)
    } else {
        target_byte_size
    }
}

fn append_runtime_binary_operation(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    match operator {
        StateGuardOperator::Add => bytes.extend([0x4d, 0x01, 0xda]), // add r10, r11
        StateGuardOperator::And => bytes.extend([0x4d, 0x21, 0xda]), // and r10, r11
        StateGuardOperator::Or => bytes.extend([0x4d, 0x09, 0xda]),  // or r10, r11
        StateGuardOperator::Subtract => bytes.extend([0x4d, 0x29, 0xda]), // sub r10, r11
        StateGuardOperator::Multiply => bytes.extend([0x4d, 0x0f, 0xaf, 0xd3]), // imul r10, r11
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => {
            // Compare at the operand width (32-bit for i32, else 64-bit) so an
            // i32 sign/high bit is read correctly, then conditionally take r11.
            // Max keeps the larger (cmovl signed / cmovb unsigned: replace when
            // r10 < r11); Min keeps the smaller (cmovg / cmova: replace when
            // r10 > r11).
            let keep_smaller =
                matches!(operator, StateGuardOperator::Min | StateGuardOperator::MinUnsigned);
            let unsigned =
                matches!(operator, StateGuardOperator::MaxUnsigned | StateGuardOperator::MinUnsigned);
            // cmov opcode byte: signed below/above use 4c/4f; unsigned 42/47.
            let cmov = match (keep_smaller, unsigned) {
                (false, false) => 0x4c, // cmovl
                (true, false) => 0x4f,  // cmovg
                (false, true) => 0x42,  // cmovb
                (true, true) => 0x47,   // cmova
            };
            if byte_size <= 4 {
                bytes.extend([0x45, 0x39, 0xda]); // cmp r10d, r11d
                bytes.extend([0x45, 0x0f, cmov, 0xd3]); // cmovcc r10d, r11d
            } else {
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x4d, 0x0f, cmov, 0xd3]); // cmovcc r10, r11
            }
        }
        StateGuardOperator::Divide
        | StateGuardOperator::Modulo
        | StateGuardOperator::DivideUnsigned
        | StateGuardOperator::ModuloUnsigned => {
            // Width must match the operands so the high bit is interpreted right
            // (a 32-bit divide reads only the low dword). Signed uses cdq/cqo +
            // `idiv`; unsigned zeroes the dividend-high half + `div`. Quotient ->
            // (r/e)ax, remainder -> (r/e)dx.
            let want_remainder = matches!(
                operator,
                StateGuardOperator::Modulo | StateGuardOperator::ModuloUnsigned
            );
            let signed = matches!(
                operator,
                StateGuardOperator::Divide | StateGuardOperator::Modulo
            );
            if byte_size <= 4 {
                bytes.extend([0x41, 0x8b, 0xc2]); // mov eax, r10d
                if signed {
                    bytes.push(0x99); // cdq (sign-extend eax -> edx)
                    bytes.extend([0x41, 0xf7, 0xfb]); // idiv r11d
                } else {
                    bytes.extend([0x31, 0xd2]); // xor edx, edx
                    bytes.extend([0x41, 0xf7, 0xf3]); // div r11d
                }
                if want_remainder {
                    bytes.extend([0x41, 0x89, 0xd2]); // mov r10d, edx (remainder)
                } else {
                    bytes.extend([0x41, 0x89, 0xc2]); // mov r10d, eax (quotient)
                }
            } else {
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                if signed {
                    bytes.extend([0x48, 0x99]); // cqo (sign-extend rax -> rdx)
                    bytes.extend([0x49, 0xf7, 0xfb]); // idiv r11
                } else {
                    bytes.extend([0x31, 0xd2]); // xor edx, edx (clears rdx)
                    bytes.extend([0x49, 0xf7, 0xf3]); // div r11
                }
                if want_remainder {
                    bytes.extend([0x49, 0x89, 0xd2]); // mov r10, rdx (remainder)
                } else {
                    bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax (quotient)
                }
            }
        }
        StateGuardOperator::ShiftLeft
        | StateGuardOperator::ShiftRight
        | StateGuardOperator::ShiftRightLogical => {
            // Shift count must live in cl. Right shift is arithmetic (`sar`) for
            // signed operands and logical (`shr`) for unsigned; sized to the
            // operands so an i32 high bit is honored.
            let arithmetic_right = matches!(operator, StateGuardOperator::ShiftRight);
            let logical_right = matches!(operator, StateGuardOperator::ShiftRightLogical);
            if byte_size <= 4 {
                bytes.extend([0x44, 0x89, 0xd9]); // mov ecx, r11d
                if arithmetic_right {
                    bytes.extend([0x41, 0xd3, 0xfa]); // sar r10d, cl
                } else if logical_right {
                    bytes.extend([0x41, 0xd3, 0xea]); // shr r10d, cl
                } else {
                    bytes.extend([0x41, 0xd3, 0xe2]); // shl r10d, cl
                }
            } else {
                bytes.extend([0x4c, 0x89, 0xd9]); // mov rcx, r11
                if arithmetic_right {
                    bytes.extend([0x49, 0xd3, 0xfa]); // sar r10, cl
                } else if logical_right {
                    bytes.extend([0x49, 0xd3, 0xea]); // shr r10, cl
                } else {
                    bytes.extend([0x49, 0xd3, 0xe2]); // shl r10, cl
                }
            }
        }
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => {
            // Compare at the operand width (`byte_size` here is the operand
            // width, not the bool result) so an i32 sign bit is read correctly.
            // Ordering uses signed setcc (setl/setg/...) or unsigned (setb/seta/
            // ...) per the operand type.
            append_cmp_r10_r11(bytes, byte_size)?;
            bytes.extend(match operator {
                StateGuardOperator::Equal => [0x0f, 0x94, 0xc0], // sete
                StateGuardOperator::NotEqual => [0x0f, 0x95, 0xc0], // setne
                StateGuardOperator::Greater => [0x0f, 0x9f, 0xc0], // setg
                StateGuardOperator::GreaterOrEqual => [0x0f, 0x9d, 0xc0], // setge
                StateGuardOperator::Less => [0x0f, 0x9c, 0xc0], // setl
                StateGuardOperator::LessOrEqual => [0x0f, 0x9e, 0xc0], // setle
                StateGuardOperator::GreaterUnsigned => [0x0f, 0x97, 0xc0], // seta
                StateGuardOperator::GreaterOrEqualUnsigned => [0x0f, 0x93, 0xc0], // setae
                StateGuardOperator::LessUnsigned => [0x0f, 0x92, 0xc0], // setb
                StateGuardOperator::LessOrEqualUnsigned => [0x0f, 0x96, 0xc0], // setbe
                _ => unreachable!(),
            });
            bytes.extend([0x44, 0x0f, 0xb6, 0xd0]); // movzx r10d, al
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime binary operator `{operator:?}` is not implemented yet"
            )));
        }
    }
    Ok(())
}

/// Floating-point binary op (f64/f32) that reuses the integer operand pipeline:
/// the operand bit patterns are already loaded in r10 (left) and r11 (right).
/// Move them into xmm0/xmm1, run the SSE arithmetic op, then move the result
/// bits back to r10 so the shared store path writes them out. `byte_size > 4`
/// selects f64 (`movq` + `*sd`); otherwise f32 (`movd` + `*ss`). Always the
/// fixed `runtime_float_binary_operation_width()` bytes.
fn append_runtime_float_binary_operation(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let wide = byte_size > 4;
    if wide {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xcb]); // movq xmm1, r11
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xcb]); // movd xmm1, r11d
    }
    // F2 = scalar-double prefix (`*sd`), F3 = scalar-single (`*ss`).
    let scalar_prefix = if wide { 0xf2 } else { 0xf3 };
    let opcode = match operator {
        StateGuardOperator::Add => 0x58,      // addsd/addss
        StateGuardOperator::Subtract => 0x5c, // subsd/subss
        StateGuardOperator::Multiply => 0x59, // mulsd/mulss
        StateGuardOperator::Divide => 0x5e,   // divsd/divss
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime float binary operator `{operator:?}` is not implemented yet"
            )));
        }
    };
    bytes.extend([scalar_prefix, 0x0f, opcode, 0xc1]); // <op> xmm0, xmm1
    if wide {
        bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
    }
    Ok(())
}

/// Fixed width of [`append_runtime_float_binary_operation`]: two operand moves
/// (5 each) + the SSE op (4) + the result move (5) = 19, for both f32 and f64.
fn runtime_float_binary_operation_width() -> usize {
    19
}

fn runtime_binary_operation_width(operator: StateGuardOperator, byte_size: usize) -> usize {
    match operator {
        StateGuardOperator::Add
        | StateGuardOperator::And
        | StateGuardOperator::Or
        | StateGuardOperator::Subtract => 3,
        StateGuardOperator::Multiply => 4,
        // cmp (3) + cmov (4), same at 32-bit or 64-bit.
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => 7,
        // signed 32-bit: mov(3)+cdq(1)+idiv(3)+mov(3)=10; signed 64-bit: cqo(2)=11.
        StateGuardOperator::Divide | StateGuardOperator::Modulo => {
            if byte_size <= 4 { 10 } else { 11 }
        }
        // unsigned: mov(3)+xor edx,edx(2)+div(3)+mov(3)=11 at either size.
        StateGuardOperator::DivideUnsigned | StateGuardOperator::ModuloUnsigned => 11,
        // mov c-reg, r11 (3) + shift r10, cl (3), same width at either size.
        StateGuardOperator::ShiftLeft
        | StateGuardOperator::ShiftRight
        | StateGuardOperator::ShiftRightLogical => 6,
        // cmp (3; 4 with the 0x66 prefix at 2-byte width) + setcc (3) + movzx (4).
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => {
            if byte_size == 2 { 11 } else { 10 }
        }
        _ => 0,
    }
}

fn append_input_delimiter_check(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    failure_branch_distance: isize,
) -> Result<(), Diagnostic> {
    append_load_al_from_r15(bytes, byte_offset)?;
    bytes.extend([0x3c, 10]); // cmp al, '\n'
    append_jcc_rel32(bytes, 0x84, 21)?; // je success
    bytes.extend([0x3c, 13]); // cmp al, '\r'
    append_jcc_rel32(bytes, 0x84, 13)?; // je success
    bytes.extend([0x3c, 0]); // cmp al, 0
    append_jcc_rel32(bytes, 0x84, 5)?; // je success
    append_jmp_rel32(bytes, failure_branch_distance)?;
    Ok(())
}

fn append_failure_branch(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    failure_branch_distance: isize,
) -> Result<(), Diagnostic> {
    // The guard jumps to the failure branch when the comparison is FALSE, so each
    // operator maps to its negation. Ordering uses signed (jl/jg/...) or unsigned
    // (jb/ja/...) conditions per the operand type.
    let opcode = match operator {
        StateGuardOperator::Equal => 0x85,                  // jne
        StateGuardOperator::NotEqual => 0x84,               // je
        StateGuardOperator::Greater => 0x8e,                // jle
        StateGuardOperator::GreaterOrEqual => 0x8c,         // jl
        StateGuardOperator::Less => 0x8d,                   // jge
        StateGuardOperator::LessOrEqual => 0x8f,            // jg
        StateGuardOperator::GreaterUnsigned => 0x86,        // jbe
        StateGuardOperator::GreaterOrEqualUnsigned => 0x82, // jb
        StateGuardOperator::LessUnsigned => 0x83,           // jae
        StateGuardOperator::LessOrEqualUnsigned => 0x87,    // ja
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime compare operator `{operator:?}` is not implemented yet"
            )));
        }
    };
    append_jcc_rel32(bytes, opcode, failure_branch_distance)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reg64 {
    R10,
    R11,
}

fn append_mov_reg_imm64(bytes: &mut Vec<u8>, register: Reg64, value: u64) {
    match register {
        Reg64::R10 => append_mov_r10_imm64(bytes, value),
        Reg64::R11 => {
            bytes.extend([0x49, 0xbb]);
            bytes.extend(value.to_le_bytes());
        }
    }
}

fn append_mov_rax_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xb8]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_rdx_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xba]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_r10_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xba]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_r14_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbe]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_r15_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbf]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_reg_reg(bytes: &mut Vec<u8>, destination: Reg64, source: Reg64) {
    match (destination, source) {
        (Reg64::R10, Reg64::R10) => bytes.extend([0x4d, 0x89, 0xd2]),
        (Reg64::R10, Reg64::R11) => bytes.extend([0x4d, 0x89, 0xda]),
        (Reg64::R11, Reg64::R10) => bytes.extend([0x4d, 0x89, 0xd3]),
        (Reg64::R11, Reg64::R11) => bytes.extend([0x4d, 0x89, 0xdb]),
    }
}

fn append_push_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x41, 0x52]); // push r10
}

fn append_pop_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x41, 0x5a]); // pop r10
}

// --- Helpers for the runtime-length text-append memcpy (`rep movsb`) ---

fn append_mov_rcx_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xb9]); // mov rcx, imm64
    bytes.extend(value.to_le_bytes());
}

fn append_load_rax_from_rcx(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x48, 0x8b, 0x81]); // mov rax, [rcx + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rcx_from_rcx(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x48, 0x8b, 0x89]); // mov rcx, [rcx + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_r10_r14(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x89, 0xf2]); // mov r10, r14
}

fn append_add_r10_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xda]); // add r10, r11
}

fn append_add_r10_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!("X86_64 encoder cannot add offset `{value}` to r10"))
    })?;
    bytes.extend([0x49, 0x81, 0xc2]); // add r10, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_add_r11_rcx(bytes: &mut Vec<u8>) {
    bytes.extend([0x49, 0x01, 0xcb]); // add r11, rcx
}

fn append_add_r11_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!("X86_64 encoder cannot add offset `{value}` to r11"))
    })?;
    bytes.extend([0x49, 0x81, 0xc3]); // add r11, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_mov_r11_rcx(bytes: &mut Vec<u8>) {
    bytes.extend([0x49, 0x89, 0xcb]); // mov r11, rcx
}

fn append_load_r11_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x9f]); // mov r11, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Load an array INDEX into r11 from `[r15 + disp32]` as a 32-bit zero-extended
/// value (`mov r11d`). An index always fits in 32 bits and is non-negative, but
/// its frame slot may be a 4-byte `i32` whose adjacent bytes hold an unrelated
/// value; a 64-bit load would splice that garbage into the high half of the index
/// and compute a wild element address. (Same rationale as the r14-based index
/// load; the length/pointer r15 loads stay 64-bit.)
fn append_load_index_r11_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x45, 0x8b, 0x9f]); // mov r11d, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r11_to_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0x9f]); // mov [r15 + disp32], r11
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_rsi_rax(bytes: &mut Vec<u8>) {
    bytes.extend([0x48, 0x89, 0xc6]); // mov rsi, rax
}

fn append_mov_rdi_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x4c, 0x89, 0xd7]); // mov rdi, r10
}

fn append_rep_movsb(bytes: &mut Vec<u8>) {
    bytes.extend([0xf3, 0xa4]); // rep movsb (copy rcx bytes [rsi]->[rdi], DF=0)
}

fn append_push_rsi_rdi(bytes: &mut Vec<u8>) {
    bytes.extend([0x56, 0x57]); // push rsi ; push rdi
}

fn append_pop_rdi_rsi(bytes: &mut Vec<u8>) {
    bytes.extend([0x5f, 0x5e]); // pop rdi ; pop rsi
}

fn append_mov_r12d_imm32(bytes: &mut Vec<u8>, value: u32) -> Result<(), Diagnostic> {
    bytes.extend([0x41, 0xbc]);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_cmp_r12d_imm32(bytes: &mut Vec<u8>, value: u32) -> Result<(), Diagnostic> {
    bytes.extend([0x41, 0x81, 0xfc]);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_add_r14_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot add offset `{value}` to r14"
        ))
    })?;
    bytes.extend([0x49, 0x81, 0xc6]);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_add_r14_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xde]); // add r14, r11
}

fn append_load_al_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x41, 0x8a, 0x87]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r15_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xbe]); // mov r15, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r15_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xbf]); // mov r15, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_add_r15_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot add offset `{value}` to r15"
        ))
    })?;
    bytes.extend([0x49, 0x81, 0xc7]); // add r15, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_store_r15_to_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0xbe]); // mov [r14 + disp32], r15
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r10_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x96]); // mov r10, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r11_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    // 32-bit load (`mov r11d`), which zero-extends into the full r11. This is the
    // array-INDEX load (every caller follows it with `imul r11, element_scale`).
    // An index is a non-negative array offset that always fits in 32 bits, but its
    // frame slot may be a 4-byte `i32` whose adjacent 4 bytes hold an unrelated
    // value; a 64-bit load would splice that garbage into the high half of the
    // index and compute a wild element address. Reading exactly 4 zero-extended
    // bytes is correct for both `i32` and (small) `usize` indices.
    bytes.extend([0x45, 0x8b, 0x9e]); // mov r11d, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_r15_r14(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x89, 0xf7]); // mov r15, r14
}

fn append_add_r15_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xdf]); // add r15, r11
}

fn append_load_rdx_from_r10(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x92]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r10_from_r10(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x92]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r8_from_r10(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x82]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rax_from_r10(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x82]); // mov rax, [r10 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rax_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rcx_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r14_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xb7]); // mov r14, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r14_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xb6]); // mov r14, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_imul_rax_imm32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend([0x48, 0x69, 0xc0]); // imul rax, rax, imm32
    bytes.extend(value.to_le_bytes());
}

fn append_imul_r11_imm32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend([0x4d, 0x69, 0xdb]); // imul r11, r11, imm32
    bytes.extend(value.to_le_bytes());
}

fn append_imul_r15_imm32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend([0x4d, 0x69, 0xff]); // imul r15, r15, imm32
    bytes.extend(value.to_le_bytes());
}

fn append_add_r14_r15(bytes: &mut Vec<u8>) {
    // add r14, r15 -- REX.W+REX.R(r15)+REX.B(r14)=0x4d, opcode 01, ModRM 11 reg=r15(111) rm=r14(110)=0xfe
    bytes.extend([0x4d, 0x01, 0xfe]);
}

fn append_add_r15_rax(bytes: &mut Vec<u8>) {
    // add r15, rax -- REX.W+REX.B (0x49), opcode 0x01, ModRM 11 reg=rax(000) rm=r15(111) = 0xc7
    bytes.extend([0x49, 0x01, 0xc7]);
}

fn append_add_rax_r11(bytes: &mut Vec<u8>) {
    // add rax, r11 -- REX.W+REX.R (0x4c), opcode 0x01, ModRM 11 reg=r11(011) rm=rax(000) = 0xd8
    bytes.extend([0x4c, 0x01, 0xd8]);
}

fn append_store_r15_to_rax(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x89, 0xb8]); // mov [rax + disp32], r15
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r11_to_rax(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x89, 0x98]); // mov [rax + disp32], r11
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r11_from_rax(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x8b, 0x98]); // mov r11, [rax + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_rax_r15(bytes: &mut Vec<u8>) {
    // mov rax, r15 -- REX.W+REX.R(no)+REX.B(r15 src as r/m): 0x4c 0x89 0xf8
    bytes.extend([0x4c, 0x89, 0xf8]);
}

fn element_scale(element_byte_size: usize) -> Result<i32, Diagnostic> {
    i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale runtime index by element size `{element_byte_size}`"
        ))
    })
}

fn append_load_reg_from_rax(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        // mov r10{b,d,}, [rax + disp32] -- ModRM mod=10 reg=r10(010) rm=rax(000) = 0x90
        (Reg64::R10, 1) => bytes.extend([0x44, 0x8a, 0x90]),
        (Reg64::R10, 4) => bytes.extend([0x44, 0x8b, 0x90]),
        (Reg64::R10, 8) => bytes.extend([0x4c, 0x8b, 0x90]),
        // mov r11{b,d,}, [rax + disp32] -- ModRM mod=10 reg=r11(011) rm=rax(000) = 0x98
        (Reg64::R11, 1) => bytes.extend([0x44, 0x8a, 0x98]),
        (Reg64::R11, 4) => bytes.extend([0x44, 0x8b, 0x98]),
        (Reg64::R11, 8) => bytes.extend([0x4c, 0x8b, 0x98]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime operands yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rax_from_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x8a, 0x86]),
        4 => bytes.extend([0x41, 0x8b, 0x86]),
        8 => bytes.extend([0x49, 0x8b, 0x86]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte storage values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_reg_from_r15(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        (Reg64::R10, 1) => bytes.extend([0x45, 0x8a, 0x97]),
        (Reg64::R10, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x97]),
        (Reg64::R10, 4) => bytes.extend([0x45, 0x8b, 0x97]),
        (Reg64::R10, 8) => bytes.extend([0x4d, 0x8b, 0x97]),
        (Reg64::R11, 1) => bytes.extend([0x45, 0x8a, 0x9f]),
        (Reg64::R11, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x9f]),
        (Reg64::R11, 4) => bytes.extend([0x45, 0x8b, 0x9f]),
        (Reg64::R11, 8) => bytes.extend([0x4d, 0x8b, 0x9f]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime operands yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_rax_to_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x88, 0x87]),
        2 => bytes.extend([0x66, 0x41, 0x89, 0x87]),
        4 => bytes.extend([0x41, 0x89, 0x87]),
        8 => bytes.extend([0x49, 0x89, 0x87]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot store {byte_size}-byte runtime values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r10_to_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x45, 0x88, 0x97]),
        2 => bytes.extend([0x66, 0x45, 0x89, 0x97]),
        4 => bytes.extend([0x45, 0x89, 0x97]),
        8 => bytes.extend([0x4d, 0x89, 0x97]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot store {byte_size}-byte runtime values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r14_to_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0xb7]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        // mov [r14+disp32], r10{b,w,d,} -- ModRM reg=r10, r/m=r14
        1 => bytes.extend([0x45, 0x88, 0x96]),
        2 => bytes.extend([0x66, 0x45, 0x89, 0x96]),
        4 => bytes.extend([0x45, 0x89, 0x96]),
        8 => bytes.extend([0x4d, 0x89, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot store {byte_size}-byte runtime values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_cmp_r10_r11(bytes: &mut Vec<u8>, byte_size: usize) -> Result<(), Diagnostic> {
    match byte_size {
        1 => bytes.extend([0x45, 0x38, 0xda]),
        2 => bytes.extend([0x66, 0x45, 0x39, 0xda]),
        4 => bytes.extend([0x45, 0x39, 0xda]),
        8 => bytes.extend([0x4d, 0x39, 0xda]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot compare {byte_size}-byte runtime values yet"
            )));
        }
    }
    Ok(())
}

fn append_jcc_rel32(
    bytes: &mut Vec<u8>,
    opcode: u8,
    byte_distance: isize,
) -> Result<(), Diagnostic> {
    let displacement = rel32(byte_distance)?;
    bytes.extend([0x0f, opcode]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_jmp_rel32(bytes: &mut Vec<u8>, byte_distance: isize) -> Result<(), Diagnostic> {
    let displacement = rel32(byte_distance)?;
    bytes.push(0xe9);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn runtime_storage_copy_chunk_count(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
) -> usize {
    let mut count = 0;
    let _ = for_each_runtime_copy_chunk(
        source_base_offset,
        target_base_offset,
        byte_count,
        |_, _| {
            count += 1;
            Ok(())
        },
    );
    count
}

fn for_each_runtime_copy_chunk(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
    mut visit: impl FnMut(usize, usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut remaining = byte_count;
    let mut offset = 0usize;

    while remaining > 0 {
        let source_offset = source_base_offset + offset;
        let target_offset = target_base_offset + offset;
        let chunk_size =
            if remaining >= 8 && source_offset.is_multiple_of(8) && target_offset.is_multiple_of(8)
            {
                8
            } else if remaining >= 4
                && source_offset.is_multiple_of(4)
                && target_offset.is_multiple_of(4)
            {
                4
            } else {
                1
            };

        visit(offset, chunk_size)?;
        offset += chunk_size;
        remaining -= chunk_size;
    }

    Ok(())
}

fn load_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 7,
        // The 2-byte form is the 4-byte form plus the 0x66 operand-size prefix.
        2 => 8,
        _ => 0,
    }
}

fn store_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 7,
        // The 2-byte form is the 4-byte form plus the 0x66 operand-size prefix.
        2 => 8,
        _ => 0,
    }
}

fn immediate_i32<T: InstructionOperandLike>(
    operands: &[T],
    index: usize,
    label: &str,
) -> Result<i32, Diagnostic> {
    let Some(operand) = operands.get(index) else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: missing {label}"
        )));
    };
    let Some(value) = operand.immediate_integer() else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} is not an immediate integer"
        )));
    };
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} value {value} does not fit i32"
        ))
    })
}

fn disp32(value: usize) -> Result<i32, Diagnostic> {
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot address displacement `{value}`"
        ))
    })
}

fn rel32(value: isize) -> Result<i32, Diagnostic> {
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 branch target is out of rel32 range: {value} byte(s)"
        ))
    })
}
