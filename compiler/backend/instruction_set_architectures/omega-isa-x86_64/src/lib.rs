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
    append_mov_r15_imm64(&mut bytes, 0);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R11, right)?;
    append_runtime_binary_operation(&mut bytes, operator)?;
    append_store_r10_to_r15(&mut bytes, target_offset, byte_size)?;
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

pub fn runtime_value_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    if runtime_value_operands.immediate_integer(operand).is_some() {
        10
    } else if let Some((_, _, byte_size)) = runtime_value_operands.storage(operand) {
        10 + load_width(byte_size)
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
