use omega_calling_conventions::{HostCapability, HostOperation, HostOperationKey};
use omega_core::arithmetic::ArithmeticDomain;
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

pub fn entry_argument_register_write_width() -> usize {
    // mov r15,imm64(frame base, relocated at +2) (10) + mov [r15+disp32],reg (7).
    17
}

/// The ENTRY PROLOGUE's inbound unmarshal: store an incoming MS-x64 argument
/// register (0=RCX 1=RDX 2=R8 3=R9 -- the order firmware/OS passes the entry's
/// arguments) into the entry parameter's runtime-frame slot. Runs BEFORE
/// anything else at the entry (the argument registers are volatile); this is
/// how a UEFI `main(image_handle, system_table)` receives RCX/RDX
/// (calling_plans.md, the entry stub = the calling plan's inbound direction).
pub fn encode_entry_argument_register_write_bytes(
    argument_index: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(entry_argument_register_write_width());
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the runtime-frame region base
    // mov [r15 + disp32], reg -- REX.W + REX.B (base r15), ModRM mod=10 rm=111,
    // reg = the argument register (r8/r9 add REX.R).
    let (rex, modrm) = match argument_index {
        0 => (0x49, 0x8f), // mov [r15+disp32], rcx
        1 => (0x49, 0x97), // mov [r15+disp32], rdx
        2 => (0x4d, 0x87), // mov [r15+disp32], r8
        3 => (0x4d, 0x8f), // mov [r15+disp32], r9
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 entry prologue supports at most 4 register arguments \
                 (argument index {other}); stack-passed entry arguments are not \
                 implemented"
            )));
        }
    };
    bytes.extend([rex, 0x89, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(bytes.len(), entry_argument_register_write_width());
    Ok(bytes)
}

pub fn entry_arguments_slice_descriptor_write_width() -> usize {
    // mov r15,imm64(frame base, relocated at +2) (10) + lea rax,[r15+spill] (7)
    // + mov [r15+desc],rax (7) + mov qword [r15+desc+8],imm32(len) (11).
    35
}

/// The bytes-handoff half of the entry prologue: bind `args: &[u8]` as a view
/// over the entry-argument spill -- write the slice descriptor
/// {ptr @ desc+0 = frame+spill_offset, len @ desc+8 = byte_length}.
pub fn encode_entry_arguments_slice_descriptor_write_bytes(
    descriptor_offset: usize,
    spill_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(entry_arguments_slice_descriptor_write_width());
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the runtime-frame region base
    bytes.extend([0x49, 0x8d, 0x87]); // lea rax, [r15 + disp32]
    bytes.extend(disp32(spill_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x89, 0x87]); // mov [r15 + disp32], rax
    bytes.extend(disp32(descriptor_offset)?.to_le_bytes());
    bytes.extend([0x49, 0xc7, 0x87]); // mov qword [r15 + disp32], imm32
    bytes.extend(disp32(descriptor_offset + 8)?.to_le_bytes());
    let length = i32::try_from(byte_length)
        .map_err(|_| Diagnostic::error("entry-argument slice length exceeds an imm32"))?;
    bytes.extend(length.to_le_bytes());
    debug_assert_eq!(bytes.len(), entry_arguments_slice_descriptor_write_width());
    Ok(bytes)
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
    // Floats prepend a 6-byte `jp` parity branch before the failure jcc (NaN routing).
    let float_parity_branch = if is_float { 6 } else { 0 };
    10 + load_width + 10 + runtime_float_or_integer_compare_width(is_float, byte_size) + 6 + float_parity_branch
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
    append_failure_branch(&mut bytes, operator, skip_byte_distance - 4, is_float)?;
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
            if operand.runtime_string_is_bounded_buffer() {
                // Owned carrier: content pointer = base + byte_offset + pointer_size.
                bytes.extend([0x49, 0x8d, 0x87]); // lea rax, [r15 + disp32]
                bytes.extend(disp32(byte_offset + 8)?.to_le_bytes());
            } else {
                append_load_rax_from_r15(&mut bytes, byte_offset)?; // rax = descriptor.pointer
            }
            append_mov_syscall_arg_from_rax(&mut bytes, index)?;
        } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
            append_mov_r15_imm64(&mut bytes, 0);
            if operand.runtime_string_is_bounded_buffer() {
                append_load_rax_from_r15(&mut bytes, byte_offset)?; // carrier len @ offset 0
            } else {
                append_load_rax_from_r15(&mut bytes, byte_offset + 8)?; // rax = descriptor.length
            }
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
        (HostCapability::Process, HostOperation::ExitProcess)
        | (HostCapability::Clock, HostOperation::Sleep) => encode_scalar_arg_call(operands),
        // A 0-arg value-returning import through the GENERAL import-call encoder
        // (byte-identical to the original bespoke tick_count sequence for an
        // 8-byte result, and width-correct for a 4-byte one).
        (HostCapability::Clock, HostOperation::TickCount) => {
            encode_win64_import_call(operands, true)
        }
        (HostCapability::Input, HostOperation::KeyState) => encode_key_state_call(operands),
        // Every Gui import is value-returning and encodes through the GENERAL
        // import call: operands[0] = result place, then the full ABI argument
        // list (selection interleaves the hard-wired immediates).
        (HostCapability::Gui, _) => encode_win64_import_call(operands, true),
        _ => Err(Diagnostic::error(format!(
            "X86_64 host operation {}.{} is not implemented",
            operation_key.capability_name(),
            operation_key.operation_name()
        ))),
    }
}

/// `GetAsyncKeyState(vk)` -- a value-returning USER32 import (the multi-DLL
/// proof): shadow space, the vk marshalled into ecx from operands[1] (constant
/// or runtime scalar), the relocated `call rel32`, the shadow restore, then
/// `movzx eax, ax` (the return is a SHORT; zero the undefined upper bits) and
/// the store-rax tail into the result place (operands[0]).
fn encode_key_state_call<T: InstructionOperandLike>(operands: &[T]) -> Result<Vec<u8>, Diagnostic> {
    let Some((_, result_offset, _)) = operands
        .first()
        .and_then(|operand| operand.runtime_scalar_integer())
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 key_state: the result storage place did not lower to a              runtime scalar operand",
        ));
    };
    let mut bytes = Vec::with_capacity(4 + 17 + 5 + 4 + 3 + 17);
    bytes.extend([0x48, 0x83, 0xec, 0x28]); // sub rsp, 40
    match operands.get(1) {
        Some(operand) if operand.runtime_scalar_integer().is_some() => {
            let (_, byte_offset, _) = operand.runtime_scalar_integer().unwrap();
            append_mov_r15_imm64(&mut bytes, 0); // relocated to the vk region base
            bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15 + disp32]
            let displacement: i32 = byte_offset
                .try_into()
                .map_err(|_| Diagnostic::error("key_state vk offset exceeds i32"))?;
            bytes.extend(displacement.to_le_bytes());
        }
        _ => {
            let vk = immediate_i32(operands, 1, "key_state virtual-key argument")?;
            bytes.push(0xb9); // mov ecx, imm32
            bytes.extend(vk.to_le_bytes());
        }
    }
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 (relocated)
    bytes.extend([0x48, 0x83, 0xc4, 0x28]); // add rsp, 40
    bytes.extend([0x0f, 0xb7, 0xc0]); // movzx eax, ax (zero the upper bits)
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the result region base
    bytes.extend([0x49, 0x89, 0x87]); // mov [r15 + disp32], rax
    let displacement: i32 = result_offset
        .try_into()
        .map_err(|_| Diagnostic::error("key_state result offset exceeds i32"))?;
    bytes.extend(displacement.to_le_bytes());
    Ok(bytes)
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
        if operand.runtime_string_is_bounded_buffer() {
            // Owned carrier: the content pointer is the COMPUTED inline-bytes
            // address `base + byte_offset + pointer_size` (lea), not a stored
            // descriptor pointer. Same width as the descriptor-pointer load.
            bytes.extend([0x49, 0x8d, 0x92]); // lea rdx, [r10 + disp32]
            bytes.extend(disp32(byte_offset + 8)?.to_le_bytes());
        } else {
            append_load_rdx_from_r10(bytes, byte_offset)?;
        }
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
        if operand.runtime_string_is_bounded_buffer() {
            // Owned carrier: length is at offset 0 (not the descriptor's len word
            // at offset pointer_size).
            append_load_r8_from_r10(bytes, byte_offset)?;
        } else {
            append_load_r8_from_r10(bytes, byte_offset + 8)?;
        }
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

/// The Win64 integer argument registers, in call order, as
/// (mov-imm32 opcode bytes, load-from-[r15+disp32] opcode bytes) pairs:
/// rcx, rdx, r8, r9. Immediates use the 32-bit `mov r32, imm32` forms (the
/// kernel32 surface is u32-shaped today); loads are 64-bit `mov r64,
/// [r15+disp32]` (callees read the low 32 bits).
const WIN64_ARG_REGISTERS: [(&[u8], &[u8]); 4] = [
    (&[0xb9], &[0x49, 0x8b, 0x8f]),       // mov ecx, imm32 / mov rcx, [r15+d]
    (&[0xba], &[0x49, 0x8b, 0x97]),       // mov edx, imm32 / mov rdx, [r15+d]
    (&[0x41, 0xb8], &[0x4d, 0x8b, 0x87]), // mov r8d, imm32 / mov r8,  [r15+d]
    (&[0x41, 0xb9], &[0x4d, 0x8b, 0x8f]), // mov r9d, imm32 / mov r9,  [r15+d]
];

/// Marshalling width of Win64 scalar argument `index` (0..=3): a constant is a
/// `mov r32, imm32` (5 bytes; 6 with the REX prefix for r8d/r9d), a
/// runtime-storage scalar is `mov r15, imm64=0` (10, relocated to the region
/// base) + `mov r64, [r15+disp32]` (7).
fn win64_scalar_arg_width<T: InstructionOperandLike>(operands: &[T], index: usize) -> usize {
    let (imm_opcode, load_opcode) = WIN64_ARG_REGISTERS[index];
    if operands
        .get(index)
        .is_some_and(|operand| operand.runtime_scalar_integer().is_some())
    {
        10 + load_opcode.len() + 4
    } else {
        imm_opcode.len() + 4
    }
}

/// Total marshalling width of the first `count` Win64 scalar arguments.
fn win64_scalar_args_width<T: InstructionOperandLike>(operands: &[T], count: usize) -> usize {
    (0..count)
        .map(|index| win64_scalar_arg_width(operands, index))
        .sum()
}

/// The GENERAL Win64 scalar-argument call: shadow space, the first `count`
/// operands marshalled into rcx/rdx/r8/r9 (each a constant immediate or a
/// runtime-storage scalar load through a relocated r15 region base), the
/// relocated `call rel32`, then the shadow restore. This is extern-ladder rung
/// 1: every new import mapping with scalar arguments encodes through here, and
/// the pre-existing single-arg kernel32 ops (ExitProcess, Sleep) delegate to it
/// byte-identically.
fn encode_win64_scalar_args_call<T: InstructionOperandLike>(
    operands: &[T],
    count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(13 + win64_scalar_args_width(operands, count));
    bytes.extend([0x48, 0x83, 0xec, 0x28]); // sub rsp, 40
    for index in 0..count {
        let (imm_opcode, load_opcode) = WIN64_ARG_REGISTERS[index];
        match operands.get(index) {
            Some(operand) if operand.runtime_scalar_integer().is_some() => {
                let (_, byte_offset, _) = operand.runtime_scalar_integer().unwrap();
                // mov r15, imm64=0 (relocated to the argument's storage-region
                // base), then mov <reg64>, [r15 + byte_offset].
                append_mov_r15_imm64(&mut bytes, 0);
                bytes.extend_from_slice(load_opcode);
                let displacement: i32 = byte_offset.try_into().map_err(|_| {
                    Diagnostic::error("host call scalar argument offset exceeds i32")
                })?;
                bytes.extend(displacement.to_le_bytes());
            }
            _ => {
                let argument = immediate_i32(operands, index, "host call u32 argument")?;
                bytes.extend_from_slice(imm_opcode);
                bytes.extend(argument.to_le_bytes());
            }
        }
    }
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32
    bytes.extend([0x48, 0x83, 0xc4, 0x28]); // add rsp, 40
    debug_assert_eq!(bytes.len(), 13 + win64_scalar_args_width(operands, count));
    Ok(bytes)
}

