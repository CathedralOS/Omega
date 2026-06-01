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

pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_width() -> usize {
    0
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

pub fn dispatch_guard_compare_static_width() -> usize {
    // mov r15, imm64 (10) + load r10, [r15+disp32] (7)
    // + mov r11, imm64 (10) + cmp r10, r11 (3) + jcc rel32 (6)
    36
}

pub fn encode_dispatch_guard_compare_static_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
        )));
    }
    let mut bytes = Vec::with_capacity(dispatch_guard_compare_static_width());
    // Storage base; the imm64 (at instruction start + 2) is relocated to the
    // guard's storage-region data symbol by the relocation planner.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_reg_from_r15(&mut bytes, Reg64::R10, byte_offset, byte_size)?;
    append_mov_reg_imm64(&mut bytes, Reg64::R11, expected_value as u64);
    append_cmp_r10_r11(&mut bytes, byte_size)?;
    // `skip_byte_distance` is measured from instruction start + 16 (the AArch64
    // conditional-branch position). On x86_64 the jcc ends at instruction start
    // + 36, so adjust the relative target by the 20-byte difference.
    append_failure_branch(&mut bytes, operator, skip_byte_distance - 20)?;
    debug_assert_eq!(bytes.len(), dispatch_guard_compare_static_width());
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
        (HostCapability::Stdin | HostCapability::Stdout, HostOperation::GetStdHandle) => {
            encode_get_std_handle(operands)
        }
        (HostCapability::Stdout, HostOperation::Write | HostOperation::WriteFile) => {
            encode_file_operation(operation_key, operands)
        }
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
    } else {
        Err(Diagnostic::error(
            "cannot encode X86_64 file operation: length operand is unsupported",
        ))
    }
}

fn encode_exit_process<T: InstructionOperandLike>(operands: &[T]) -> Result<Vec<u8>, Diagnostic> {
    let exit_code = immediate_i32(operands, 0, "ExitProcess exit code")?;
    let mut bytes = Vec::with_capacity(18);
    bytes.extend([0x48, 0x83, 0xec, 0x28]); // sub rsp, 40
    bytes.push(0xb9); // mov ecx, imm32
    bytes.extend(exit_code.to_le_bytes());
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32
    bytes.extend([0x48, 0x83, 0xc4, 0x28]); // add rsp, 40
    Ok(bytes)
}

fn host_call_relocation_sites<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    match (operation_key.capability, operation_key.operation) {
        (HostCapability::Stdin | HostCapability::Stdout, HostOperation::GetStdHandle)
        | (HostCapability::Process, HostOperation::ExitProcess) => {
            vec![X86_64RelocationSite {
                operand_index: None,
                byte_offset: 10,
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            }]
        }
        (HostCapability::Stdout, HostOperation::Write | HostOperation::WriteFile)
        | (HostCapability::Stdin, HostOperation::ReadFile) => {
            let mut sites = Vec::new();
            let Ok((pointer_index, length_index)) = file_pointer_and_length_indices(operands)
            else {
                return sites;
            };
            let mut cursor = if pointer_index == 1 { 9 } else { 7 };

            if operands.get(pointer_index).is_some_and(|operand| {
                operand.data_address().is_some() || operand.runtime_string_pointer().is_some()
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

            if operands
                .get(length_index)
                .is_some_and(|operand| operand.runtime_string_length().is_some())
            {
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
            if operand.data_address().is_some() || operand.runtime_string_pointer().is_some() =>
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
        _ => 0,
    }
}

fn file_length_operand_width<T: InstructionOperandLike>(operand: Option<&T>) -> usize {
    match operand {
        Some(operand) if operand.byte_length().is_some() => 6,
        Some(operand) if operand.runtime_string_length().is_some() => 17,
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
    debug_assert_eq!(bytes.len(), runtime_text_literal_segment_write_width(literal));
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
        bytes[at..at + 4].copy_from_slice(&((loop_start as isize - (at as isize + 4)) as i32).to_le_bytes());
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
    debug_assert_eq!(match_jmp_at + 4, width, "match jmp must terminate the instruction");

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
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + 9
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
        left,
        right,
    ));
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R11, right)?;
    append_cmp_r10_r11(&mut bytes, byte_size)?;
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4)?;
    Ok(bytes)
}