/// Relocation sites for a `encode_win64_scalar_args_call` sequence: one
/// Absolute64 region-base site per runtime-scalar argument (inside its
/// `mov r15, imm64`) plus the Relative32 `call rel32` site after all
/// marshalling.
fn win64_scalar_args_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
    count: usize,
) -> Vec<X86_64RelocationSite> {
    let mut sites = Vec::new();
    let mut cursor = 4usize; // past sub rsp, 40
    for index in 0..count {
        if operands
            .get(index)
            .is_some_and(|operand| operand.runtime_scalar_integer().is_some())
        {
            sites.push(X86_64RelocationSite {
                operand_index: Some(index),
                byte_offset: cursor + 2, // inside mov r15, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_scalar_arg_width(operands, index);
    }
    sites.push(X86_64RelocationSite {
        operand_index: None,
        byte_offset: cursor + 1, // past the call opcode
        byte_width: 4,
        kind: X86_64RelocationSiteKind::Relative32,
    });
    sites
}

/// A kernel32 call taking a single u32 first argument in ecx and no return:
/// `ExitProcess(code)` and `Sleep(ms)` are the same shape. Shadow space, the arg
/// marshalled from a constant or a runtime-storage scalar, the relocated call, then
/// the shadow restore (no-op for ExitProcess, which never returns).
fn encode_scalar_arg_call<T: InstructionOperandLike>(operands: &[T]) -> Result<Vec<u8>, Diagnostic> {
    encode_win64_scalar_args_call(operands, 1)
}

/// `lea <reg64>, [r15+disp32]` opcode bytes for the Win64 integer argument
/// registers rcx/rdx/r8/r9 -- `WIN64_ARG_REGISTERS`' load opcodes with the mov
/// (8B) swapped for lea (8D), byte-for-byte the same width.
const WIN64_ARG_LEA_OPCODES: [&[u8]; 4] = [
    &[0x49, 0x8d, 0x8f], // lea rcx, [r15+d]
    &[0x49, 0x8d, 0x97], // lea rdx, [r15+d]
    &[0x4d, 0x8d, 0x87], // lea r8,  [r15+d]
    &[0x4d, 0x8d, 0x8f], // lea r9,  [r15+d]
];

/// The outgoing stack-argument area starts right above the 32-byte shadow space.
const WIN64_STACK_ARG_HOME: usize = 32;

/// The stack reservation for a general Win64 import call with `arg_count`
/// arguments: the 32-byte shadow space plus one 8-byte outgoing slot per
/// argument past the 4 register args, padded so rsp stays 16-byte aligned at
/// the `call` (the emitted code runs with rsp ≡ 8 mod 16 -- the invariant the
/// existing 40-byte no-stack-arg reservation encodes).
fn win64_import_reserve(arg_count: usize) -> usize {
    let stack_slots = arg_count.saturating_sub(4);
    let mut reserve = WIN64_STACK_ARG_HOME + 8 * stack_slots;
    if reserve % 16 == 0 {
        reserve += 8;
    }
    reserve
}

/// `sub/add rsp, imm` width: the imm8 form (4 bytes) up to 127, else imm32 (7).
fn rsp_adjust_width(reserve: usize) -> usize {
    if reserve <= 127 { 4 } else { 7 }
}

fn append_sub_rsp(bytes: &mut Vec<u8>, reserve: usize) {
    if reserve <= 127 {
        bytes.extend([0x48, 0x83, 0xec, reserve as u8]); // sub rsp, imm8
    } else {
        bytes.extend([0x48, 0x81, 0xec]); // sub rsp, imm32
        bytes.extend((reserve as u32).to_le_bytes());
    }
}

fn append_add_rsp(bytes: &mut Vec<u8>, reserve: usize) {
    if reserve <= 127 {
        bytes.extend([0x48, 0x83, 0xc4, reserve as u8]); // add rsp, imm8
    } else {
        bytes.extend([0x48, 0x81, 0xc4]); // add rsp, imm32
        bytes.extend((reserve as u32).to_le_bytes());
    }
}

/// Register-indirect near call -- `call r/m64` in the `FF /2` register-DIRECT
/// form: optional `REX.B` (0x41) for r8-r15, `FF`, then ModRM `11 010 rrr`
/// (`0xD0 | (reg & 7)`; mod=11 register-direct, reg=`/2`=010, rm=the register).
/// The target is a POINTER VALUE already in `reg`, NOT an import relocation --
/// this is the runtime-pointer call the first-boot path needs (UEFI
/// `SystemTable -> ConOut -> OutputString` is three pointer hops) and the same
/// emission a `VtableSlot` dispatch will use (extern brief §12.4). `reg` is the
/// x86_64 register number 0..=15 (0=rax..7=rdi, 8=r8..15=r15).
///
fn append_call_register(bytes: &mut Vec<u8>, reg: u8) {
    debug_assert!(reg < 16, "x86_64 register number out of range");
    if reg >= 8 {
        bytes.push(0x41); // REX.B extends ModRM.rm into r8-r15
    }
    bytes.push(0xff);
    bytes.push(0xd0 | (reg & 0x7)); // ModRM: mod=11 reg=/2(010) rm=reg
}

/// Whether a general-import argument operand marshals through the relocated r15
/// region base (a runtime-storage scalar LOAD or a runtime-storage ADDRESS lea)
/// rather than as a constant immediate.
fn win64_import_arg_is_staged<T: InstructionOperandLike>(operand: Option<&T>) -> bool {
    operand.is_some_and(|operand| {
        operand.runtime_scalar_integer().is_some() || operand.runtime_storage_address().is_some()
    })
}

/// Marshalling width of general-import argument `index` (0-based ABI order,
/// stored at `operands[arg_start + index]`). Register args mirror
/// `win64_scalar_arg_width` (an address lea is the same width as a scalar
/// load); stack args stage through r15/rax (10 + 7 + a 5-byte
/// `mov [rsp+disp8], rax`), or store a constant directly (9-byte
/// `mov qword [rsp+disp8], imm32`).
fn win64_import_arg_width<T: InstructionOperandLike>(
    operands: &[T],
    arg_start: usize,
    index: usize,
) -> usize {
    let staged = win64_import_arg_is_staged(operands.get(arg_start + index));
    if index < 4 {
        let (imm_opcode, load_opcode) = WIN64_ARG_REGISTERS[index];
        if staged {
            10 + load_opcode.len() + 4
        } else {
            imm_opcode.len() + 4
        }
    } else if staged {
        10 + 7 + 5
    } else {
        9
    }
}

/// Total width of a `encode_win64_import_call` sequence -- must mirror the
/// encoder byte for byte (the relocation cursor math depends on it).
fn win64_import_call_width<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> usize {
    let arg_start = usize::from(returns_value);
    let arg_count = operands.len().saturating_sub(arg_start);
    let reserve = win64_import_reserve(arg_count);
    let mut width = 2 * rsp_adjust_width(reserve) + 5;
    for index in 0..arg_count {
        width += win64_import_arg_width(operands, arg_start, index);
    }
    if returns_value {
        width += 17; // mov r15, imm64 (10) + mov [r15+disp32], eax/rax (7)
    }
    width
}

/// A host-call immediate encoded into a 32-bit field: accepts the i32 range AND
/// the u32 range (DWORD flag words like `WS_POPUP|WS_VISIBLE` = 0x9000_0000),
/// encoding the low 32 bits. Register args use `mov r32, imm32` (zero-extends);
/// stack slots use `mov qword, imm32` (SIGN-extends -- correct for ints and for
/// DWORD-consuming callees, so keep pointer-sized big constants out of stack
/// slots).
fn immediate_imm32<T: InstructionOperandLike>(
    operands: &[T],
    index: usize,
    label: &str,
) -> Result<i32, Diagnostic> {
    let Some(value) = operands.get(index).and_then(|operand| operand.immediate_integer()) else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} did not lower to a marshallable operand"
        )));
    };
    if value < i64::from(i32::MIN) || value > i64::from(u32::MAX) {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} value {value} does not fit a 32-bit immediate"
        )));
    }
    Ok(value as u32 as i32)
}

/// The GENERAL Win64 import call -- the full extern-ABI shape. Marshals the
/// argument operands into rcx/rdx/r8/r9 then the outgoing stack slots
/// `[rsp + 32 + 8k]`, emits the relocated `call rel32`, restores the stack
/// reservation, and (for a value-returning import) stores rax into the result
/// place at the result's declared width (4-byte results store eax -- an int
/// return's upper 32 bits are undefined under Win64).
///
/// Operand roles: when `returns_value`, `operands[0]` is the RESULT place (a
/// runtime scalar; its byte_count picks the store width) and the arguments
/// follow; otherwise every operand is an argument. Each argument is a constant
/// immediate, a runtime-storage scalar (loaded through the relocated r15 region
/// base), or a runtime-storage ADDRESS (`lea` through the same base -- the
/// pointer-argument shape: buffers, OS structs, C strings).
/// Marshal MS-x64 call arguments `operands[arg_start..]` into RCX/RDX/R8/R9
/// (staged runtime loads/leas through the relocated r15 region base, or plain
/// immediates) and the shadow-space stack home for args past the fourth.
/// Shared by the import call and the vtable call (their only difference is how
/// the callee address is obtained: a relocated `call rel32` vs `call rax`).
fn append_win64_call_arguments<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operands: &[T],
    arg_start: usize,
) -> Result<(), Diagnostic> {
    let arg_count = operands.len() - arg_start;
    for index in 0..arg_count {
        let operand = &operands[arg_start + index];
        if index < 4 {
            let (imm_opcode, load_opcode) = WIN64_ARG_REGISTERS[index];
            if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
                append_mov_r15_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend_from_slice(load_opcode);
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
            } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
                append_mov_r15_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend_from_slice(WIN64_ARG_LEA_OPCODES[index]);
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
            } else {
                let argument = immediate_imm32(operands, arg_start + index, "call argument")?;
                bytes.extend_from_slice(imm_opcode);
                bytes.extend(argument.to_le_bytes());
            }
        } else {
            let stack_offset = WIN64_STACK_ARG_HOME + 8 * (index - 4);
            let stack_disp8 = u8::try_from(stack_offset)
                .ok()
                .filter(|_| stack_offset <= 127)
                .ok_or_else(|| Diagnostic::error("X86_64 call supports at most 16 arguments"))?;
            if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
                append_mov_r15_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15+disp32]
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
                bytes.extend([0x48, 0x89, 0x44, 0x24, stack_disp8]); // mov [rsp+o], rax
            } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
                append_mov_r15_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend([0x49, 0x8d, 0x87]); // lea rax, [r15+disp32]
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
                bytes.extend([0x48, 0x89, 0x44, 0x24, stack_disp8]); // mov [rsp+o], rax
            } else {
                let argument = immediate_imm32(operands, arg_start + index, "call argument")?;
                bytes.extend([0x48, 0xc7, 0x44, 0x24, stack_disp8]); // mov qword [rsp+o], imm32
                bytes.extend(argument.to_le_bytes());
            }
        }
    }
    Ok(())
}

fn encode_win64_import_call<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if returns_value && operands.is_empty() {
        return Err(Diagnostic::error(
            "cannot encode X86_64 import call: the result storage place did not lower to a \
             runtime scalar operand",
        ));
    }
    let arg_start = usize::from(returns_value);
    let arg_count = operands.len() - arg_start;
    let reserve = win64_import_reserve(arg_count);
    let mut bytes = Vec::with_capacity(win64_import_call_width(operands, returns_value));
    append_sub_rsp(&mut bytes, reserve);
    append_win64_call_arguments(&mut bytes, operands, arg_start)?;
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 (relocated)
    append_add_rsp(&mut bytes, reserve);
    if returns_value {
        let Some((_, byte_offset, byte_count)) = operands[0].runtime_scalar_integer() else {
            return Err(Diagnostic::error(
                "cannot encode X86_64 import call: the result storage place did not lower to a \
                 runtime scalar operand",
            ));
        };
        append_mov_r15_imm64(&mut bytes, 0); // relocated to the result region base
        match byte_count {
            4 => bytes.extend([0x41, 0x89, 0x87]), // mov [r15+disp32], eax
            8 => bytes.extend([0x49, 0x89, 0x87]), // mov [r15+disp32], rax
            other => {
                return Err(Diagnostic::error(format!(
                    "X86_64 import call cannot store a {other}-byte result (expected 4 or 8)"
                )));
            }
        }
        bytes.extend(disp32(byte_offset)?.to_le_bytes());
    }
    debug_assert_eq!(
        bytes.len(),
        win64_import_call_width(operands, returns_value)
    );
    Ok(bytes)
}

/// Relocation sites for a `encode_win64_import_call` sequence: one Absolute64
/// region-base site per staged argument (inside its `mov r15, imm64`), the
/// Relative32 `call rel32` after all marshalling, and (value-returning) the
/// result region base inside the store tail's `mov r15, imm64`.
fn win64_import_call_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Vec<X86_64RelocationSite> {
    let arg_start = usize::from(returns_value);
    let arg_count = operands.len().saturating_sub(arg_start);
    let reserve = win64_import_reserve(arg_count);
    let mut sites = Vec::new();
    let mut cursor = rsp_adjust_width(reserve);
    for index in 0..arg_count {
        if win64_import_arg_is_staged(operands.get(arg_start + index)) {
            sites.push(X86_64RelocationSite {
                operand_index: Some(arg_start + index),
                byte_offset: cursor + 2, // inside mov r15, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_import_arg_width(operands, arg_start, index);
    }
    sites.push(X86_64RelocationSite {
        operand_index: None,
        byte_offset: cursor + 1, // past the call opcode
        byte_width: 4,
        kind: X86_64RelocationSiteKind::Relative32,
    });
    cursor += 5 + rsp_adjust_width(reserve);
    if returns_value
        && operands
            .first()
            .is_some_and(|operand| operand.runtime_scalar_integer().is_some())
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2, // inside the result mov r15, imm64
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
    }
    sites
}

/// A VtableSlot call (extern brief §12.1): marshal the declared args MS-x64
/// (this -> RCX, then RDX/R8/R9), then read the callee from the RECEIVER --
/// `mov rax, [rcx + index*8]; call rax`. The protocol struct IS the vtable
/// (UEFI SimpleTextOutput: OutputString at slot 1 = +8). No result store
/// (void), no import thunk, no call relocation (the target is a runtime
/// pointer). The receiver (arg 0) must already sit in RCX -- so it is a plain
/// register arg like any other; the `mov rax, [rcx..]` reads it back.
pub fn encode_win64_vtable_call<T: InstructionOperandLike>(
    operands: &[T],
    index: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if operands.is_empty() {
        return Err(Diagnostic::error(
            "cannot encode X86_64 vtable call: the receiver (arg 0) did not lower to an operand",
        ));
    }
    let arg_count = operands.len();
    let reserve = win64_import_reserve(arg_count);
    let mut bytes = Vec::with_capacity(win64_vtable_call_width(operands, index));
    append_sub_rsp(&mut bytes, reserve);
    append_win64_call_arguments(&mut bytes, operands, 0)?;
    // Read the callee from the receiver (still in RCX) and call it.
    let slot_disp = i32::try_from(index.checked_mul(8).ok_or_else(|| {
        Diagnostic::error("vtable slot index overflows a byte offset")
    })?)
    .map_err(|_| Diagnostic::error("vtable slot offset exceeds an imm32"))?;
    bytes.extend([0x48, 0x8b, 0x81]); // mov rax, [rcx + disp32]
    bytes.extend(slot_disp.to_le_bytes());
    append_call_register(&mut bytes, 0); // call rax
    append_add_rsp(&mut bytes, reserve);
    debug_assert_eq!(bytes.len(), win64_vtable_call_width(operands, index));
    Ok(bytes)
}

pub fn win64_vtable_call_width<T: InstructionOperandLike>(operands: &[T], _index: i64) -> usize {
    let arg_count = operands.len();
    let reserve = win64_import_reserve(arg_count);
    let mut width = rsp_adjust_width(reserve);
    for index in 0..arg_count {
        width += win64_import_arg_width(operands, 0, index);
    }
    width += 7; // mov rax, [rcx + disp32]
    width += 2; // call rax (no REX.B for rax)
    width += rsp_adjust_width(reserve);
    width
}

/// The region-base fixup byte offset for vtable-call argument `operand_index`
/// (the `mov r15, imm64` imm), matching `encode_win64_vtable_call`'s layout.
pub fn vtable_call_data_relocation_byte_offset<T: InstructionOperandLike>(
    operands: &[T],
    operand_index: usize,
) -> usize {
    win64_vtable_call_relocation_sites(operands)
        .into_iter()
        .find(|site| site.operand_index == Some(operand_index))
        .map(|site| site.byte_offset)
        .unwrap_or(0)
}

/// Relocation sites for a vtable call: the staged-argument region bases only
/// (no call relocation -- the callee is a runtime pointer read from RCX).
pub fn win64_vtable_call_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    let reserve = win64_import_reserve(operands.len());
    let mut sites = Vec::new();
    let mut cursor = rsp_adjust_width(reserve);
    for index in 0..operands.len() {
        if win64_import_arg_is_staged(operands.get(index)) {
            sites.push(X86_64RelocationSite {
                operand_index: Some(index),
                byte_offset: cursor + 2, // inside mov r15, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_import_arg_width(operands, 0, index);
    }
    sites
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
        (HostCapability::Process, HostOperation::ExitProcess)
        | (HostCapability::Clock, HostOperation::Sleep) => {
            // Single-u32-arg kernel32 call (ExitProcess/Sleep), re-expressed
            // through the general Win64 scalar-args helper (extern rung 1).
            win64_scalar_args_relocation_sites(operands, 1)
        }
        (HostCapability::Input, HostOperation::KeyState) => {
            // Layout: sub(4) + vk marshalling (17 runtime / 5 const) + call(5)
            // + add(4) + movzx(3) + mov r15,imm64(10) + store(7).
            let vk_is_runtime = operands
                .get(1)
                .is_some_and(|operand| operand.runtime_scalar_integer().is_some());
            let vk_width = if vk_is_runtime { 17 } else { 5 };
            let mut sites = Vec::new();
            if vk_is_runtime {
                sites.push(X86_64RelocationSite {
                    operand_index: Some(1),
                    byte_offset: 4 + 2, // inside the vk mov r15, imm64
                    byte_width: 8,
                    kind: X86_64RelocationSiteKind::Absolute64,
                });
            }
            sites.push(X86_64RelocationSite {
                operand_index: None,
                byte_offset: 4 + vk_width + 1, // past the call opcode
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            });
            sites.push(X86_64RelocationSite {
                operand_index: Some(0),
                byte_offset: 4 + vk_width + 5 + 4 + 3 + 2, // inside the result mov r15, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
            sites
        }
        (HostCapability::Clock, HostOperation::TickCount) => {
            // 0-arg value-returning call through the general import-call layout
            // (call at 4+1; result-region base at 13+2 -- identical to the
            // original bespoke site list).
            win64_import_call_relocation_sites(operands, true)
        }
        (HostCapability::Gui, _) => {
            // Value-returning general import calls (mirrors the encode arm).
            win64_import_call_relocation_sites(operands, true)
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
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4, false)?;
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
    // Floats prepend a 6-byte `jp` parity branch before the failure jcc (NaN routing).
    let float_parity_branch = if is_float { 6 } else { 0 };
    10 + load_width
        + 10
        + load_width
        + runtime_float_or_integer_compare_width(is_float, byte_size)
        + 6
        + float_parity_branch
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
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4, is_float)?;
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
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4, false)?;
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
            append_load_index_eax_from_r10(&mut bytes, index_offset)?;
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            append_load_index_eax_from_r15(&mut bytes, index_offset)?;
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

/// Width of [`encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage`]
/// for the machine-resident-index case. MUST equal the emitter exactly. The
/// two relocations are the machine source base (@+2, the instruction start) and
/// the target base (@+36); there is NO runtime-frame load, so a program without
/// any frame storage relocates cleanly.
pub fn runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width() -> usize {
    // mov r15,imm64 (10) + mov eax,[r15+idx] (7) + imul rax,rax,imm32 (7)
    // + add r15,rax (3) + mov rax,[r15+disp] (7) + mov r15,imm64 (10)
    // + store [r15+disp] (7).
    51
}

/// Read `collection[index]` (an element of a machine-resident inline array,
/// indexed by a runtime field) and copy it into a runtime-storage target -- the
/// mirror of [`encode_runtime_machine_indexed_integer_write`] in the read
/// direction. Only a machine-resident index is implemented; a frame-resident
/// index (`let i = ..; self.arr[i]`) is a clean error for now.
pub fn encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
    base_byte_offset: usize,
    index_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot read {byte_count}-byte machine indexed values yet"
        )));
    }
    if index_region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "X86_64 MVP encoder cannot read a machine indexed value with a frame-resident index yet",
        ));
    }
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale machine index by element size `{element_byte_size}`"
        ))
    })?;
    let index_displacement = disp32(index_offset)?;
    let mut bytes = Vec::with_capacity(
        runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(),
    );
    // r15 = machine source base (imm64 at +2 relocated to the machine symbol).
    append_mov_r15_imm64(&mut bytes, 0);
    // eax = index, loaded 32-bit (zero-extended, so a nonzero adjacent slot can't
    // splice into it) from the machine storage base.
    bytes.extend([0x41, 0x8b, 0x87]); // mov eax, [r15+disp32]
    bytes.extend(index_displacement.to_le_bytes());
    // rax = index * element_byte_size; r15 = source base + scaled index.
    append_imul_rax_imm32(&mut bytes, element_scale);
    append_add_r15_rax(&mut bytes);
    // rax = the source element at [r15 + base + field].
    append_load_rax_from_r15(&mut bytes, base_byte_offset + field_byte_offset)?;
    // r15 = target base (imm64 at +36 relocated to the target region symbol);
    // store the low byte_count bytes of rax there.
    append_mov_r15_imm64(&mut bytes, 0);
    append_store_rax_to_r15(&mut bytes, target_offset, byte_count)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width()
    );
    Ok(bytes)
}

pub fn runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage_width(
    source_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    match source_region {
        // mov r15,imm64 (10) + load rax,[r15+src] (7) + SECOND mov r15,imm64
        // (10, the machine base) + mov r10d,[r15+idx] (7) + imul (7) + add (3)
        // + store (7): the source rides the FRAME base first.
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => 51,
        // mov r15,imm64 (10) + mov rax,[r15+src] (7) + mov r10d,[r15+idx] (7)
        // + imul r10,r10,imm32 (7) + add r15,r10 (3) + store [r15+disp] (7).
        _ => 41,
    }
}

/// Start of the SECOND `mov r15,imm64` (the machine base) inside the
/// frame-source variant of the write half -- the machine relocation; the
/// relocation planner adds the +2 immediate offset itself.
pub fn runtime_storage_copy_to_runtime_machine_indexed_frame_source_machine_base_offset() -> usize {
    17
}

/// Write a runtime-storage value into `collection[index]` (an element of a
/// machine-resident inline array, indexed by a runtime field) -- the mirror of
/// [`encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage`]
/// in the write direction (`self.nums[self.j] = self.b`). Only a machine-resident
/// source AND a machine-resident index are implemented (the common case where
/// every field shares the machine base); a frame-resident source or index is a
/// clean error for now.
pub fn encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    index_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot write {byte_count}-byte machine indexed values yet"
        )));
    }
    if index_region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "X86_64 MVP encoder cannot write a machine indexed value with a frame-resident index yet",
        ));
    }
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale machine index by element size `{element_byte_size}`"
        ))
    })?;
    let index_displacement = disp32(index_offset)?;
    let mut bytes = Vec::with_capacity(
        runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage_width(source_region),
    );
    // r15 = the SOURCE base (imm64 at +2, relocated to the source region's
    // symbol -- the machine for a field source, the runtime frame for a
    // slot-backed local); rax = the source value off that clean base.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_rax_from_r15(&mut bytes, source_offset)?;
    if source_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        // Frame-resident source: re-load r15 with the MACHINE base (imm64 at
        // +17+2, the second relocation) for the index read + element store.
        append_mov_r15_imm64(&mut bytes, 0);
    }
    // r10d = index, loaded 32-bit (zero-extended) from the machine base.
    bytes.extend([0x45, 0x8b, 0x97]); // mov r10d, [r15+disp32]
    bytes.extend(index_displacement.to_le_bytes());
    // r10 = index * element_byte_size.
    bytes.extend([0x4d, 0x69, 0xd2]); // imul r10, r10, imm32
    bytes.extend(element_scale.to_le_bytes());
    // r15 = machine base + scaled index = target element base.
    bytes.extend([0x4d, 0x01, 0xd7]); // add r15, r10
    // store the low byte_count bytes of rax at [r15 + base + field].
    append_store_rax_to_r15(&mut bytes, base_byte_offset + field_byte_offset, byte_count)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage_width(source_region)
    );
    Ok(bytes)
}

pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_width() -> usize {
    // Read part: mov r15,imm64 (10) + mov eax,[r15+idx] (7) + imul rax,imm32 (7)
    // + add r15,rax (3) + load rax,[r15+disp] (7) = 34.
    // Write part: mov r15,imm64 (10) + mov r10d,[r15+idx] (7) + imul r10,imm32
    // (7) + add r15,r10 (3) + store [r15+disp] (7) = 34.
    68
}

/// The relative offset of the WRITE part's `mov r15, imm64` immediate inside
/// [`encode_runtime_storage_copy_machine_indexed_to_machine_indexed`] -- the
/// second machine-base relocation (the first sits at instruction start +2).
pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset() -> usize {
    34 + 2
}