pub fn runtime_machine_integer_write_width(_byte_offset: usize, _byte_size: usize) -> usize {
    27
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
    let mut bytes =
        Vec::with_capacity(runtime_pointee_integer_write_width(field_byte_offset, byte_size));
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
    bytes.extend([0x3c, 0x0a]); // cmp al, '\n'
    jcc_done(&mut bytes, 0x84);
    bytes.extend([0x3c, 0x0d]); // cmp al, '\r'
    jcc_done(&mut bytes, 0x84);
    bytes.extend([0x3c, 0x00]); // cmp al, 0
    jcc_done(&mut bytes, 0x84);
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
        bytes[loop_jmp_disp..loop_jmp_disp + 4]
            .copy_from_slice(&(rel as i32).to_le_bytes());
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

pub fn runtime_machine_string_write_width(_byte_length: usize) -> usize {
    44
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

pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    10 + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(operator)
        + 7.max(store_width(byte_size))
}

pub fn encode_runtime_storage_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
    ));
    // Hold the target base in r14, not r15: evaluating the operands below
    // reloads r15 with each source base, which would otherwise clobber the
    // target pointer before the store. r14 is untouched by operand evaluation.
    // `mov r14, imm64` and `mov r15, imm64` are both 10 bytes with the relocated
    // immediate at +2, so the target relocation offset is unchanged.
    append_mov_r14_imm64(&mut bytes, 0);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R11, right)?;
    append_runtime_binary_operation(&mut bytes, operator)?;
    append_store_r10_to_r14(&mut bytes, target_offset, byte_size)?;
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
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(operator)
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
    debug_assert_eq!(bytes.len(), runtime_frame_base_indexed_binary_left_operand_offset());
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R11, right)?;
    append_runtime_binary_operation(&mut bytes, operator)?;
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
    let mut bytes = Vec::with_capacity(runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
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
    for_each_runtime_copy_chunk(source_offset, target_offset, byte_count, |offset, chunk_size| {
        append_load_rax_from_r14(&mut bytes, source_offset + offset, chunk_size)?;
        append_store_rax_to_r15(&mut bytes, target_offset + offset, chunk_size)?;
        Ok(())
    })?;
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
        32
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + runtime_binary_operation_width(operator)
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
        append_load_reg_from_rax(bytes, destination, base_byte_offset + field_byte_offset, byte_size)
    } else if runtime_value_operands
        .frame_fixed_indexed(operand)
        .is_some()
    {
        Err(Diagnostic::error(
            "X86_64 runtime fixed indexed value operand is not implemented yet",
        ))
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, left)?;
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R11, right)?;
        append_runtime_binary_operation(bytes, operator)?;
        append_mov_reg_reg(bytes, destination, Reg64::R10);
        Ok(())
    } else {
        Err(Diagnostic::error(
            "X86_64 runtime value operand is not implemented yet",
        ))
    }
}

fn append_runtime_binary_operation(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
) -> Result<(), Diagnostic> {
    match operator {
        StateGuardOperator::Add => bytes.extend([0x4d, 0x01, 0xda]), // add r10, r11
        StateGuardOperator::And => bytes.extend([0x4d, 0x21, 0xda]), // and r10, r11
        StateGuardOperator::Or => bytes.extend([0x4d, 0x09, 0xda]),  // or r10, r11
        StateGuardOperator::Subtract => bytes.extend([0x4d, 0x29, 0xda]), // sub r10, r11
        StateGuardOperator::Multiply => bytes.extend([0x4d, 0x0f, 0xaf, 0xd3]), // imul r10, r11
        StateGuardOperator::Modulo => {
            bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
            bytes.extend([0x48, 0x31, 0xd2]); // xor rdx, rdx
            bytes.extend([0x49, 0xf7, 0xf3]); // div r11
            bytes.extend([0x49, 0x89, 0xd2]); // mov r10, rdx
        }
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual => {
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend(match operator {
                StateGuardOperator::Equal => [0x0f, 0x94, 0xc0],
                StateGuardOperator::NotEqual => [0x0f, 0x95, 0xc0],
                StateGuardOperator::Greater => [0x0f, 0x9f, 0xc0],
                StateGuardOperator::GreaterOrEqual => [0x0f, 0x9d, 0xc0],
                StateGuardOperator::Less => [0x0f, 0x9c, 0xc0],
                StateGuardOperator::LessOrEqual => [0x0f, 0x9e, 0xc0],
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

fn runtime_binary_operation_width(operator: StateGuardOperator) -> usize {
    match operator {
        StateGuardOperator::Add
        | StateGuardOperator::And
        | StateGuardOperator::Or
        | StateGuardOperator::Subtract => 3,
        StateGuardOperator::Multiply => 4,
        StateGuardOperator::Modulo => 12,
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual => 10,
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
    let opcode = match operator {
        StateGuardOperator::Equal => 0x85,          // jne
        StateGuardOperator::NotEqual => 0x84,       // je
        StateGuardOperator::Greater => 0x8e,        // jle
        StateGuardOperator::GreaterOrEqual => 0x8c, // jl
        StateGuardOperator::Less => 0x8d,           // jge
        StateGuardOperator::LessOrEqual => 0x8f,    // jg
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

fn append_load_r11_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x9e]); // mov r11, [r14 + disp32]
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

fn append_load_r14_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xb7]); // mov r14, [r15 + disp32]
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
        (Reg64::R10, 4) => bytes.extend([0x45, 0x8b, 0x97]),
        (Reg64::R10, 8) => bytes.extend([0x4d, 0x8b, 0x97]),
        (Reg64::R11, 1) => bytes.extend([0x45, 0x8a, 0x9f]),
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
        // mov [r14+disp32], r10{b,d,} -- ModRM reg=r10, r/m=r14
        1 => bytes.extend([0x45, 0x88, 0x96]),
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
        _ => 0,
    }
}

fn store_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 7,
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