/// The DUAL-indexed copy `arr[i] = arr[j]` (task #38): read a machine-owned
/// runtime-indexed SOURCE element, store it into a machine-owned runtime-indexed
/// TARGET element. Byte-for-byte composition of the two proven halves -- the
/// read front of
/// [`encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage`]
/// (eax = source index, scale, add, load rax) and the write tail of
/// [`encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage`]
/// (r10d = target index, scale, add, store rax). The value rides in rax across
/// the re-load of the machine base into r15, and the two index computations use
/// distinct registers (rax before the load vs r10), so nothing clobbers. Both
/// indices must be machine-resident (the same gate as the halves).
pub fn encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
    source_base_byte_offset: usize,
    source_index_offset: usize,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    source_element_byte_size: usize,
    source_field_byte_offset: usize,
    target_base_byte_offset: usize,
    target_index_offset: usize,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
    target_element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot copy {byte_count}-byte machine indexed values yet"
        )));
    }
    if source_index_region != omega_target_operations::RuntimeStorageRegion::Machine
        || target_index_region != omega_target_operations::RuntimeStorageRegion::Machine
    {
        return Err(Diagnostic::error(
            "X86_64 MVP encoder cannot copy machine indexed values with a frame-resident index yet",
        ));
    }
    let source_scale = i32::try_from(source_element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale machine index by element size `{source_element_byte_size}`"
        ))
    })?;
    let target_scale = i32::try_from(target_element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale machine index by element size `{target_element_byte_size}`"
        ))
    })?;
    let source_index_displacement = disp32(source_index_offset)?;
    let target_index_displacement = disp32(target_index_offset)?;
    let mut bytes =
        Vec::with_capacity(runtime_storage_copy_machine_indexed_to_machine_indexed_width());
    // READ PART. r15 = machine base (imm64 at +2 relocated to the machine
    // symbol); eax = source index (32-bit, zero-extended); scale; walk r15 to
    // the source element; rax = the element.
    append_mov_r15_imm64(&mut bytes, 0);
    bytes.extend([0x41, 0x8b, 0x87]); // mov eax, [r15+disp32]
    bytes.extend(source_index_displacement.to_le_bytes());
    append_imul_rax_imm32(&mut bytes, source_scale);
    append_add_r15_rax(&mut bytes);
    append_load_rax_from_r15(
        &mut bytes,
        source_base_byte_offset + source_field_byte_offset,
    )?;
    // WRITE PART. r15 = machine base again (imm64 at +36, the second machine
    // relocation); r10d = target index; scale; walk r15 to the target element;
    // store the low byte_count bytes of rax.
    append_mov_r15_imm64(&mut bytes, 0);
    bytes.extend([0x45, 0x8b, 0x97]); // mov r10d, [r15+disp32]
    bytes.extend(target_index_displacement.to_le_bytes());
    bytes.extend([0x4d, 0x69, 0xd2]); // imul r10, r10, imm32
    bytes.extend(target_scale.to_le_bytes());
    bytes.extend([0x4d, 0x01, 0xd7]); // add r15, r10
    append_store_rax_to_r15(
        &mut bytes,
        target_base_byte_offset + target_field_byte_offset,
        byte_count,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_machine_indexed_to_machine_indexed_width()
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
    is_bounded_buffer: bool,
) -> Result<(Vec<u8>, RuntimeTextLineReadLayout), Diagnostic> {
    let target_ptr_disp = disp32(target_offset)?;
    let target_len_disp = disp32(target_offset + 8)?;
    // Owned carrier: r14 must point at the inline bytes (`region + target_offset +
    // pointer_size`), so the imm64 relocates to the carrier's own region and an
    // `add` advances past the leading 8-byte length word.
    let carrier_bytes_disp = disp32(target_offset + 8)?;
    let mut bytes = Vec::with_capacity(128);

    // r14 = read buffer (imm64 at +2 relocated to the buffer data symbol, OR to
    // the carrier's own region for an owned `[u8; N]` target).
    bytes.extend([0x49, 0xbe]);
    bytes.extend(0u64.to_le_bytes());
    if is_bounded_buffer {
        // add r14, target_offset + pointer_size -> r14 = carrier inline bytes.
        bytes.extend([0x49, 0x81, 0xc6]);
        bytes.extend(carrier_bytes_disp.to_le_bytes());
    }
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

    let target_mov_offset = if is_bounded_buffer {
        // Owned carrier: the bytes are already in place (r14 read straight into the
        // inline storage). Write only the length at `[r14 - 8]` (= region +
        // target_offset, the leading len word). No `{ptr, len}` descriptor, hence
        // no second relocation.
        bytes.extend([0x4d, 0x89, 0x7e, 0xf8]); // mov [r14-8], r15
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

fn runtime_text_line_read_layout_for(is_bounded_buffer: bool) -> RuntimeTextLineReadLayout {
    // Capacity/target do not affect the layout (all immediates are fixed width),
    // so encode once with placeholders to recover the authoritative offsets.
    build_runtime_text_line_read(0, 1, is_bounded_buffer)
        .expect("runtime text line read layout encodes")
        .1
}

fn runtime_text_line_read_layout() -> RuntimeTextLineReadLayout {
    runtime_text_line_read_layout_for(false)
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
    runtime_text_line_read_layout_for(true).width
}

pub fn runtime_text_line_read_carrier_get_std_handle_call_offset() -> usize {
    runtime_text_line_read_layout_for(true).get_std_handle_call_offset
}

pub fn runtime_text_line_read_carrier_read_file_call_offset() -> usize {
    runtime_text_line_read_layout_for(true).read_file_call_offset
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
    Ok(build_runtime_text_line_read_syscall(target_offset, capacity, number, false)?.0)
}

/// Linux `read(2)` line read into an owned `[u8; N]` carrier: stdin bytes land in
/// the carrier's inline storage and the line length is written to its leading
/// length word; no `{ptr, len}` descriptor.
pub fn encode_runtime_text_line_read_syscall_carrier(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read_syscall(target_offset, capacity, number, true)?.0)
}

fn build_runtime_text_line_read_syscall(
    target_offset: usize,
    capacity: u32,
    number: u32,
    is_bounded_buffer: bool,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    let target_ptr_disp = disp32(target_offset)?;
    let target_len_disp = disp32(target_offset + 8)?;
    let carrier_bytes_disp = disp32(target_offset + 8)?;
    let mut bytes = Vec::with_capacity(128);

    // r14 = read buffer (imm64 at +2 relocated to the buffer data symbol, OR to the
    // carrier's own region for an owned `[u8; N]` target); r15 = length.
    bytes.extend([0x49, 0xbe]);
    bytes.extend(0u64.to_le_bytes());
    if is_bounded_buffer {
        // add r14, target_offset + pointer_size -> r14 = carrier inline bytes.
        bytes.extend([0x49, 0x81, 0xc6]);
        bytes.extend(carrier_bytes_disp.to_le_bytes());
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
    let target_mov_offset = if is_bounded_buffer {
        // Owned carrier: the bytes are already in place; write only the length at
        // `[r14 - 8]` (the leading len word). No `{ptr, len}` descriptor.
        bytes.extend([0x4d, 0x89, 0x7e, 0xf8]); // mov [r14-8], r15
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

fn runtime_text_line_read_syscall_layout_for(is_bounded_buffer: bool) -> (usize, usize) {
    // Capacity/number/target are all fixed-width immediates, so they do not affect the
    // layout; encode once with placeholders to recover the width + target imm offset.
    let (bytes, target_mov_offset) =
        build_runtime_text_line_read_syscall(0, 1, 0, is_bounded_buffer)
            .expect("runtime text line read syscall layout encodes");
    (bytes.len(), target_mov_offset)
}

fn runtime_text_line_read_syscall_layout() -> (usize, usize) {
    runtime_text_line_read_syscall_layout_for(false)
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
    runtime_text_line_read_syscall_layout_for(true).0
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
    Ok(build_runtime_text_line_read(target_offset, capacity, false)?.0)
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
    Ok(build_runtime_text_line_read(target_offset, capacity, true)?.0)
}

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

/// Byte offset of the WRITTEN page mov inside both wire appends (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_append_written_page_offset(_out_offset: usize) -> usize {
    17
}

/// Byte offset of the SOURCE page mov inside the varint append AND the
/// text-bytes append (both materialize the source page right after the shared
/// prologue).
pub fn wire_append_varint_source_page_offset(
    _out_offset: usize,
    _written_offset: usize,
) -> usize {
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

/// The fixed LEB128 read loop + fail tail (see the decoder body).
fn wire_varint_read_loop_width() -> usize {
    56
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
) -> usize {
    // Prologue + success/value/shift init (10) + read loop + optional
    // unzigzag + target imm64 (10) + truncating store (7) + epilogue.
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 10
        + 7
        + wire_decode_tail_width()
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
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x31, 0xc0]); // xor eax, eax (value)
    bytes.extend([0x31, 0xc9]); // xor ecx, ecx (shift)

    // LEB128 read loop (fixed 56 bytes, `wire_varint_read_loop_width`):
    //   loop: cmp  rcx, 63
    //         ja   fail            (+47: overlong varint, >10 groups)
    //         cmp  r10, imm32(length)
    //         jae  fail            (+38: truncated input)
    //         movzx r11d, byte [r15]
    //         inc  r15
    //         inc  r10
    //         mov  r8, r11
    //         and  r8, 0x7f
    //         shl  r8, cl
    //         or   rax, r8
    //         add  rcx, 7
    //         test r11, 0x80       (continuation bit)
    //         jnz  loop            (-51)
    //         jmp  done            (+3: skip the fail xor)
    //   fail: xor  r9d, r9d
    //   done:
    bytes.extend([0x48, 0x83, 0xf9, 0x3f]); // cmp rcx, 63
    bytes.extend([0x77, 0x2f]); // ja +47 -> fail
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x26]); // jae +38 -> fail
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
    bytes.extend([0xeb, 0x03]); // jmp +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

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
            zigzag
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
) -> Result<Vec<u8>, Diagnostic> {
    // The region only picks the relocation symbol; the shape is identical.
    let _ = target_region;

    let mut bytes = Vec::with_capacity(read_wire_byte_slice_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        target_offset,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1 (ok)
    bytes.extend([0x31, 0xc0]); // xor eax, eax (length)
    bytes.extend([0x31, 0xc9]); // xor ecx, ecx (shift)

    // Identical LEB128 read loop to the scalar decoder: rax = length, r15 now
    // points at the CONTENT (just past the length varint), r10 = cursor.
    bytes.extend([0x48, 0x83, 0xf9, 0x3f]); // cmp rcx, 63
    bytes.extend([0x77, 0x2f]); // ja +47 -> fail
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x26]); // jae +38 -> fail
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
    bytes.extend([0xeb, 0x03]); // jmp +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (fail: clear ok)

    // Bounds + advance (fixed 21 bytes): end = cursor + len; if end >
    // buffer_length clear ok; cursor = end.
    bytes.extend([0x4d, 0x89, 0xd0]); // mov r8, r10  (r8 = cursor)
    bytes.extend([0x49, 0x01, 0xc0]); // add r8, rax  (r8 = cursor + len = end)
    bytes.extend([0x49, 0x81, 0xf8]); // cmp r8, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x76, 0x03]); // jbe +3 (skip clear when end <= length)
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (content overruns -> clear ok)
    bytes.extend([0x4d, 0x89, 0xc2]); // mov r10, r8 (advance cursor to end)

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
            target_offset
        )
    );
    Ok(bytes)
}

pub fn read_wire_byte_slice_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _target_offset: usize,
) -> usize {
    // Prologue + success/value/shift init (10) + read loop + bounds&advance (21)
    // + target imm64 (10) + ptr store (7) + len store (7) + epilogue.
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + 21
        + 10
        + 7
        + 7
        + wire_decode_tail_width()
}

/// Byte offset of the TARGET page mov inside the byte-slice decode (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_decode_byte_slice_target_page_offset(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
) -> usize {
    wire_decode_prologue_width() + 10 + wire_varint_read_loop_width() + 21
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
// when its compile-time element index is below the count-companion slot's
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

/// LEB128-encode element `index` of a packed repeated field at the cursor,
/// ONLY IF `index < count` (the count-companion slot, read as unsigned
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
    let skip_rel8 = i8::try_from(skip_distance)
        .expect("the guarded append body is well under the rel8 range");
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
pub fn wire_append_repeated_count_page_offset(
    _out_offset: usize,
    _written_offset: usize,
) -> usize {
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
        + 7
        + wire_repeated_read_count_bump_width()
        + wire_decode_tail_width()
}

/// LEB128-read one packed repeated element at the cursor into the target
/// slot, ONLY IF the cursor sits strictly below the end bound the
/// surrounding nested OPEN stored; the taken path also increments the
/// count-companion slot. A skipped read changes nothing -- the jump lands
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
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    // Guard: r8 = the end-slot page (imm64 relocated at the nested end page
    // offset), rax = the absolute end bound stored there; skip everything
    // (including the epilogue) when cursor >= end.
    let skip_distance = 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 10
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

    bytes.extend([0x48, 0x83, 0xf9, 0x3f]); // cmp rcx, 63
    bytes.extend([0x77, 0x2f]); // ja +47 -> fail
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x26]); // jae +38 -> fail
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
    bytes.extend([0xeb, 0x03]); // jmp +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

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
            zigzag
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
) -> usize {
    wire_decode_repeated_target_page_offset(
        buffer_offset,
        buffer_length,
        read_offset,
        end_offset,
        zigzag,
    ) + 10
        + 7
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

// Write a string literal into an owned `[u8; N]` bounded byte carrier at machine
// storage (`{len, bytes}` inline). r15 = machine storage base (reloc @ +2); store
// `len` (the literal length) as the leading 8-byte word at [r15 + byte_offset],
// then copy each literal byte inline at [r15 + byte_offset + 8 + i] as an
// immediate. The carrier OWNS its bytes (a value), unlike the String descriptor
// which stores a {ptr -> rodata, len}. Content is immediate, so the ONLY
// relocation is the base address (the leading `mov r15, imm64`).
pub fn runtime_machine_bounded_buffer_write_width(literal: &str) -> usize {
    // mov r15,imm64 (10) + mov rax,imm64 (10) + store rax->[r15+off] 8B (7) = 27,
    // then per content byte: mov byte [r15 + disp32], imm8 (8).
    27 + literal.len() * 8
}

// Write a string literal into an owned `[u8; N]` carrier reached THROUGH a stored
// pointer (`rooms[0].label = "Gate"`): load the pointer from `frame[ptr]` into r15,
// then store `len` + the literal bytes inline at `*ptr + field`. Content is
// immediate, so the ONLY relocation is the base (the leading `mov r15, imm64`).
pub fn runtime_pointee_bounded_buffer_write_width(literal: &str) -> usize {
    // mov r15,imm64 (10) + mov r15,[r15+ptr] (7) + mov rax,imm64 (10)
    // + store rax->[r15+field] 8B (7), then per content byte:
    // mov byte [r15 + disp32], imm8 (8) = 34 + 8*len
    34 + literal.len() * 8
}

pub fn encode_runtime_pointee_bounded_buffer_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_bounded_buffer_write_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // frame/machine base (reloc @ +2)
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?; // r15 = stored pointer
    append_mov_rax_imm64(&mut bytes, literal.len() as u64);
    append_store_rax_to_r15(&mut bytes, field_byte_offset, 8)?; // [*ptr + field] = len word
    for (index, byte) in literal.as_bytes().iter().enumerate() {
        let displacement = disp32(field_byte_offset + 8 + index)?;
        bytes.extend([0x41, 0xc6, 0x87]); // mov byte [r15 + disp32], imm8
        bytes.extend(displacement.to_le_bytes());
        bytes.push(*byte);
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_bounded_buffer_write_width(literal)
    );
    Ok(bytes)
}

pub fn encode_runtime_machine_bounded_buffer_write(
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_write_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // machine storage base (reloc @ +2)
    append_mov_rax_imm64(&mut bytes, literal.len() as u64);
    append_store_rax_to_r15(&mut bytes, byte_offset, 8)?; // [base + off] = len word
    for (index, byte) in literal.as_bytes().iter().enumerate() {
        let displacement = disp32(byte_offset + 8 + index)?;
        bytes.extend([0x41, 0xc6, 0x87]); // mov byte [r15 + disp32], imm8
        bytes.extend(displacement.to_le_bytes());
        bytes.push(*byte);
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_write_width(literal)
    );
    Ok(bytes)
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
    let mut bytes =
        Vec::with_capacity(runtime_machine_bounded_buffer_source_append_width(source_in_frame));
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
pub fn runtime_machine_bounded_buffer_literal_append_width(literal: &str) -> usize {
    // mov r15,imm64 (10) + mov rax,[r15+t] (7) + lea rdi,[r15+t+8] (7)
    // + add rdi,rax (3) + per byte: mov byte [rdi+disp8],imm8 (4)
    // + add rax,imm32 (`48 05`+imm32 = 6) + mov [r15+t],rax (7) = 40 + 4*len
    40 + 4 * literal.len()
}

pub fn encode_runtime_machine_bounded_buffer_literal_append(
    target_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let target = disp32(target_byte_offset)?;
    let target_bytes = disp32(target_byte_offset + 8)?;
    let literal_bytes = literal.as_bytes();
    let literal_len = u32::try_from(literal_bytes.len()).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 encoder cannot append a carrier literal of {} bytes",
            literal_bytes.len()
        ))
    })?;
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_literal_append_width(literal));
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

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> usize {
    // The integer op is normally the default 64-bit op; Saturating/Trapping
    // instead emit a width-correct add/sub followed by the clamp/trap sequence.
    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );
    let operation_width = if saturating_or_trapping && operator == StateGuardOperator::Multiply {
        saturating_trapping_multiply_width(domain, byte_size, target_signed)
    } else if saturating_or_trapping
        && matches!(
            operator,
            StateGuardOperator::Add | StateGuardOperator::Subtract
        )
    {
        width_integer_add_sub_width(byte_size)
            + arithmetic_domain_clamp_width(domain, operator, byte_size, target_signed)
    } else if domain == ArithmeticDomain::Saturating
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Saturating SIGNED divide/modulo wraps the normal idiv in a TYPE_MIN/-1
        // guard (see append_saturating_signed_divide_modulo).
        saturating_signed_divide_modulo_width(byte_size, operator == StateGuardOperator::Modulo)
    } else if domain == ArithmeticDomain::Wrapping
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Wrapping SIGNED divide/modulo guards TYPE_MIN/-1 so idiv does not #DE
        // (see append_wrapping_signed_divide_modulo). Unsigned uses the *Unsigned
        // operators and cannot overflow, so it falls through.
        wrapping_signed_divide_modulo_width(byte_size, operator == StateGuardOperator::Modulo)
    } else {
        // Trapping div/mod (idiv traps == Trapping semantics), Exact (proven
        // non-overflowing), and unsigned div/mod (cannot overflow) use the normal
        // op width.
        runtime_binary_operation_or_float_width(operator, byte_size, is_float)
    };
    // 10 (mov r14,imm64) + left + push r10 (2) + right + mov r11,r10 (3)
    // + pop r10 (2) + operation + store.
    10 + runtime_value_operand_width(runtime_value_operands, left)
        + 2
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3
        + 2
        + operation_width
        + 7.max(store_width(byte_size))
}

/// Bytes of [`append_saturating_signed_divide_modulo`], for the relocation layout.
/// MUST equal the emitter exactly. cmp r11,-1 (4) + jne (2) + the divisor==-1
/// fixup + jmp (2) + the normal idiv core (the plain signed op width).
fn saturating_signed_divide_modulo_width(byte_size: usize, want_remainder: bool) -> usize {
    let fixup = if want_remainder {
        3 // xor r10d, r10d
    } else if byte_size <= 2 {
        16 // neg r10d (3) + mov r9d,imm32 (6) + cmp r10d,r9d (3) + cmovg r10d,r9d (4)
    } else if byte_size <= 4 {
        13 // neg r10d (3) + mov r9d,imm32 (6) + cmovo r10d,r9d (4)
    } else {
        17 // neg r10 (3) + mov r9,imm64 (10) + cmovo r10,r9 (4)
    };
    let normal = runtime_binary_operation_width(
        if want_remainder {
            StateGuardOperator::Modulo
        } else {
            StateGuardOperator::Divide
        },
        byte_size,
    );
    4 + 2 + fixup + 2 + normal
}

/// Bytes of [`append_wrapping_signed_divide_modulo`], for the relocation layout.
/// MUST equal the emitter exactly. cmp r11,-1 (4) + jne (2) + the divisor==-1
/// fixup (always 3: `neg r10` for divide, `xor r10d,r10d` for modulo) + jmp (2) +
/// the normal idiv core.
fn wrapping_signed_divide_modulo_width(byte_size: usize, want_remainder: bool) -> usize {
    let fixup = 3; // neg r10/r10d, or xor r10d,r10d
    let normal = runtime_binary_operation_width(
        if want_remainder {
            StateGuardOperator::Modulo
        } else {
            StateGuardOperator::Divide
        },
        byte_size,
    );
    4 + 2 + fixup + 2 + normal
}

/// Bytes of [`append_width_integer_add_sub`]: 4 for 16-bit (0x66 prefix), else 3.
fn width_integer_add_sub_width(byte_size: usize) -> usize {
    if byte_size == 2 { 4 } else { 3 }
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

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
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
    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );
    if is_float {
        append_runtime_float_binary_operation(&mut bytes, operator, byte_size)?;
    } else if saturating_or_trapping && operator == StateGuardOperator::Multiply {
        // Saturating/Trapping multiply: a 64-bit `imul` yields the EXACT product
        // for <=32-bit operands (it cannot exceed 64 bits), so compare the full
        // product against the target type's range and clamp / trap.
        append_saturating_trapping_multiply(&mut bytes, domain, byte_size, target_signed)?;
    } else if saturating_or_trapping
        && matches!(
            operator,
            StateGuardOperator::Add | StateGuardOperator::Subtract
        )
    {
        // Decision 17: the default integer path does a 64-bit op and lets the
        // store truncate (== Wrapping). Saturating/Trapping instead need the
        // overflow flags to reflect the TARGET width, so emit a width-correct
        // add/sub here and then clamp (Saturating) or trap (Trapping).
        append_width_integer_add_sub(&mut bytes, operator, byte_size)?;
        append_arithmetic_domain_clamp(&mut bytes, domain, operator, byte_size, target_signed)?;
    } else if domain == ArithmeticDomain::Saturating
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Saturating SIGNED divide/modulo: clamp the one overflowing corner
        // (TYPE_MIN / -1) to TYPE_MAX / 0 instead of trapping. The UNSIGNED variants
        // cannot overflow, so they are absent from this arm and fall through to the
        // normal path below. (Trapping div/mod also falls through, where `idiv`
        // traps on overflow and divide-by-zero -- exactly Trapping semantics.)
        append_saturating_signed_divide_modulo(
            &mut bytes,
            byte_size,
            operator == StateGuardOperator::Modulo,
        )?;
    } else if domain == ArithmeticDomain::Wrapping
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Wrapping SIGNED divide/modulo: guard TYPE_MIN / -1 so the bare `idiv`
        // does not raise #DE -- produce the WRAPPED result (TYPE_MIN / 0) instead.
        // Unsigned div/mod uses the *Unsigned operators (cannot overflow) and
        // falls through to the normal path below.
        append_wrapping_signed_divide_modulo(
            &mut bytes,
            byte_size,
            operator == StateGuardOperator::Modulo,
        )?;
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

/// Bytes of [`append_saturating_trapping_multiply`], for the relocation layout.
/// MUST equal what that function emits.
fn saturating_trapping_multiply_width(
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
) -> usize {
    let imul = 4; // imul r10, r11
    if !matches!(byte_size, 1 | 2 | 4) {
        return imul; // emission errors; width is irrelevant then
    }
    // Two sign-extension instructions for signed narrow operands (see emission):
    // movsx is 4 bytes (8/16-bit), movsxd is 3 bytes (32-bit).
    let sign_extend = if target_signed {
        if byte_size == 4 { 6 } else { 8 }
    } else {
        0
    };
    let clamp = match (domain, target_signed) {
        // mov r11,imm64 (10) + cmp r10,r11 (3) + cmova r10,r11 (4)
        (ArithmeticDomain::Saturating, false) => 17,
        // (mov + cmp + cmovg) + (mov + cmp + cmovl)
        (ArithmeticDomain::Saturating, true) => 34,
        // mov (10) + cmp (3) + jbe rel8 (2) + ud2 (2)
        (ArithmeticDomain::Trapping, false) => 17,
        // mov (10) + cmp (3) + jg (2) + mov (10) + cmp (3) + jge (2) + ud2 (2)
        (ArithmeticDomain::Trapping, true) => 32,
        _ => 0,
    };
    imul + sign_extend + clamp
}

/// Saturating/Trapping multiply (decision 17). A 64-bit `imul r10, r11` produces
/// the EXACT product for <=32-bit operands (the product cannot exceed 64 bits),
/// so the full result is range-compared against the target type and clamped
/// (Saturating) or trapped (Trapping). 64-bit targets are not handled (the
/// product can exceed 64 bits -- needs the 128-bit `mul`/`imul` form). r11 (the
/// spent right operand) is the clamp-constant scratch.
fn append_saturating_trapping_multiply(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
) -> Result<(), Diagnostic> {
    if byte_size == 8 {
        return Err(Diagnostic::error(
            "saturating/trapping multiply on 64-bit integers is not implemented yet \
             (the product can exceed 64 bits, which needs the 128-bit multiply form)"
                .to_owned(),
        ));
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return Err(Diagnostic::error(format!(
            "saturating/trapping multiply cannot handle {byte_size}-byte targets yet"
        )));
    }
    // The 64-bit `imul` needs full-width-correct operands. Narrow operands are
    // loaded ZERO-extended, so a SIGNED negative value (e.g. i8 -50 -> 0xCE = 206)
    // would multiply wrong. Sign-extend r10/r11 from the target width to 64 bits
    // first. (Unsigned operands are already correct zero-extended.)
    if target_signed {
        match byte_size {
            1 => {
                bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]); // movsx r10, r10b
                bytes.extend([0x4d, 0x0f, 0xbe, 0xdb]); // movsx r11, r11b
            }
            2 => {
                bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]); // movsx r10, r10w
                bytes.extend([0x4d, 0x0f, 0xbf, 0xdb]); // movsx r11, r11w
            }
            4 => {
                bytes.extend([0x4d, 0x63, 0xd2]); // movsxd r10, r10d
                bytes.extend([0x4d, 0x63, 0xdb]); // movsxd r11, r11d
            }
            _ => {}
        }
    }
    bytes.extend([0x4d, 0x0f, 0xaf, 0xd3]); // imul r10, r11 (64-bit)
    let unsigned_max: u64 = (1u64 << (8 * byte_size)) - 1;
    let signed_min = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
    let signed_max = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
    fn mov_r11(bytes: &mut Vec<u8>, value: u64) {
        bytes.push(0x49);
        bytes.push(0xbb);
        bytes.extend(value.to_le_bytes());
    }
    match (domain, target_signed) {
        (ArithmeticDomain::Saturating, false) => {
            mov_r11(bytes, unsigned_max);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x4d, 0x0f, 0x47, 0xd3]); // cmova r10, r11 (r10 >u max -> max)
        }
        (ArithmeticDomain::Saturating, true) => {
            mov_r11(bytes, signed_max);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x4d, 0x0f, 0x4f, 0xd3]); // cmovg r10, r11 (> imax -> imax)
            mov_r11(bytes, signed_min);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x4d, 0x0f, 0x4c, 0xd3]); // cmovl r10, r11 (< imin -> imin)
        }
        (ArithmeticDomain::Trapping, false) => {
            mov_r11(bytes, unsigned_max);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x76, 0x02]); // jbe +2 (<= max: ok)
            bytes.extend([0x0f, 0x0b]); // ud2
        }
        (ArithmeticDomain::Trapping, true) => {
            mov_r11(bytes, signed_max);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x7f, 0x0f]); // jg +15 -> ud2 (skip mov+cmp+jge)
            mov_r11(bytes, signed_min);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x7d, 0x02]); // jge +2 (>= imin: ok)
            bytes.extend([0x0f, 0x0b]); // ud2
        }
        _ => {}
    }
    Ok(())
}

/// Width-correct integer `add`/`sub` of `r10 (op)= r11` so the carry/overflow
/// flags reflect the TARGET byte width (the default binary op is always 64-bit
/// and relies on the truncating store). Only `+`/`-` are supported for the
/// saturating/trapping domains today; other operators error.
fn append_width_integer_add_sub(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    // ADD r/m,r = 0x00 (8-bit) / 0x01 (wider); SUB = 0x28 / 0x29. ModRM 0xDA is
    // (r/m = r10, reg = r11); the REX prefix selects the width and extends both.
    let (op8, opw) = match operator {
        StateGuardOperator::Add => (0x00u8, 0x01u8),
        StateGuardOperator::Subtract => (0x28u8, 0x29u8),
        _ => {
            return Err(Diagnostic::error(
                "saturating/trapping arithmetic is only implemented for + and - so far".to_owned(),
            ));
        }
    };
    match byte_size {
        1 => bytes.extend([0x45, op8, 0xda]),
        2 => bytes.extend([0x66, 0x45, opw, 0xda]),
        4 => bytes.extend([0x45, opw, 0xda]),
        8 => bytes.extend([0x4d, opw, 0xda]),
        _ => {
            return Err(Diagnostic::error(format!(
                "saturating/trapping arithmetic cannot handle {byte_size}-byte targets yet"
            )));
        }
    }
    Ok(())
}

/// Bytes of [`append_arithmetic_domain_clamp`], for the relocation layout. MUST
/// equal what that function emits.
fn arithmetic_domain_clamp_width(
    domain: ArithmeticDomain,
    _operator: StateGuardOperator,
    _byte_size: usize,
    target_signed: bool,
) -> usize {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => 0,
        // jno/jnc rel8 (2) + ud2 (2)
        ArithmeticDomain::Trapping => 4,
        ArithmeticDomain::Saturating => {
            if target_signed {
                // mov r11,imm64 (10) + mov r9,imm64 (10) + cmovs r11,r9 (4) + cmovo r10,r11 (4)
                28
            } else {
                // mov r11,imm64 (10) + cmovc r10,r11 (4)
                14
            }
        }
    }
}

/// Clamp (Saturating) or trap (Trapping) the width-correct op's result in r10,
/// reading the flags it set. Unsigned overflow is the carry flag (add: clamp to
/// the unsigned max; sub: clamp to 0); signed overflow is the overflow flag
/// (clamp to the signed min/max, chosen by the result's sign bit). r11 (the
/// spent right operand) and r9 are used as scratch.
fn append_arithmetic_domain_clamp(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
) -> Result<(), Diagnostic> {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {}
        ArithmeticDomain::Trapping => {
            // Skip the 2-byte ud2 when there was NO overflow: unsigned watches the
            // carry flag (jnc/jae), signed watches the overflow flag (jno).
            let skip_when_ok = if target_signed { 0x71u8 } else { 0x73u8 };
            bytes.extend([skip_when_ok, 0x02, 0x0f, 0x0b]);
        }
        ArithmeticDomain::Saturating if target_signed => {
            let imin = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
            let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
            bytes.push(0x49);
            bytes.push(0xbb);
            bytes.extend(imin.to_le_bytes()); // mov r11, IMIN
            bytes.push(0x49);
            bytes.push(0xb9);
            bytes.extend(imax.to_le_bytes()); // mov r9, IMAX
            // On signed overflow the stored result's sign is inverted, so a
            // negative result means the true value overflowed POSITIVE -> IMAX.
            bytes.extend([0x4d, 0x0f, 0x48, 0xd9]); // cmovs r11, r9
            bytes.extend([0x4d, 0x0f, 0x40, 0xd3]); // cmovo r10, r11
        }
        ArithmeticDomain::Saturating => {
            let clamp_value: u64 = match operator {
                StateGuardOperator::Add => {
                    if byte_size >= 8 {
                        u64::MAX
                    } else {
                        (1u64 << (8 * byte_size)) - 1
                    }
                }
                StateGuardOperator::Subtract => 0,
                _ => {
                    return Err(Diagnostic::error(
                        "saturating arithmetic is only implemented for + and - so far".to_owned(),
                    ));
                }
            };
            bytes.push(0x49);
            bytes.push(0xbb);
            bytes.extend(clamp_value.to_le_bytes()); // mov r11, clamp
            bytes.extend([0x4d, 0x0f, 0x42, 0xd3]); // cmovc r10, r11
        }
    }
    Ok(())
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
            // Widen a narrow integer source into r10. A 1/2-byte source was loaded
            // with movb/movw, which leave the upper bits GARBAGE, so it MUST be
            // movzx/movsx-extended (zero for unsigned, sign for signed). A 4-byte
            // source was loaded with movl (already zero-extended), so only a SIGNED
            // 4-byte source needs movsxd; an unsigned 4-byte source is already
            // correct. Narrowing/equal widths need nothing (the store truncates).
            if target_byte_size > source_byte_size {
                match source_byte_size {
                    1 | 2 => 4, // movzx/movsx r10, r10b / r10w
                    4 if source_signed => 3, // movsxd r10, r10d
                    _ => 0,
                }
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
            if target_byte_size > source_byte_size {
                match (source_byte_size, source_signed) {
                    // movb/movw left the upper bits garbage: extend r10b/r10w -> r10.
                    (1, true) => bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]), // movsx r10, r10b
                    (1, false) => bytes.extend([0x4d, 0x0f, 0xb6, 0xd2]), // movzx r10, r10b
                    (2, true) => bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]), // movsx r10, r10w
                    (2, false) => bytes.extend([0x4d, 0x0f, 0xb7, 0xd2]), // movzx r10, r10w
                    (4, true) => bytes.extend([0x4d, 0x63, 0xd2]),       // movsxd r10, r10d
                    // 4-byte unsigned (and 8-byte) sources were already zero-extended
                    // by the movl/movq load.
                    _ => {}
                }
            }
        }
    }
}

pub fn runtime_atomic_fetch_add_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    // mov r14,imm64(target base) (10) + delta operand load into r10 + lock xadd.
    10 + runtime_value_operand_width(runtime_value_operands, delta)
        + lock_xadd_r10_to_r14_width(byte_size)
}

/// Atomic `fetch_add`: hold the target base in r14 (untouched by operand
/// evaluation, which reloads r15), evaluate `delta` into r10, then `lock xadd
/// [r14+offset], r10` -- one atomic read-modify-write of the place. The prior
/// value (left in r10 by xadd) is discarded here; the desugar's preceding
/// `let old = place` captured it separately.
pub fn encode_atomic_fetch_add(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    delta: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_add_width(
        runtime_value_operands,
        byte_size,
        delta,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // target base (imm64 @ +2 relocated)
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, delta)?;
    append_lock_xadd_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_add_width(runtime_value_operands, byte_size, delta)
    );
    Ok(bytes)
}

pub fn runtime_atomic_compare_exchange_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    // mov r14,imm64(base) (10) + new_value load (r10) + push r10 + expected load
    // (r10) + mov rax,r10 + pop r10 + lock cmpxchg. The push/pop stash mirrors
    // the binary write so operand evaluation (which accumulates in r10) cannot
    // clobber the other operand; `new_value` is the "left" at the fixed offset 10
    // and `expected` the "right" after the push gap.
    10 + runtime_value_operand_width(runtime_value_operands, new_value)
        + BINARY_RIGHT_OPERAND_PUSH_WIDTH
        + runtime_value_operand_width(runtime_value_operands, expected)
        + MOV_RAX_R10_WIDTH
        + BINARY_RIGHT_OPERAND_PUSH_WIDTH
        + lock_cmpxchg_r10_to_r14_width(byte_size)
}

/// Atomic `compare_exchange`: hold the target base in r14, evaluate `new_value`
/// into r10 and stash it on the stack, evaluate `expected` into r10 and move it
/// to rax, restore `new_value` into r10, then `lock cmpxchg [r14+offset], r10`.
/// CMPXCHG compares rax (expected) with the place and swaps in r10 (new_value)
/// only on equality; the returned prior (left in rax) is discarded -- the
/// desugar's preceding `let prior = place` captured it. The stash mirrors the
/// binary write because operand evaluation accumulates in r10.
pub fn encode_atomic_compare_exchange(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_compare_exchange_width(
        runtime_value_operands,
        byte_size,
        expected,
        new_value,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // target base (imm64 @ +2 relocated)
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, new_value)?;
    append_push_r10(&mut bytes); // stash new_value across the expected eval
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, expected)?;
    append_mov_rax_r10(&mut bytes); // expected -> rax (CMPXCHG's implicit accumulator)
    append_pop_r10(&mut bytes); // restore new_value -> r10
    append_lock_cmpxchg_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_compare_exchange_width(
            runtime_value_operands,
            byte_size,
            expected,
            new_value
        )
    );
    Ok(bytes)
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

pub fn runtime_machine_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    _index_region: omega_target_operations::RuntimeStorageRegion,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    // Byte layout is identical to the frame-base binary write; only the base
    // relocation targets the machine symbol (handled by the relocations crate).
    // The frame-resident-index case errors in the encoder before width matters,
    // so a single width keeps the function total.
    runtime_frame_base_indexed_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let store_displacement = base_byte_offset + field_byte_offset;
    let mut bytes = Vec::with_capacity(runtime_machine_indexed_binary_write_width(
        runtime_value_operands,
        index_region,
        byte_size,
        left,
        operator,
        right,
    ));
    // r14 = machine storage base + index*element (target address held across
    // operand evaluation, which freely clobbers r15/r10/r11 but never r14). The
    // imm64 at +2 is relocated to the MACHINE symbol (relocations crate), unlike
    // the frame-base sibling that relocates to the frame symbol.
    append_mov_r14_imm64(&mut bytes, 0);
    // Load the index. Only a machine-resident index is implemented (the index is
    // read from the machine base already in r14, matching the frame-base sibling
    // which reads the frame-resident index from the frame base in r14). A
    // frame-resident index (`let i = ..; self.arr[i]`) is a clean error for now.
    match index_region {
        omega_target_operations::RuntimeStorageRegion::Machine => {
            append_load_index_r15d_from_r14(&mut bytes, index_offset)?;
        }
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            return Err(Diagnostic::error(
                "X86_64 MVP encoder cannot write a machine-indexed binary with a frame-resident index yet".to_string(),
            ));
        }
    }
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

/// Fixed width of a value-position text-equals operand (the `TextEquals` arm
/// of `append_runtime_value_operand`): two relocated descriptor-base imm64
/// movs (10 each) with two 7-byte disp32 descriptor word loads apiece, then a
/// fixed 39-byte length-compare + bounded byte loop block and the 3-byte
/// result mov. MUST stay in lockstep with that encoder (it ends with a
/// `debug_assert_eq!` against this function) and with
/// `RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET` below.
pub fn runtime_text_equals_operand_width() -> usize {
    (10 + 7 + 7) + (10 + 7 + 7) + 39 + 3
}

/// Byte offset of the RIGHT descriptor's base `mov r15, imm64` inside a
/// text-equals operand (the relocation planner adds the +2 imm offset itself).
pub const RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET: usize = 10 + 7 + 7;

/// Width of a guard-position text-vs-literal content compare operand (the
/// `TextEqualsLiteral` arm of `append_runtime_value_operand`): the place's
/// descriptor-address setup (13 bytes for a storage base, 17 for a pointee or
/// fixed-indexed deref, 30 for a frame-base-indexed element address, 34 for a
/// frame-indexed element address, each starting with the relocated
/// `mov r15, imm64`), then a fixed 30-byte head (two disp32 descriptor word
/// loads, result zero, length compare + branch), one 13-byte disp32 byte
/// compare + branch per literal byte, and the fixed 9-byte tail (equal-result
/// mov + result move into the destination). MUST stay in lockstep with that
/// encoder (it ends with a `debug_assert_eq!` against this function).
pub fn runtime_text_equals_literal_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    place: RuntimeValueOperandHandle,
    literal: &str,
) -> usize {
    let place_setup_width = if runtime_value_operands.storage(place).is_some() {
        // mov r15,imm64 (10) + mov rax,r15 (3)
        13
    } else if runtime_value_operands.pointee(place).is_some() {
        // mov r15,imm64 (10) + mov rax,[r15+ptr_off] (7)
        17
    } else if runtime_value_operands.frame_indexed(place).is_some() {
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7) + mov r11,[r15+idx] (7)
        // + imul r11,r11,elem (7) + add rax,r11 (3)
        34
    } else if runtime_value_operands.frame_base_indexed(place).is_some() {
        // mov r15,imm64 (10) + mov r11,[r15+idx] (7) + imul r11,r11,elem (7)
        // + mov rax,r15 (3) + add rax,r11 (3)
        30
    } else if runtime_value_operands.frame_fixed_indexed(place).is_some() {
        // Constant element index folds into the descriptor displacement:
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7)
        17
    } else {
        // Selection only builds this operand over storage/pointee/indexed
        // text places; the encoder rejects anything else with a hard
        // diagnostic before this width could be compared against emitted
        // bytes.
        0
    };
    place_setup_width + 30 + 13 * literal.len() + 9
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
    } else if runtime_value_operands.text_equals(operand).is_some() {
        runtime_text_equals_operand_width()
    } else if let Some((place, literal, _is_bounded_buffer)) =
        runtime_value_operands.text_equals_literal(operand)
    {
        // Carrier vs descriptor place are byte-width identical, so the width is
        // independent of `is_bounded_buffer`.
        runtime_text_equals_literal_operand_width(runtime_value_operands, place, &literal)
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        let operation_width = if runtime_value_operands.binary_is_float(operand) {
            // Float operands: the SSE op (movq xmm<-r, op, movq r<-xmm) is a fixed
            // width regardless of operator. MUST match the emission below or the
            // recorded relocation offsets drift (silent runtime segfault).
            runtime_float_binary_operation_width()
        } else {
            // Use the SAME byte_size the emission picks (runtime_binary_operation_byte_size):
            // div/mod run at the operand width so a negative i32 dividend is handled
            // correctly, which changes the idiv/div core length -- the width MUST track
            // it or relocation offsets drift (silent segfault). Other ops keep 64-bit.
            runtime_binary_operation_width(
                operator,
                runtime_binary_operation_byte_size(runtime_value_operands, operator, left, right, 8),
            )
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
    } else if let Some((_, left_offset, _, right_offset)) =
        runtime_value_operands.text_equals(operand)
    {
        append_runtime_text_equals_operand(bytes, destination, left_offset, right_offset)?;
        Ok(())
    } else if let Some((place, literal, place_is_bounded_buffer)) =
        runtime_value_operands.text_equals_literal(operand)
    {
        append_runtime_text_equals_literal_operand(
            runtime_value_operands,
            bytes,
            destination,
            place,
            &literal,
            place_is_bounded_buffer,
        )?;
        Ok(())
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
            // bits (addss/addsd/...) rather than an integer add over them. The width
            // is threaded from build time (set once from the operands' scalar type),
            // so f32 picks `addss`/`movss` (4) and f64 picks `addsd`/`movsd` (8) —
            // no longer hardcoded. The encoded length is identical for both widths
            // (runtime_float_binary_operation_width() == 19), so relocation offsets
            // are unaffected.
            let byte_width = runtime_value_operands.binary_byte_width(operand).unwrap_or(8);
            append_runtime_float_binary_operation(bytes, operator, byte_width)?;
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

/// Value-position text content equality: `destination = (left == right)` as
/// bool 0/1, where both sides are `{ptr @ +0, len @ +8}` text descriptors at
/// relocated region bases. FIXED-WIDTH (`runtime_text_equals_operand_width`):
/// every descriptor word loads through a disp32 form, keeping the relocation
/// offsets (left base mov at the operand start, right base mov at
/// `RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET`) pinned.
///
/// Register use: r15 = descriptor base, then the right length, then the byte
/// scratch in the loop; rax/rcx = left ptr/len, rdx = right ptr, r9 = the
/// bool result (moved into `destination` last). r12/r13/r14 stay untouched
/// (dispatch state and the binary-write shapes' target base live there).
fn append_runtime_text_equals_operand(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    left_offset: usize,
    right_offset: usize,
) -> Result<(), Diagnostic> {
    let operand_start = bytes.len();

    // Left descriptor: base (imm64 relocated at the operand start), ptr, len.
    append_mov_r15_imm64(bytes, 0);
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15+disp32] (left ptr)
    bytes.extend(disp32(left_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15+disp32] (left len)
    bytes.extend(disp32(left_offset + 8)?.to_le_bytes());

    // Right descriptor: base relocated at the pinned right-base offset; the
    // length load consumes r15 LAST (the base is no longer needed after it).
    debug_assert_eq!(
        bytes.len() - operand_start,
        RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
        "right descriptor base must sit at the pinned relocation offset"
    );
    append_mov_r15_imm64(bytes, 0);
    bytes.extend([0x49, 0x8b, 0x97]); // mov rdx, [r15+disp32] (right ptr)
    bytes.extend(disp32(right_offset)?.to_le_bytes());
    bytes.extend([0x4d, 0x8b, 0xbf]); // mov r15, [r15+disp32] (right len)
    bytes.extend(disp32(right_offset + 8)?.to_le_bytes());

    // result = 0; unequal lengths are unequal text. The jne also means a
    // zero-length pair never enters the loop, so an all-zero (default)
    // descriptor's null pointer is never dereferenced. Fixed 39-byte block:
    //         xor   r9d, r9d
    //         cmp   rcx, r15
    //         jne   done            (+31)
    //   loop: test  rcx, rcx
    //         je    equal           (+20: all bytes matched)
    //         movzx r15d, byte [rax]
    //         cmp   r15b, [rdx]
    //         jne   done            (+17)
    //         inc   rax
    //         inc   rdx
    //         dec   rcx
    //         jmp   loop            (-25)
    //  equal: mov   r9d, 1
    //   done:
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
    bytes.extend([0x4c, 0x39, 0xf9]); // cmp rcx, r15
    bytes.extend([0x75, 0x1f]); // jne +31 -> done
    bytes.extend([0x48, 0x85, 0xc9]); // test rcx, rcx
    bytes.extend([0x74, 0x14]); // je +20 -> equal
    bytes.extend([0x44, 0x0f, 0xb6, 0x38]); // movzx r15d, byte [rax]
    bytes.extend([0x44, 0x3a, 0x3a]); // cmp r15b, [rdx]
    bytes.extend([0x75, 0x11]); // jne +17 -> done
    bytes.extend([0x48, 0xff, 0xc0]); // inc rax
    bytes.extend([0x48, 0xff, 0xc2]); // inc rdx
    bytes.extend([0x48, 0xff, 0xc9]); // dec rcx
    bytes.extend([0xeb, 0xe7]); // jmp -25 -> loop
    bytes.extend([0x41, 0xb9]); // mov r9d, imm32 (equal: result = 1)
    bytes.extend(1i32.to_le_bytes());

    // done: move the bool into the requested destination register.
    match destination {
        Reg64::R10 => bytes.extend([0x4d, 0x89, 0xca]), // mov r10, r9
        Reg64::R11 => bytes.extend([0x4d, 0x89, 0xcb]), // mov r11, r9
    }

    debug_assert_eq!(
        bytes.len() - operand_start,
        runtime_text_equals_operand_width(),
        "text-equals operand encoder length must match its width"
    );
    Ok(())
}

/// Guard-position text content equality against an inline literal:
/// `destination = (place == literal)` as bool 0/1, where `place` names the
/// String side's `{ptr @ +0, len @ +8}` text descriptor (a relocated storage
/// base, a pointee field behind a frame pointer slot, or a frame-indexed /
/// frame-base-indexed / frame-fixed-indexed element field) and the literal's
/// expected bytes are compared as inline immediates -- no rodata descriptor
/// exists for the literal side. Width is
/// `runtime_text_equals_literal_operand_width`
/// (place-setup plus a fixed head plus 13 bytes per literal byte; every
/// memory operand uses the disp32 form so the shape never varies with the
/// offsets).
///
/// Register use: r15 = relocated base, rax = descriptor address base,
/// r11 = index scratch (frame-indexed setup), rcx/rdx = ptr/len, r9 = the
/// bool result (moved into `destination` last). r12/r13/r14 stay untouched.
fn append_runtime_text_equals_literal_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination: Reg64,
    place: RuntimeValueOperandHandle,
    literal: &str,
    place_is_bounded_buffer: bool,
) -> Result<(), Diagnostic> {
    let operand_start = bytes.len();

    // Descriptor address base -> rax (+ `descriptor_disp` displacement). The
    // relocated `mov r15, imm64` sits at the operand start (the relocation
    // planner targets it there).
    let descriptor_disp;
    if let Some((_, byte_offset, _)) = runtime_value_operands.storage(place) {
        append_mov_r15_imm64(bytes, 0);
        append_mov_rax_r15(bytes);
        descriptor_disp = byte_offset;
    } else if let Some((pointer_byte_offset, field_byte_offset, _)) =
        runtime_value_operands.pointee(place)
    {
        // r15 = frame base (relocated); rax = the stored pointer. The
        // descriptor sits in the POINTEE at the field offset -- never read
        // the pointer slot's own bytes as a descriptor.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, pointer_byte_offset)?;
        descriptor_disp = field_byte_offset;
    } else if let Some((descriptor_offset, index_offset, element_byte_size, field_byte_offset, _)) =
        runtime_value_operands.frame_indexed(place)
    {
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 8)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_add_rax_r11(bytes);
        descriptor_disp = field_byte_offset;
    } else if let Some((
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_base_indexed(place)
    {
        // Inline frame fixed array: the elements live in the frame itself at
        // base_byte_offset; rax = frame base + index*element (same shape as
        // the frame-base-indexed load operand above).
        append_mov_r15_imm64(bytes, 0);
        append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 8)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_mov_rax_r15(bytes);
        append_add_rax_r11(bytes);
        descriptor_disp = base_byte_offset + field_byte_offset;
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_fixed_indexed(place)
    {
        // Constant element index: rax = the slice data pointer; the scaled
        // index folds into the descriptor displacement.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        descriptor_disp = element_index
            .checked_mul(element_byte_size)
            .and_then(|scaled| scaled.checked_add(field_byte_offset))
            .ok_or_else(|| {
                Diagnostic::error("X86_64 fixed indexed text descriptor offset overflow")
            })?;
    } else {
        return Err(Diagnostic::error(
            "X86_64 MVP encoder cannot compare this text place against a literal yet",
        ));
    }

    if place_is_bounded_buffer {
        // Owned carrier `{len@0, bytes@8}`: rcx = bytes ADDRESS (rax+disp+8,
        // computed, not a stored pointer); rdx = len read at offset 0. Same widths
        // as the descriptor path (lea/mov are both `48 .. 88/90 disp32` = 7 bytes),
        // so the byte-compare loop, branch offsets, and operand width are all
        // unchanged.
        bytes.extend([0x48, 0x8d, 0x88]); // lea rcx, [rax+disp32] (carrier bytes addr)
        bytes.extend(disp32(descriptor_disp + 8)?.to_le_bytes());
        bytes.extend([0x48, 0x8b, 0x90]); // mov rdx, [rax+disp32] (carrier.len @ 0)
        bytes.extend(disp32(descriptor_disp)?.to_le_bytes());
    } else {
        bytes.extend([0x48, 0x8b, 0x88]); // mov rcx, [rax+disp32] (ptr)
        bytes.extend(disp32(descriptor_disp)?.to_le_bytes());
        bytes.extend([0x48, 0x8b, 0x90]); // mov rdx, [rax+disp32] (len)
        bytes.extend(disp32(descriptor_disp + 8)?.to_le_bytes());
    }

    // result = 0; a length mismatch is unequal text. The jne also means an
    // all-zero (default) descriptor never has its null pointer dereferenced
    // when the literal is non-empty.
    let literal_bytes = literal.as_bytes();
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
    bytes.extend([0x48, 0x81, 0xfa]); // cmp rdx, imm32 (literal length)
    bytes.extend(disp32(literal_bytes.len())?.to_le_bytes());
    // Forward distances to `done` (the result move at the end): each byte
    // compare block is 13 bytes, plus the 6-byte equal-result mov.
    bytes.extend([0x0f, 0x85]); // jne rel32 -> done
    bytes.extend(disp32(13 * literal_bytes.len() + 6)?.to_le_bytes());
    for (byte_index, expected_byte) in literal_bytes.iter().enumerate() {
        bytes.extend([0x80, 0xb9]); // cmp byte [rcx+disp32], imm8
        bytes.extend(disp32(byte_index)?.to_le_bytes());
        bytes.push(*expected_byte);
        let remaining_blocks = literal_bytes.len() - 1 - byte_index;
        bytes.extend([0x0f, 0x85]); // jne rel32 -> done
        bytes.extend(disp32(13 * remaining_blocks + 6)?.to_le_bytes());
    }
    bytes.extend([0x41, 0xb9]); // mov r9d, imm32 (equal: result = 1)
    bytes.extend(1i32.to_le_bytes());

    // done: move the bool into the requested destination register.
    match destination {
        Reg64::R10 => bytes.extend([0x4d, 0x89, 0xca]), // mov r10, r9
        Reg64::R11 => bytes.extend([0x4d, 0x89, 0xcb]), // mov r11, r9
    }

    debug_assert_eq!(
        bytes.len() - operand_start,
        runtime_text_equals_literal_operand_width(runtime_value_operands, place, literal),
        "text-equals-literal operand encoder length must match its width"
    );
    Ok(())
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
    if operands.text_equals(operand).is_some() || operands.text_equals_literal(operand).is_some() {
        // Text content equality evaluates to a bool.
        return Some(1);
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
    } else if matches!(
        operator,
        StateGuardOperator::Divide
            | StateGuardOperator::Modulo
            | StateGuardOperator::DivideUnsigned
            | StateGuardOperator::ModuloUnsigned
    ) {
        // Division/modulo are NOT modular: a 64-bit idiv/div on a zero-extended
        // negative i32 dividend yields a wrong quotient. Run at the OPERAND width (an
        // immediate has no width, so use the non-immediate operand's), so a 32-bit
        // op handles the i32 dividend correctly -- signed via cdq, unsigned via the
        // resolver mapping Divide->DivideUnsigned. Add/sub/mul are modular and keep
        // the default 64-bit form. See [[guard-negative-i32-arithmetic]].
        //
        // When BOTH operands are immediates (a constant/constant divide that did not
        // fold) neither has a storage width, so fall back to the TARGET (declared)
        // width -- NOT 4. An i64 constant divide must run 64-bit; a 32-bit core would
        // truncate the dividend (e.g. -9_000_000_000) and the planned/emitted widths
        // would disagree (`runtime_storage_binary_write_width` uses the target size).
        runtime_value_operand_value_byte_size(operands, left)
            .or_else(|| runtime_value_operand_value_byte_size(operands, right))
            .unwrap_or(target_byte_size)
    } else {
        target_byte_size
    }
}

/// The width-correct integer idiv/div core: dividend in r10, divisor in r11,
/// quotient (or remainder, when `want_remainder`) back in r10. A 32-bit divide
/// reads only the low dword, so the width must match the operands. Signed uses
/// cdq/cqo + `idiv`; unsigned zeroes the dividend-high half + `div`. Shared by the
/// normal binary-op path and the saturating divide/modulo helper.
fn append_integer_divide_modulo_core(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
    signed: bool,
) {
    if byte_size <= 4 {
        // Narrow SIGNED operands may arrive ZERO-extended (e.g. the guard-subject
        // load path; see append_saturating_trapping_multiply), so a 32-bit idiv would
        // divide i8 -20 as 236. Sign-extend both to 32 bits first. Idempotent when
        // they are already sign-extended (the storage-write path); unsigned div is
        // correct zero-extended and skips this.
        if signed && byte_size == 1 {
            bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]); // movsx r10, r10b
            bytes.extend([0x4d, 0x0f, 0xbe, 0xdb]); // movsx r11, r11b
        } else if signed && byte_size == 2 {
            bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]); // movsx r10, r10w
            bytes.extend([0x4d, 0x0f, 0xbf, 0xdb]); // movsx r11, r11w
        }
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

/// Saturating SIGNED divide/modulo (dividend r10, divisor r11, result r10).
/// Integer division overflows only at TYPE_MIN / -1, the one corner `idiv`
/// hardware-traps on; guard the `divisor == -1` case so Saturating clamps instead
/// of trapping: `a % -1 == 0`, and `a / -1 == -a` saturating TYPE_MIN -> TYPE_MAX.
/// Every other divisor goes through the normal idiv (division reduces magnitude,
/// so no quotient/remainder can overflow). Unsigned div/mod never overflow and so
/// never reach here -- they fall through to the normal path.
fn append_saturating_signed_divide_modulo(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
) -> Result<(), Diagnostic> {
    // cmp r11, -1 (sized): the only divisor needing the saturating fixup.
    if byte_size <= 4 {
        bytes.extend([0x41, 0x83, 0xfb, 0xff]); // cmp r11d, -1
    } else {
        bytes.extend([0x49, 0x83, 0xfb, 0xff]); // cmp r11, -1
    }
    // The divisor == -1 fixup block.
    let mut special: Vec<u8> = Vec::new();
    if want_remainder {
        special.extend([0x45, 0x31, 0xd2]); // xor r10d, r10d  (a % -1 == 0)
    } else if byte_size <= 2 {
        // i8/i16: the dividend rides sign-extended in a 32-bit register, so `neg`
        // does NOT wrap at the narrow width -- a == TYPE_MIN yields -TYPE_MIN ==
        // TYPE_MAX + 1 (e.g. 128 for i8), the only overflow. The i32/i64 path below
        // detects TYPE_MIN via `neg`'s overflow flag, which a narrow TYPE_MIN cannot
        // set; here instead clamp any result above TYPE_MAX down to TYPE_MAX.
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u32;
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (-a; a==TYPE_MIN -> TYPE_MAX+1)
        special.push(0x41);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9d, TYPE_MAX
        special.extend([0x45, 0x39, 0xca]); // cmp r10d, r9d
        special.extend([0x45, 0x0f, 0x4f, 0xd1]); // cmovg r10d, r9d  (> TYPE_MAX -> TYPE_MAX)
    } else if byte_size <= 4 {
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u32;
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (sets OF iff r10d == TYPE_MIN)
        special.push(0x41);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9d, TYPE_MAX
        special.extend([0x45, 0x0f, 0x40, 0xd1]); // cmovo r10d, r9d  (TYPE_MIN -> TYPE_MAX)
    } else {
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
        special.extend([0x49, 0xf7, 0xda]); // neg r10
        special.push(0x49);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9, TYPE_MAX
        special.extend([0x4d, 0x0f, 0x40, 0xd1]); // cmovo r10, r9
    }
    // The normal idiv (every divisor except -1).
    let mut normal: Vec<u8> = Vec::new();
    append_integer_divide_modulo_core(&mut normal, byte_size, want_remainder, true);
    // jne over (special + the jmp) to the idiv; run special; jmp past the idiv.
    // Both blocks are well under 128 bytes, so rel8 offsets suffice.
    bytes.push(0x75);
    bytes.push((special.len() + 2) as u8); // jne -> normal
    bytes.extend(special);
    bytes.push(0xeb);
    bytes.push(normal.len() as u8); // jmp -> done
    bytes.extend(normal);
    Ok(())
}

/// WRAPPING signed divide/modulo. x86 `idiv` raises #DE (integer-overflow trap)
/// for TYPE_MIN / -1; the Wrapping domain must instead produce the WRAPPED result
/// (TYPE_MIN for divide -- the true quotient TYPE_MAX+1 wraps to TYPE_MIN -- and 0
/// for modulo). Guard the single overflowing divisor (-1) and avoid idiv for it:
/// `a / -1 == -a` via `neg r10` (and `neg` of TYPE_MIN naturally wraps to
/// TYPE_MIN, so no clamp is needed, unlike the saturating variant); `a % -1 == 0`.
/// Narrow widths (i8/i16) let the store truncate the negated 32-bit value back to
/// the correct wrapped byte. Divide-by-zero still reaches `idiv` and traps,
/// matching the interpreter. (aarch64 `sdiv` does not trap on overflow, so this
/// guard is x86_64-only.)
fn append_wrapping_signed_divide_modulo(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
) -> Result<(), Diagnostic> {
    // cmp r11, -1 (sized): the only divisor that would overflow idiv.
    if byte_size <= 4 {
        bytes.extend([0x41, 0x83, 0xfb, 0xff]); // cmp r11d, -1
    } else {
        bytes.extend([0x49, 0x83, 0xfb, 0xff]); // cmp r11, -1
    }
    // The divisor == -1 fixup block (always 3 bytes).
    let mut special: Vec<u8> = Vec::new();
    if want_remainder {
        special.extend([0x45, 0x31, 0xd2]); // xor r10d, r10d  (a % -1 == 0)
    } else if byte_size <= 4 {
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (-a; TYPE_MIN wraps to TYPE_MIN)
    } else {
        special.extend([0x49, 0xf7, 0xda]); // neg r10
    }
    // The normal idiv (every divisor except -1).
    let mut normal: Vec<u8> = Vec::new();
    append_integer_divide_modulo_core(&mut normal, byte_size, want_remainder, true);
    bytes.push(0x75);
    bytes.push((special.len() + 2) as u8); // jne -> normal
    bytes.extend(special);
    bytes.push(0xeb);
    bytes.push(normal.len() as u8); // jmp -> done
    bytes.extend(normal);
    Ok(())
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
        StateGuardOperator::BitwiseAnd => bytes.extend([0x4d, 0x21, 0xda]), // and r10, r11
        StateGuardOperator::BitwiseOr => bytes.extend([0x4d, 0x09, 0xda]),  // or r10, r11
        StateGuardOperator::BitwiseXor => bytes.extend([0x4d, 0x31, 0xda]), // xor r10, r11
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
            // Quotient -> (r/e)ax, remainder -> (r/e)dx; the width-correct idiv
            // sequence lives in the shared core (also used by saturating div/mod).
            let want_remainder = matches!(
                operator,
                StateGuardOperator::Modulo | StateGuardOperator::ModuloUnsigned
            );
            let signed = matches!(
                operator,
                StateGuardOperator::Divide | StateGuardOperator::Modulo
            );
            append_integer_divide_modulo_core(bytes, byte_size, want_remainder, signed);
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
        // `maxsd a, b` / `minsd a, b` return b on unordered (NaN) or equal, so
        // they realize `if a > b { a } else { b }` (and the min mirror) --
        // which the interpreter's float min/max matches exactly. This is what
        // makes float min/max, and hence abs/clamp over floats, lower.
        StateGuardOperator::Max => 0x5f, // maxsd/maxss
        StateGuardOperator::Min => 0x5d, // minsd/minss
        // sqrt is UNARY, carried with both operands = x: `sqrtsd xmm0, xmm1`
        // computes sqrt(xmm1) = sqrt(x) into xmm0, so the shared final line
        // below (op on xmm0, xmm1) already produces the right result.
        StateGuardOperator::Sqrt => 0x51, // sqrtsd/sqrtss
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
        | StateGuardOperator::BitwiseAnd
        | StateGuardOperator::BitwiseOr
        | StateGuardOperator::BitwiseXor
        | StateGuardOperator::Subtract => 3,
        StateGuardOperator::Multiply => 4,
        // cmp (3) + cmov (4), same at 32-bit or 64-bit.
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => 7,
        // signed 32-bit: mov(3)+cdq(1)+idiv(3)+mov(3)=10; signed 64-bit: cqo(2)=11.
        // Narrow signed (i8/i16) prepends two movsx (8) to sign-extend the operands
        // to the 32-bit op width; see append_integer_divide_modulo_core.
        StateGuardOperator::Divide | StateGuardOperator::Modulo => {
            let sign_extend = if byte_size <= 2 { 8 } else { 0 };
            sign_extend + if byte_size <= 4 { 10 } else { 11 }
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
    is_float: bool,
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
    // IEEE semantics for a NaN operand: every comparison is FALSE except `!=` (true).
    // `ucomis*` reports an unordered/NaN operand by setting PF=1 (alongside ZF=CF=1),
    // which the ZF/CF-only failure jcc above misreads as "equal". Prepend a parity
    // branch so NaN is routed correctly. This 6-byte `jp` sits BEFORE the main jcc, so
    // the main jcc's own rel32 is unchanged (both it and its target shift down by 6);
    // the float width functions account for the extra 6 bytes.
    if is_float {
        if matches!(operator, StateGuardOperator::NotEqual) {
            // `!=` on NaN is TRUE (guard succeeds): jump PAST the 6-byte `je` so NaN
            // falls through to the success arm instead of taking the equal-failure jump.
            append_jcc_rel32(bytes, 0x8a, 6)?; // jp over the je
        } else {
            // Every other operator is FALSE on NaN (guard fails): jump to the same
            // failure arm as the main jcc, which now sits 6 bytes further along.
            append_jcc_rel32(bytes, 0x8a, failure_branch_distance + 6)?; // jp to failure
        }
    }
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

fn append_load_rax_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// 32-bit zero-extending load of an array INDEX. A 64-bit `mov rax` reads 8 bytes,
/// which for a 4-byte index field (i32/u32) pulls in the ADJACENT field's bytes as
/// the high dword -> a garbage index and an OOB store (segfault). Every valid array
/// index fits in 32 bits, so load `eax` (which zero-extends into rax); matches the
/// machine-indexed COPY encoder.
fn append_load_index_eax_from_r10(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x41, 0x8b, 0x82]); // mov eax, [r10 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// See [`append_load_index_eax_from_r10`].
fn append_load_index_eax_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x41, 0x8b, 0x87]); // mov eax, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Load a 4-byte runtime index into r15, ZERO-EXTENDING into the upper 32 bits (`mov r15d`).
/// A 64-bit load here would pull the 4 bytes ADJACENT to a 4-byte index field into the high
/// dword, producing a garbage index and an out-of-bounds store when that neighbour is non-zero
/// (the same class of bug fixed for the integer indexed write). Byte-count-identical to the
/// 64-bit `append_load_r15_from_r14`, so instruction widths are unchanged.
fn append_load_index_r15d_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x45, 0x8b, 0xbe]); // mov r15d, [r14 + disp32] (32-bit, zero-extends into r15)
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

fn append_mov_rax_r10(bytes: &mut Vec<u8>) {
    // mov rax, r10 -- 89 /r with r10 in the reg field (REX.R) and rax in r/m.
    bytes.extend([0x4c, 0x89, 0xd0]);
}

/// Byte count of [`append_mov_rax_r10`].
const MOV_RAX_R10_WIDTH: usize = 3;

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

/// Emit `lock xadd [r14 + disp32], r10` at the given operand width. XADD swaps
/// then adds: it loads the prior `[mem]` into the source register (r10) and
/// stores `[mem] + r10` back, all as ONE atomic read-modify-write under the
/// LOCK prefix -- exactly `fetch_add`'s contract (r10 ends with the OLD value).
/// Caller sets r10 = the delta and r14 = the atomic field's base BEFORE this.
/// Used by `encode_atomic_fetch_add`; byte-verified by `atomic_tests` below.
fn append_lock_xadd_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    // F0 = LOCK. REX picks operand width (W) + r10 (R) + r14 (B). XADD is
    // `0F C1 /r` (or `0F C0 /r` for 8-bit). ModRM 0x96 = mod=10 (disp32),
    // reg=r10&7=2, r/m=r14&7=6.
    match byte_size {
        1 => bytes.extend([0xf0, 0x45, 0x0f, 0xc0, 0x96]),
        2 => bytes.extend([0xf0, 0x66, 0x45, 0x0f, 0xc1, 0x96]),
        4 => bytes.extend([0xf0, 0x45, 0x0f, 0xc1, 0x96]),
        8 => bytes.extend([0xf0, 0x4d, 0x0f, 0xc1, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 encoder cannot LOCK xadd {byte_size}-byte atomics yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Emitted byte count of [`append_lock_xadd_r10_to_r14`] (opcode block + disp32).
fn lock_xadd_r10_to_r14_width(byte_size: usize) -> usize {
    let opcode = match byte_size {
        1 | 4 => 5,
        2 => 6,
        8 => 5,
        _ => 5,
    };
    opcode + 4
}

/// `LOCK CMPXCHG [r14+disp32], r10`: compare rax with the place; if equal store
/// r10 (ZF=1), else load the place into rax (ZF=0). Identical layout to
/// `append_lock_xadd_r10_to_r14` but with the CMPXCHG opcode (`0F B1`, or
/// `0F B0` for 8-bit). Used by `encode_atomic_compare_exchange`; byte-verified
/// by `atomic_tests`.
fn append_lock_cmpxchg_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0xf0, 0x45, 0x0f, 0xb0, 0x96]),
        2 => bytes.extend([0xf0, 0x66, 0x45, 0x0f, 0xb1, 0x96]),
        4 => bytes.extend([0xf0, 0x45, 0x0f, 0xb1, 0x96]),
        8 => bytes.extend([0xf0, 0x4d, 0x0f, 0xb1, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 encoder cannot LOCK cmpxchg {byte_size}-byte atomics yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Emitted byte count of [`append_lock_cmpxchg_r10_to_r14`] (opcode + disp32).
/// Same layout as `lock_xadd_r10_to_r14_width` (only the opcode byte differs).
fn lock_cmpxchg_r10_to_r14_width(byte_size: usize) -> usize {
    lock_xadd_r10_to_r14_width(byte_size)
}

#[cfg(test)]
mod call_encoding_tests {
    use super::append_call_register;

    #[test]
    fn low_registers_emit_ff_d0_through_ff_d7_without_rex() {
        // `FF /2` register-direct: ModRM = 0xD0 | rm, no REX for rax..rdi.
        // rax=D0 rcx=D1 rdx=D2 rbx=D3 rsp=D4 rbp=D5 rsi=D6 rdi=D7.
        for reg in 0u8..8 {
            let mut bytes = Vec::new();
            append_call_register(&mut bytes, reg);
            assert_eq!(
                bytes,
                vec![0xff, 0xd0 + reg],
                "call r{reg} must be FF {:02X} with no REX",
                0xd0 + reg
            );
        }
    }

    #[test]
    fn extended_registers_take_a_rex_b_prefix() {
        // r8..r15 need REX.B (0x41); ModRM low 3 bits wrap (r8 -> D0, r11 -> D3).
        for reg in 8u8..16 {
            let mut bytes = Vec::new();
            append_call_register(&mut bytes, reg);
            assert_eq!(
                bytes,
                vec![0x41, 0xff, 0xd0 | (reg & 0x7)],
                "call r{reg} must be 41 FF {:02X}",
                0xd0 | (reg & 0x7)
            );
        }
    }

    #[test]
    fn canonical_targets_are_exact() {
        // Spot-check the registers the first-boot path actually uses.
        let mut rax = Vec::new();
        append_call_register(&mut rax, 0);
        assert_eq!(rax, vec![0xff, 0xd0], "call rax");

        let mut r11 = Vec::new();
        append_call_register(&mut r11, 11);
        assert_eq!(r11, vec![0x41, 0xff, 0xd3], "call r11");
    }
}

#[cfg(test)]
mod vtable_call_encoding_tests {
    use super::{encode_win64_vtable_call, win64_vtable_call_width};
    use omega_target_operations::{InstructionOperandLike, RuntimeStorageRegion};

    /// A minimal operand: either a runtime scalar (RCX = this from a field) or
    /// a runtime storage address (RDX = &text field). Everything else None.
    enum Op {
        Scalar { region: RuntimeStorageRegion, offset: usize, size: usize },
        Address { region: RuntimeStorageRegion, offset: usize },
    }
    impl InstructionOperandLike for Op {
        fn data_address(&self) -> Option<omega_target_operations::TargetDataObjectHandle> { None }
        fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> { None }
        fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> { None }
        fn runtime_string_is_bounded_buffer(&self) -> bool { false }
        fn runtime_pointee_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> { None }
        fn runtime_pointee_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> { None }
        fn runtime_scalar_integer(&self) -> Option<(RuntimeStorageRegion, usize, usize)> {
            match self {
                Op::Scalar { region, offset, size } => Some((*region, *offset, *size)),
                _ => None,
            }
        }
        fn runtime_storage_address(&self) -> Option<(RuntimeStorageRegion, usize)> {
            match self {
                Op::Address { region, offset } => Some((*region, *offset)),
                _ => None,
            }
        }
        fn immediate_integer(&self) -> Option<i64> { None }
        fn byte_length(&self) -> Option<usize> { None }
    }

    #[test]
    fn output_string_marshals_this_and_text_then_calls_through_slot_1() {
        // output_string(this: addr@machine+0, text: &field@machine+8) -> VtableSlot(1).
        let operands = vec![
            Op::Scalar { region: RuntimeStorageRegion::Machine, offset: 0, size: 8 },
            Op::Address { region: RuntimeStorageRegion::Machine, offset: 8 },
        ];
        let bytes = encode_win64_vtable_call(&operands, 1).expect("encode");
        assert_eq!(bytes.len(), win64_vtable_call_width(&operands, 1), "width matches");

        // 2 register args -> reserve = 32 (padded to 40); sub rsp, 40 (imm8).
        assert_eq!(&bytes[0..4], &[0x48, 0x83, 0xec, 40], "sub rsp, 40");
        // arg 0 (this -> RCX): mov r15,imm64 (10) then mov rcx,[r15+0] (49 8b 8f + disp32 0).
        assert_eq!(bytes[4], 0x49, "mov r15,imm64 opcode #0");
        assert_eq!(&bytes[14..21], &[0x49, 0x8b, 0x8f, 0, 0, 0, 0], "rcx = [r15+0]");
        // arg 1 (text -> RDX lea): mov r15,imm64 (10) then lea rdx,[r15+8] (49 8d 97 + disp32 8).
        assert_eq!(&bytes[31..38], &[0x49, 0x8d, 0x97, 8, 0, 0, 0], "lea rdx, [r15+8]");
        // the vtable read + indirect call, then restore.
        assert_eq!(&bytes[38..45], &[0x48, 0x8b, 0x81, 8, 0, 0, 0], "mov rax, [rcx+8] (slot 1)");
        assert_eq!(&bytes[45..47], &[0xff, 0xd0], "call rax");
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40], "add rsp, 40");
    }
}

#[cfg(test)]
mod atomic_tests {
    use super::*;

    #[test]
    fn lock_xadd_emits_lock_prefix_and_xadd_opcode() {
        for &byte_size in &[1usize, 2, 4, 8] {
            let mut bytes = Vec::new();
            append_lock_xadd_r10_to_r14(&mut bytes, 0x18, byte_size).expect("encode");
            assert_eq!(
                bytes.len(),
                lock_xadd_r10_to_r14_width(byte_size),
                "width mismatch for {byte_size}-byte lock xadd"
            );
            assert_eq!(bytes[0], 0xf0, "must begin with the LOCK prefix (0xF0)");
            // Operand-size prefix only for 16-bit.
            let rex_index = if byte_size == 2 { 2 } else { 1 };
            if byte_size == 2 {
                assert_eq!(bytes[1], 0x66, "16-bit needs the operand-size prefix");
            }
            assert_eq!(bytes[rex_index], if byte_size == 8 { 0x4d } else { 0x45 }, "REX");
            assert_eq!(bytes[rex_index + 1], 0x0f, "two-byte opcode escape");
            let xadd_opcode = if byte_size == 1 { 0xc0 } else { 0xc1 };
            assert_eq!(bytes[rex_index + 2], xadd_opcode, "XADD opcode");
            assert_eq!(bytes[rex_index + 3], 0x96, "ModRM [r14+disp32], r10");
            // disp32 little-endian tail.
            assert_eq!(&bytes[rex_index + 4..], &0x18i32.to_le_bytes());
        }
    }

    #[test]
    fn lock_cmpxchg_emits_lock_prefix_and_cmpxchg_opcode() {
        for &byte_size in &[1usize, 2, 4, 8] {
            let mut bytes = Vec::new();
            append_lock_cmpxchg_r10_to_r14(&mut bytes, 0x24, byte_size).expect("encode");
            assert_eq!(
                bytes.len(),
                lock_cmpxchg_r10_to_r14_width(byte_size),
                "width mismatch for {byte_size}-byte lock cmpxchg"
            );
            assert_eq!(bytes[0], 0xf0, "must begin with the LOCK prefix (0xF0)");
            let rex_index = if byte_size == 2 { 2 } else { 1 };
            if byte_size == 2 {
                assert_eq!(bytes[1], 0x66, "16-bit needs the operand-size prefix");
            }
            assert_eq!(bytes[rex_index], if byte_size == 8 { 0x4d } else { 0x45 }, "REX");
            assert_eq!(bytes[rex_index + 1], 0x0f, "two-byte opcode escape");
            // CMPXCHG is 0F B1 (or 0F B0 for 8-bit), NOT xadd's 0F C1/C0.
            let cmpxchg_opcode = if byte_size == 1 { 0xb0 } else { 0xb1 };
            assert_eq!(bytes[rex_index + 2], cmpxchg_opcode, "CMPXCHG opcode");
            assert_eq!(bytes[rex_index + 3], 0x96, "ModRM [r14+disp32], r10");
            assert_eq!(&bytes[rex_index + 4..], &0x24i32.to_le_bytes());
        }
    }
}
