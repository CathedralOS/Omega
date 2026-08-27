use super::{
    X86_64RelocationSite, X86_64RelocationSiteKind, append_load_r8_from_r10,
    append_load_r10_from_r10, append_load_rdx_from_r10, append_mov_r10_imm64, append_mov_r11_imm64,
    append_mov_r15_imm64, append_mov_rax_imm64, append_mov_rdx_imm64, disp32,
    encode_outgoing_stack_address_load_bytes, immediate_i32, x86_gpr_number,
};
use crate::caller_frame::append_store_rax_to_rsp_disp32;
use crate::caller_frame::rsp_adjust_width;
pub(super) use crate::caller_frame::{append_add_rsp, append_sub_rsp};
use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, EntryControl, HostCapability, HostOperation,
    HostOperationKey, IndirectPointerLocation, MachineRegister, RegisterSet, SystemVEightbyteClass,
    ValueClass, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan, validate_call_plan,
};
use omega_target_operations::{InstructionOperandLike, RuntimeStorageRegion};
use psi_diagnostics::Diagnostic;

#[cfg(test)]
mod tests;

pub fn host_call_sequence_width_no_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
) -> usize {
    match encode_host_call_sequence_no_plan(policy, operation_key, operands) {
        Ok(bytes) => bytes.len(),
        Err(error) => {
            if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
                eprintln!(
                    "BTW host call width 0: {}.{}: {}",
                    operation_key.capability_name(),
                    operation_key.operation_name(),
                    error.message
                );
            }
            0
        }
    }
}

pub fn host_call_data_relocation_site_no_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    operand_index: usize,
) -> Option<X86_64RelocationSite> {
    host_call_data_relocation_site_for_plan(
        policy,
        operation_key,
        operands,
        operand_index,
        HostCallPlan::CompatibilityOracle,
    )
}

pub fn host_call_data_relocation_site_with_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    operand_index: usize,
    authoritative_plan: &CallPlan,
) -> Option<X86_64RelocationSite> {
    host_call_data_relocation_site_for_plan(
        policy,
        operation_key,
        operands,
        operand_index,
        HostCallPlan::Authoritative(authoritative_plan),
    )
}

fn host_call_data_relocation_site_for_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    operand_index: usize,
    plan_source: HostCallPlan<'_>,
) -> Option<X86_64RelocationSite> {
    host_call_relocation_sites_for_plan(policy, operation_key, operands, plan_source)
        .into_iter()
        .find(|site| {
            site.operand_index == Some(operand_index)
                && site.kind == X86_64RelocationSiteKind::Absolute64
        })
}

pub fn host_call_external_relocation_site_no_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Option<X86_64RelocationSite> {
    host_call_external_relocation_site_for_plan(
        policy,
        operation_key,
        operands,
        HostCallPlan::CompatibilityOracle,
    )
}

pub fn host_call_external_relocation_site_with_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    authoritative_plan: &CallPlan,
) -> Option<X86_64RelocationSite> {
    host_call_external_relocation_site_for_plan(
        policy,
        operation_key,
        operands,
        HostCallPlan::Authoritative(authoritative_plan),
    )
}

fn host_call_external_relocation_site_for_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    plan_source: HostCallPlan<'_>,
) -> Option<X86_64RelocationSite> {
    host_call_relocation_sites_for_plan(policy, operation_key, operands, plan_source)
        .into_iter()
        .find(|site| site.kind == X86_64RelocationSiteKind::Relative32)
}

pub fn encode_host_call_sequence_no_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    encode_host_call_sequence_for_plan(
        policy,
        operation_key,
        operands,
        HostCallPlan::CompatibilityOracle,
    )
}

pub fn encode_host_call_sequence_with_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    encode_host_call_sequence_for_plan(
        policy,
        operation_key,
        operands,
        HostCallPlan::Authoritative(authoritative_plan),
    )
}

#[derive(Clone, Copy)]
pub(super) enum HostCallPlan<'plan> {
    CompatibilityOracle,
    Authoritative(&'plan CallPlan),
}

fn encode_host_call_sequence_for_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    plan_source: HostCallPlan<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    // Target calibration constants do not cross a call boundary. Keep their
    // architecture-local materialization available under every x86 policy.
    if matches!(
        (operation_key.capability, operation_key.operation),
        (
            HostCapability::Clock,
            HostOperation::WallClockUnitsPerSecond | HostOperation::WallClockEpochOffsetSeconds
        )
    ) {
        return encode_constant_result(operands);
    }
    if policy == CallingPolicy::SystemVAMD64
        && matches!(
            operation_key.capability,
            HostCapability::Unknown | HostCapability::Custom(_)
        )
    {
        return Ok(sysv_import_layout_for_plan(operands, true, plan_source)?.bytes);
    }
    if policy != CallingPolicy::MicrosoftX64 {
        return Err(Diagnostic::error(format!(
            "X86_64 compatibility host encoder implements Microsoft x64, not {policy:?}"
        )));
    }
    match (operation_key.capability, operation_key.operation) {
        (
            HostCapability::Stdin | HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::GetStdHandle,
        ) => {
            validate_normalized_win64_get_std_handle_plan(plan_source)?;
            encode_win64_import_call_for_plan(
                operands,
                false,
                false,
                HostCallPlan::CompatibilityOracle,
            )
        }
        (
            HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::Write | HostOperation::WriteFile,
        ) => encode_file_operation(operation_key, operands, plan_source),
        (HostCapability::Stdin, HostOperation::ReadFile) => {
            encode_file_operation(operation_key, operands, plan_source)
        }
        (HostCapability::Process, HostOperation::ExitProcess)
        | (HostCapability::Clock, HostOperation::Sleep) => {
            encode_win64_import_call_for_plan(operands, false, false, plan_source)
        }
        // A 0-arg value-returning import through the GENERAL import-call encoder
        // (byte-identical to the original bespoke tick_count sequence for an
        // 8-byte result, and width-correct for a 4-byte one).
        (HostCapability::Clock, HostOperation::TickCount) => {
            encode_win64_import_call_for_plan(operands, true, false, plan_source)
        }
        // 0-arg value-returning imports whose result arrives through an
        // OUT-PARAM (QueryPerformanceCounter/-Frequency write a LARGE_INTEGER,
        // GetSystemTimePreciseAsFileTime a FILETIME): bracket the call with a
        // stack slot and load the u64 back (std::time rung 5).
        (
            HostCapability::Clock,
            HostOperation::MonotonicTicks
            | HostOperation::MonotonicTicksPerSecond
            | HostOperation::WallClockRaw,
        ) => encode_win64_out_param_call(operation_key, operands, plan_source),
        (HostCapability::Input, HostOperation::KeyState) => {
            encode_key_state_call(operands, plan_source)
        }
        // Every Gui import is value-returning and encodes through the GENERAL
        // import call: operands[0] = result place, then the full ABI argument
        // list (selection interleaves the hard-wired immediates).
        (HostCapability::Gui, _) => {
            encode_win64_import_call_for_plan(operands, true, false, plan_source)
        }
        // Every Filesystem raw-seam op is value-returning (fd/count/rc) and
        // rides the same general import call (msvcrt's POSIX-shaped CRT calls
        // marshal like any Win64 import). `read_errno` (`_errno()` returns
        // `&errno`) derefs the returned pointer before the store, exactly the
        // darwin `___error()` shape.
        (HostCapability::Filesystem, _) => encode_win64_import_call_for_plan(
            operands,
            true,
            operation_key.dereferences_result(),
            plan_source,
        ),
        // Provides-AUTHORED ops (extern brief §12): outside the closed catalog
        // the key is (Unknown, Unknown), and the op only reaches encoding when
        // its authored DllImport binding exists -- ride the same general
        // value-returning import call as the Filesystem/Gui rows.
        (HostCapability::Unknown | HostCapability::Custom(_), _) => {
            encode_win64_import_call_for_plan(operands, true, false, plan_source)
        }
        _ => Err(Diagnostic::error(format!(
            "X86_64 host operation {}.{} is not implemented",
            operation_key.capability_name(),
            operation_key.operation_name()
        ))),
    }
}

/// Encode an authored import from the exact validated source-selected plan.
/// The concrete image target does not replace the boundary's policy choice.
pub fn encode_authored_import_call_sequence<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match plan.policy {
        CallingPolicy::MicrosoftX64 => encode_win64_import_call_for_plan(
            operands,
            true,
            false,
            HostCallPlan::Authoritative(plan),
        ),
        CallingPolicy::SystemVAMD64 => {
            Ok(
                sysv_import_layout_for_plan(operands, true, HostCallPlan::Authoritative(plan))?
                    .bytes,
            )
        }
        policy => Err(Diagnostic::error(format!(
            "x86-64 authored import encoder cannot realize {policy:?}"
        ))),
    }
}

pub fn authored_import_relocation_sites<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    match plan.policy {
        CallingPolicy::MicrosoftX64 => win64_import_call_relocation_sites_for_plan(
            operands,
            true,
            false,
            HostCallPlan::Authoritative(plan),
        ),
        CallingPolicy::SystemVAMD64 => {
            sysv_import_layout_for_plan(operands, true, HostCallPlan::Authoritative(plan))
                .map(|layout| layout.relocation_sites)
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// `GetAsyncKeyState(vk)` -- a value-returning USER32 import (the multi-DLL
/// proof): shadow space, the vk marshalled into ecx from operands[1] (constant
/// or runtime scalar), the relocated `call rel32`, the shadow restore, then
/// `movzx eax, ax` (the return is a SHORT; zero the undefined upper bits) and
/// the store-rax tail into the result place (operands[0]).
fn encode_key_state_call<T: InstructionOperandLike>(
    operands: &[T],
    plan_source: HostCallPlan<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    let Some((_, result_offset, _)) = operands
        .first()
        .and_then(|operand| operand.runtime_scalar_integer())
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 key_state: the result storage place did not lower to a              runtime scalar operand",
        ));
    };
    let plan = match plan_source {
        HostCallPlan::Authoritative(plan) => {
            validate_win64_encoder_plan(plan)?;
            validate_win64_call_plan_operand_shapes(plan, operands, Some(0), 1)?;
            plan.clone()
        }
        HostCallPlan::CompatibilityOracle => normalized_win64_call_plan(operands, Some(0), 1)?,
    };
    let result_register = normalized_win64_result_register(&plan, true)?;
    if result_register != Some(MachineRegister::X86Rax) {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 key-state result requires rax, got {result_register:?}"
        )));
    }
    let reserve = win64_import_reserve(plan.parameters.len());
    let mut bytes = Vec::with_capacity(4 + 17 + 5 + 4 + 3 + 17);
    append_sub_rsp(&mut bytes, reserve);
    append_win64_call_arguments(&mut bytes, operands, 1, Some(&plan.parameters))?;
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 (relocated)
    append_add_rsp(&mut bytes, reserve);
    bytes.extend([0x0f, 0xb7, 0xc0]); // movzx eax, ax (zero the upper bits)
    append_mov_r11_imm64(&mut bytes, 0); // relocated to the result region base
    bytes.extend([0x49, 0x89, 0x83]); // mov [r11 + disp32], rax
    let displacement: i32 = result_offset
        .try_into()
        .map_err(|_| Diagnostic::error("key_state result offset exceeds i32"))?;
    bytes.extend(displacement.to_le_bytes());
    Ok(bytes)
}

fn encode_file_operation<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    plan_source: HostCallPlan<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    let (pointer_index, length_index) = file_pointer_and_length_indices(operands)?;
    if operands.len() <= length_index {
        return Err(Diagnostic::error(
            "cannot encode X86_64 file operation: missing pointer/length operands",
        ));
    }
    let layout = normalized_win64_file_io_layout(plan_source)?;

    let mut bytes = Vec::new();
    append_sub_rsp(&mut bytes, layout.reserve);
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
    bytes.extend([0x4c, 0x8d, 0x4c, 0x24, layout.transferred_disp]);
    bytes.extend([0x48, 0xc7, 0x44, 0x24, layout.overlapped_disp, 0, 0, 0, 0]);
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32
    append_add_rsp(&mut bytes, layout.reserve);
    Ok(bytes)
}

/// ReadFile and WriteFile share the same five-argument Win32 signature:
/// HANDLE, buffer pointer, DWORD count, transferred-count pointer, and an
/// optional OVERLAPPED pointer. Their BOOL result is intentionally ignored by
/// this compatibility sequence.
fn normalized_win64_file_io_plan(plan_source: HostCallPlan<'_>) -> Result<CallPlan, Diagnostic> {
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(4, 4),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
        ],
        result: Some(ValueShape::integer(4, 4)),
    };
    selected_win64_composite_plan(&signature, plan_source, "ReadFile/WriteFile")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Win64FileIoLayout {
    pub(super) reserve: usize,
    pub(super) overlapped_disp: u8,
    pub(super) transferred_disp: u8,
}

pub(super) fn normalized_win64_file_io_layout(
    plan_source: HostCallPlan<'_>,
) -> Result<Win64FileIoLayout, Diagnostic> {
    let plan = normalized_win64_file_io_plan(plan_source)?;
    for (index, expected) in [
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ]
    .into_iter()
    .enumerate()
    {
        let actual = win64_argument_location(&plan.parameters[index], index)?;
        if actual != Win64ArgumentLocation::Register(expected) {
            return Err(Diagnostic::error(format!(
                "Win64 file-I/O parameter {index} requires {expected:?}, got {actual:?}"
            )));
        }
    }
    let overlapped_location = win64_argument_location(&plan.parameters[4], 4)?;
    let Win64ArgumentLocation::Stack(overlapped_offset) = overlapped_location else {
        return Err(Diagnostic::error(format!(
            "Win64 file-I/O encoder requires OVERLAPPED on the stack, got {overlapped_location:?}"
        )));
    };
    let native_result = normalized_win64_result_register(&plan, true)?;
    if native_result != Some(MachineRegister::X86Rax) {
        return Err(Diagnostic::error(format!(
            "Win64 file-I/O encoder requires its native BOOL result in rax, got {native_result:?}"
        )));
    }
    let transferred_offset = overlapped_offset
        .checked_add(8)
        .ok_or_else(|| Diagnostic::error("Win64 file-I/O temporary stack offset overflowed"))?;
    Ok(Win64FileIoLayout {
        reserve: win64_composite_reserve(transferred_offset + 8)?,
        transferred_disp: u8::try_from(transferred_offset).map_err(|_| {
            Diagnostic::error("Win64 file-I/O transferred-count slot exceeds disp8")
        })?,
        overlapped_disp: u8::try_from(overlapped_offset)
            .map_err(|_| Diagnostic::error("Win64 file-I/O OVERLAPPED slot exceeds disp8"))?,
    })
}

pub(super) fn validate_normalized_win64_get_std_handle_plan(
    plan_source: HostCallPlan<'_>,
) -> Result<(), Diagnostic> {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(4, 4)],
        result: Some(ValueShape::integer(8, 8)),
    };
    let plan = selected_win64_composite_plan(&signature, plan_source, "GetStdHandle")?;
    let argument = win64_argument_location(&plan.parameters[0], 0)?;
    let result = normalized_win64_result_register(&plan, true)?;
    if argument != Win64ArgumentLocation::Register(MachineRegister::X86Rcx)
        || result != Some(MachineRegister::X86Rax)
    {
        return Err(Diagnostic::error(format!(
            "Win64 GetStdHandle encoder cannot realize argument={argument:?}, result={result:?}"
        )));
    }
    Ok(())
}

fn selected_win64_composite_plan(
    signature: &CallSignature,
    plan_source: HostCallPlan<'_>,
    label: &str,
) -> Result<CallPlan, Diagnostic> {
    match plan_source {
        HostCallPlan::Authoritative(plan) => {
            validate_win64_encoder_plan(plan)?;
            validate_call_plan(plan, signature).map_err(|error| {
                Diagnostic::error(format!(
                    "retained Win64 {label} plan does not match its concrete native signature: {error}"
                ))
            })?;
            Ok(plan.clone())
        }
        HostCallPlan::CompatibilityOracle => evaluate_normalized_win64_plan(signature),
    }
}

pub fn validate_win64_runtime_file_adapter_plans(
    get_std_handle_plan: &CallPlan,
    file_io_plan: &CallPlan,
) -> Result<(), Diagnostic> {
    validate_normalized_win64_get_std_handle_plan(HostCallPlan::Authoritative(
        get_std_handle_plan,
    ))?;
    normalized_win64_file_io_layout(HostCallPlan::Authoritative(file_io_plan))?;
    Ok(())
}

pub fn validate_win64_runtime_file_adapter_no_plan() -> Result<(), Diagnostic> {
    validate_normalized_win64_get_std_handle_plan(HostCallPlan::CompatibilityOracle)?;
    normalized_win64_file_io_layout(HostCallPlan::CompatibilityOracle)?;
    Ok(())
}

/// Reserve through a composite call's final local byte while preserving the
/// encoder's entry invariant: rsp is 8 mod 16 before `sub`, so the reservation
/// itself must also be 8 mod 16 at the call boundary.
fn win64_composite_reserve(required_bytes: u32) -> Result<usize, Diagnostic> {
    let required = usize::try_from(required_bytes)
        .map_err(|_| Diagnostic::error("Win64 composite stack reservation exceeds usize"))?;
    let remainder = required % 16;
    let padding = (8 + 16 - remainder) % 16;
    Ok(required + padding)
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
/// (mov-imm32 opcode bytes, load-from-[r11+disp32] opcode bytes) pairs:
/// rcx, rdx, r8, r9. Immediates use the 32-bit `mov r32, imm32` forms (the
/// kernel32 surface is u32-shaped today); loads are 64-bit `mov r64,
/// [r11+disp32]` (callees read the low 32 bits).
const WIN64_ARG_REGISTERS: [(&[u8], &[u8]); 4] = [
    (&[0xb9], &[0x49, 0x8b, 0x8b]), // mov ecx, imm32 / mov rcx, [r11+d]
    (&[0xba], &[0x49, 0x8b, 0x93]), // mov edx, imm32 / mov rdx, [r11+d]
    (&[0x41, 0xb8], &[0x4d, 0x8b, 0x83]), // mov r8d, imm32 / mov r8,  [r11+d]
    (&[0x41, 0xb9], &[0x4d, 0x8b, 0x8b]), // mov r9d, imm32 / mov r9,  [r11+d]
];

/// `lea <reg64>, [r11+disp32]` opcode bytes for the Win64 integer argument
/// registers rcx/rdx/r8/r9 -- `WIN64_ARG_REGISTERS`' load opcodes with the mov
/// (8B) swapped for lea (8D), byte-for-byte the same width.
const WIN64_ARG_LEA_OPCODES: [&[u8]; 4] = [
    &[0x49, 0x8d, 0x8b], // lea rcx, [r11+d]
    &[0x49, 0x8d, 0x93], // lea rdx, [r11+d]
    &[0x4d, 0x8d, 0x83], // lea r8,  [r11+d]
    &[0x4d, 0x8d, 0x8b], // lea r9,  [r11+d]
];

/// `mov <reg64>, imm64` opcode bytes for the Win64 integer argument registers
/// rcx/rdx/r8/r9 -- a DATA-ADDRESS argument (a string-literal path, e.g.
/// `_open("...")`) marshals as the data symbol's absolute address, imm64=0
/// relocated Absolute64 at the opcode's +2 (the same imm64 position as the
/// staged `mov r11, imm64` forms, so the relocation-site walker treats all
/// three identically).
const WIN64_ARG_MOV_IMM64_OPCODES: [&[u8]; 4] = [
    &[0x48, 0xb9], // mov rcx, imm64
    &[0x48, 0xba], // mov rdx, imm64
    &[0x49, 0xb8], // mov r8,  imm64
    &[0x49, 0xb9], // mov r9,  imm64
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
    win64_import_reserve_bytes(WIN64_STACK_ARG_HOME + 8 * stack_slots)
}

fn win64_import_reserve_for_plan(plan: &CallPlan) -> usize {
    let stack_bytes = plan
        .parameters
        .iter()
        .flat_map(|placement| placement.locations.iter())
        .map(|location| match location {
            ValueLocation::Register { .. } => 0,
            ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } => *stack_byte_offset as usize + usize::from((*byte_size).max(8)),
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                ..
            } => {
                let pointer_end = match pointer {
                    IndirectPointerLocation::Register(_) => 0,
                    IndirectPointerLocation::Stack {
                        stack_byte_offset, ..
                    } => *stack_byte_offset as usize + 8,
                };
                let copy_end = copy_stack_byte_offset
                    .map(|offset| offset as usize + usize::from(*byte_size))
                    .unwrap_or(0);
                pointer_end.max(copy_end)
            }
        })
        .max()
        .unwrap_or(0)
        .max(usize::from(plan.shadow_bytes));
    win64_import_reserve_bytes(stack_bytes)
}

fn win64_import_reserve_bytes(stack_bytes: usize) -> usize {
    // Emitted Omega call sites enter with rsp == 8 (mod 16). Reserve the
    // smallest area that covers every slot/copy and leaves rsp 16-byte aligned
    // immediately before CALL, including odd-sized indirect record copies.
    (stack_bytes + 8).next_multiple_of(16) - 8
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

/// Whether a general-import argument operand marshals through the relocated r11
/// region base (a runtime-storage scalar LOAD or a runtime-storage ADDRESS lea)
/// rather than as a constant immediate.
fn win64_import_arg_is_staged<T: InstructionOperandLike>(operand: Option<&T>) -> bool {
    operand.is_some_and(|operand| {
        operand.runtime_scalar_integer().is_some()
            || operand.runtime_scalar_float().is_some()
            || operand.runtime_small_aggregate().is_some()
            || operand.runtime_large_aggregate().is_some()
            || operand.runtime_storage_address().is_some()
            || operand.data_address().is_some()
            || operand.runtime_string_pointer().is_some()
    })
}

/// Whether a general-import argument is a data-object address (`mov <reg>,
/// imm64` relocated to the symbol, no r11 staging) -- narrower than
/// `win64_import_arg_is_staged`, which also covers the r11-staged forms; both
/// place their relocated imm64 at the argument's start + 2.
fn win64_import_arg_is_data_address<T: InstructionOperandLike>(operand: Option<&T>) -> bool {
    operand.is_some_and(|operand| operand.data_address().is_some())
}

/// Marshalling width of general-import argument `index` (0-based ABI order,
/// stored at `operands[arg_start + index]`). For register args, an address lea
/// is the same width as a scalar load; a data-object address is one
/// `mov <reg64>, imm64` (10). Stack args
/// stage through r11/rax (10 + 7 + a 5-byte `mov [rsp+disp8], rax`; a data
/// address is 10 + 5), or store a constant directly (9-byte
/// `mov qword [rsp+disp8], imm32`).
fn win64_import_arg_width<T: InstructionOperandLike>(
    operands: &[T],
    arg_start: usize,
    index: usize,
    placement: Option<&ValuePlacement>,
) -> usize {
    let operand = operands.get(arg_start + index);
    if let Some((_, _, byte_count)) = operand.and_then(|operand| operand.runtime_scalar_float())
        && let Some(placement) = placement
    {
        return match placement.locations.as_slice() {
            [ValueLocation::Register { .. }] => 19,
            [ValueLocation::Stack { .. }] => {
                10 + 7 + win64_direct_aggregate_stack_store_width(byte_count)
            }
            _ => 0,
        };
    }
    if let Some((_, _, byte_count, _)) = operand.and_then(win64_aggregate_operand)
        && let Some(placement) = placement
    {
        match placement.locations.as_slice() {
            [
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset: Some(_),
                    ..
                },
            ] => {
                return 10
                    + win64_indirect_aggregate_copy_width(byte_count)
                    + match pointer {
                        IndirectPointerLocation::Register(_) => 8,
                        IndirectPointerLocation::Stack { .. } => 16,
                    };
            }
            [ValueLocation::Register { .. }] => {
                return 10 + win64_direct_aggregate_load_width(byte_count);
            }
            [ValueLocation::Stack { .. }] => {
                return 10
                    + win64_direct_aggregate_load_width(byte_count)
                    + win64_direct_aggregate_stack_store_width(byte_count);
            }
            _ => {}
        }
    }
    let data_address = win64_import_arg_is_data_address(operand);
    let staged = win64_import_arg_is_staged(operand);
    if index < 4 {
        let (imm_opcode, load_opcode) = WIN64_ARG_REGISTERS[index];
        if data_address {
            10
        } else if staged {
            10 + load_opcode.len() + 4
        } else {
            imm_opcode.len() + 4
        }
    } else if data_address {
        10 + 5
    } else if staged {
        10 + 7 + 5
    } else {
        9
    }
}

fn win64_direct_aggregate_load_width(byte_count: usize) -> usize {
    7 + usize::from(byte_count == 2)
}

fn win64_direct_aggregate_stack_store_width(byte_count: usize) -> usize {
    match byte_count {
        8 | 2 => 8,
        4 | 1 => 7,
        _ => 0,
    }
}

fn win64_aggregate_operand<T: InstructionOperandLike>(
    operand: &T,
) -> Option<(RuntimeStorageRegion, usize, usize, usize)> {
    operand
        .runtime_small_aggregate()
        .or_else(|| operand.runtime_large_aggregate())
}

fn win64_indirect_aggregate_copy_width(byte_count: usize) -> usize {
    let mut copied = 0usize;
    let mut width = 0usize;
    while copied < byte_count {
        let fragment = win64_aggregate_copy_fragment_byte_count(byte_count - copied);
        width += match fragment {
            8 => 15,
            4 | 1 => 14,
            2 => 16,
            _ => unreachable!("aggregate copy fragment width is canonical"),
        };
        copied += fragment;
    }
    width
}

fn win64_aggregate_copy_fragment_byte_count(remaining: usize) -> usize {
    [8, 4, 2, 1]
        .into_iter()
        .find(|fragment| remaining >= *fragment)
        .expect("aggregate copy always has bytes remaining")
}

/// Total width of a `encode_win64_import_call` sequence -- must mirror the
/// encoder byte for byte (the relocation cursor math depends on it).
fn win64_import_call_width<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
) -> usize {
    let arg_start = usize::from(returns_value);
    let arg_count = operands.len().saturating_sub(arg_start);
    let plan = normalized_win64_import_plan(operands, returns_value).ok();
    let reserve = plan
        .as_ref()
        .map(win64_import_reserve_for_plan)
        .unwrap_or_else(|| win64_import_reserve(arg_count));
    let mut width = 2 * rsp_adjust_width(reserve) + 5;
    width += plan.as_ref().map(win64_result_pre_call_width).unwrap_or(0);
    for index in 0..arg_count {
        width += win64_import_arg_width(
            operands,
            arg_start,
            index,
            plan.as_ref().and_then(|plan| plan.parameters.get(index)),
        );
    }
    if dereferences_result {
        width += 2; // mov eax, [rax]
    }
    width += plan
        .as_ref()
        .map(win64_result_post_call_width)
        .unwrap_or_else(|| usize::from(returns_value) * 17);
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
    let Some(value) = operands
        .get(index)
        .and_then(|operand| operand.immediate_integer())
    else {
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
/// immediate, a runtime-storage scalar (loaded through the relocated r11 region
/// base), or a runtime-storage ADDRESS (`lea` through the same base -- the
/// pointer-argument shape: buffers, OS structs, C strings).
/// Marshal MS-x64 call arguments `operands[arg_start..]` into RCX/RDX/R8/R9
/// (staged runtime loads/leas through the relocated r11 region base, or plain
/// immediates) and the shadow-space stack home for args past the fourth.
/// Shared by the import call and the vtable call (their only difference is how
/// the callee address is obtained: a relocated `call rel32` vs `call rax`).
fn append_win64_aggregate_argument<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    parameter_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    match placement.locations.as_slice() {
        [ValueLocation::Register { .. } | ValueLocation::Stack { .. }] => {
            append_win64_direct_aggregate_argument(bytes, operand, parameter_index, placement)
        }
        [ValueLocation::Indirect { .. }] => {
            append_win64_indirect_aggregate_argument(bytes, operand, parameter_index, placement)
        }
        locations => Err(Diagnostic::error(format!(
            "Microsoft x64 aggregate parameter {parameter_index} has unsupported placement {locations:?}"
        ))),
    }
}

fn append_win64_float_argument<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    parameter_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() else {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 float parameter {parameter_index} is not a float operand"
        )));
    };
    if !matches!(byte_count, 4 | 8)
        || !matches!(placement.shape.class, ValueClass::Float)
        || usize::from(placement.shape.byte_size) != byte_count
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 float parameter {parameter_index} has inconsistent shape"
        )));
    }

    append_mov_r11_imm64(bytes, 0); // relocated to the float's region base
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if usize::from(*byte_size) == byte_count => {
            append_x86_load_float_from_r11(bytes, *register, byte_offset, byte_count)
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if usize::from(*byte_size) == byte_count => {
            append_win64_load_register_from_r11(
                bytes,
                MachineRegister::X86Rax,
                byte_offset,
                byte_count,
            )?;
            append_win64_store_rax_to_rsp(bytes, *stack_byte_offset, byte_count)
        }
        locations => Err(Diagnostic::error(format!(
            "Microsoft x64 float parameter {parameter_index} has unsupported placement {locations:?}"
        ))),
    }
}

fn append_win64_direct_aggregate_argument<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    parameter_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let Some((_, byte_offset, byte_count, alignment)) = win64_aggregate_operand(operand) else {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 direct parameter {parameter_index} is not an aggregate operand"
        )));
    };
    if !matches!(byte_count, 1 | 2 | 4 | 8)
        || usize::from(placement.shape.byte_size) != byte_count
        || usize::from(placement.shape.alignment) != alignment
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 direct aggregate parameter {parameter_index} has inconsistent shape"
        )));
    }

    append_mov_r11_imm64(bytes, 0); // relocated to the aggregate's region base
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if usize::from(*byte_size) == byte_count => {
            append_win64_load_register_from_r11(bytes, *register, byte_offset, byte_count)
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if usize::from(*byte_size) == byte_count => {
            append_win64_load_register_from_r11(
                bytes,
                MachineRegister::X86Rax,
                byte_offset,
                byte_count,
            )?;
            append_win64_store_rax_to_rsp(bytes, *stack_byte_offset, byte_count)
        }
        locations => Err(Diagnostic::error(format!(
            "Microsoft x64 direct aggregate parameter {parameter_index} has unsupported placement {locations:?}"
        ))),
    }
}

fn append_win64_load_register_from_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_count: usize,
) -> Result<(), Diagnostic> {
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "Microsoft x64 direct aggregate uses unsupported register {register:?}"
        ))
    })?;
    if !matches!(
        register,
        MachineRegister::X86Rax
            | MachineRegister::X86Rcx
            | MachineRegister::X86Rdx
            | MachineRegister::X86R8
            | MachineRegister::X86R9
    ) || !matches!(byte_count, 1 | 2 | 4 | 8)
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 direct aggregate cannot load {byte_count} bytes into {register:?}"
        )));
    }
    if byte_count == 2 {
        bytes.push(0x66);
    }
    bytes.extend([
        0x40 | u8::from(byte_count == 8) * 0x08 | u8::from(register_number >= 8) * 0x04 | 0x01,
        if byte_count == 1 { 0x8a } else { 0x8b },
        0x83 | ((register_number & 7) << 3),
    ]); // mov selected register, [r11+disp32]
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_win64_store_rax_to_rsp(
    bytes: &mut Vec<u8>,
    stack_byte_offset: u32,
    byte_count: usize,
) -> Result<(), Diagnostic> {
    if byte_count == 8 {
        return append_store_rax_to_rsp_disp32(bytes, stack_byte_offset);
    }
    match byte_count {
        4 => bytes.extend([0x89, 0x84, 0x24]),
        2 => bytes.extend([0x66, 0x89, 0x84, 0x24]),
        1 => bytes.extend([0x88, 0x84, 0x24]),
        _ => {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 direct aggregate stack width {byte_count} is unsupported"
            )));
        }
    }
    bytes.extend(
        i32::try_from(stack_byte_offset)
            .map_err(|_| Diagnostic::error("Microsoft x64 stack offset exceeds disp32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_win64_indirect_aggregate_argument<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    parameter_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let Some((_, byte_offset, byte_count, alignment)) = win64_aggregate_operand(operand) else {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 indirect parameter {parameter_index} is not an aggregate operand"
        )));
    };
    let [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: Some(copy_stack_byte_offset),
            byte_size,
            alignment: planned_alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 aggregate parameter {parameter_index} has no caller-copy placement"
        )));
    };
    if matches!(byte_count, 1 | 2 | 4 | 8)
        || usize::from(*byte_size) != byte_count
        || usize::from(*planned_alignment) != alignment
        || !alignment.is_power_of_two()
        || copy_stack_byte_offset % 16 != 0
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 aggregate parameter {parameter_index} has inconsistent shape or copy alignment"
        )));
    }

    append_mov_r11_imm64(bytes, 0); // relocated to the aggregate's region base
    let mut copied = 0usize;
    while copied < byte_count {
        let fragment = win64_aggregate_copy_fragment_byte_count(byte_count - copied);
        let source_offset = byte_offset
            .checked_add(copied)
            .ok_or_else(|| Diagnostic::error("Microsoft x64 aggregate source offset overflow"))?;
        match fragment {
            8 => bytes.extend([0x49, 0x8b, 0x83]), // mov rax, [r11+disp32]
            4 => bytes.extend([0x41, 0x8b, 0x83]), // mov eax, [r11+disp32]
            2 => bytes.extend([0x66, 0x41, 0x8b, 0x83]), // mov ax, [r11+disp32]
            1 => bytes.extend([0x41, 0x8a, 0x83]), // mov al, [r11+disp32]
            _ => unreachable!("aggregate copy fragment width is canonical"),
        }
        bytes.extend(disp32(source_offset)?.to_le_bytes());

        let target_offset = usize::try_from(*copy_stack_byte_offset)
            .ok()
            .and_then(|offset| offset.checked_add(copied))
            .ok_or_else(|| Diagnostic::error("Microsoft x64 aggregate copy offset overflow"))?;
        if fragment == 8 {
            append_store_rax_to_rsp_disp32(
                bytes,
                u32::try_from(target_offset).map_err(|_| {
                    Diagnostic::error("Microsoft x64 aggregate copy offset exceeds u32")
                })?,
            )?;
        } else {
            match fragment {
                4 => bytes.extend([0x89, 0x84, 0x24]), // mov [rsp+disp32], eax
                2 => bytes.extend([0x66, 0x89, 0x84, 0x24]), // mov [rsp+disp32], ax
                1 => bytes.extend([0x88, 0x84, 0x24]), // mov [rsp+disp32], al
                _ => unreachable!("aggregate copy fragment width is canonical"),
            }
            bytes.extend(disp32(target_offset)?.to_le_bytes());
        }
        copied += fragment;
    }

    match *pointer {
        IndirectPointerLocation::Register(register) => {
            bytes.extend(encode_outgoing_stack_address_load_bytes(
                register,
                *copy_stack_byte_offset,
            )?);
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            bytes.extend(encode_outgoing_stack_address_load_bytes(
                MachineRegister::X86Rax,
                *copy_stack_byte_offset,
            )?);
            bytes.extend([0x48, 0x89, 0x84, 0x24]); // mov [rsp+disp32], rax
            bytes.extend(
                i32::try_from(stack_byte_offset)
                    .map_err(|_| {
                        Diagnostic::error("Microsoft x64 pointer stack offset exceeds disp32")
                    })?
                    .to_le_bytes(),
            );
        }
    }
    Ok(())
}

fn append_win64_call_arguments<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operands: &[T],
    arg_start: usize,
    planned_parameters: Option<&[ValuePlacement]>,
) -> Result<(), Diagnostic> {
    let arg_count = operands.len() - arg_start;
    if let Some(parameters) = planned_parameters
        && parameters.len() != arg_count
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 call plan supplied {} parameter placements for {arg_count} operands",
            parameters.len()
        )));
    }
    for index in 0..arg_count {
        let operand = &operands[arg_start + index];
        if operand.runtime_scalar_float().is_some() {
            let placement = planned_parameters
                .and_then(|parameters| parameters.get(index))
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Microsoft x64 float parameter {index} has no normalized placement"
                    ))
                })?;
            append_win64_float_argument(bytes, operand, index, placement)?;
            continue;
        }
        if win64_aggregate_operand(operand).is_some() {
            let placement = planned_parameters
                .and_then(|parameters| parameters.get(index))
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Microsoft x64 aggregate parameter {index} has no normalized placement"
                    ))
                })?;
            append_win64_aggregate_argument(bytes, operand, index, placement)?;
            continue;
        }
        let planned_location = planned_parameters
            .map(|parameters| win64_argument_location(&parameters[index], index))
            .transpose()?;
        let register_slot = match planned_location {
            Some(Win64ArgumentLocation::Register(register)) => Some(
                win64_argument_register_slot(register).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Microsoft x64 import parameter {index} uses unsupported register {register:?}"
                    ))
                })?,
            ),
            Some(Win64ArgumentLocation::Stack(_)) => None,
            None if index < 4 => Some(index),
            None => None,
        };
        if let Some(register_slot) = register_slot {
            let (imm_opcode, load_opcode) = WIN64_ARG_REGISTERS[register_slot];
            if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend_from_slice(load_opcode);
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
            } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
                // A string/slice DESCRIPTOR in a storage region (a path or text
                // argument riding a runtime slot, e.g. a value-call param bound
                // to a literal): the C-string argument is the descriptor's
                // POINTER word (at +0), or the inline content after the len
                // word for an owned bounded-buffer carrier -- mirroring the
                // syscall encoder's string-pointer staging.
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                if operand.runtime_string_is_bounded_buffer() {
                    bytes.extend_from_slice(WIN64_ARG_LEA_OPCODES[register_slot]);
                    bytes.extend(disp32(byte_offset + 8)?.to_le_bytes());
                } else {
                    bytes.extend_from_slice(load_opcode);
                    bytes.extend(disp32(byte_offset)?.to_le_bytes());
                }
            } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend_from_slice(WIN64_ARG_LEA_OPCODES[register_slot]);
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
            } else if operand.data_address().is_some() {
                // A data-object address (string-literal path): the imm64 is
                // relocated Absolute64 to the symbol's address.
                bytes.extend_from_slice(WIN64_ARG_MOV_IMM64_OPCODES[register_slot]);
                bytes.extend(0u64.to_le_bytes());
            } else if let Some(length) = operand.byte_length() {
                // A literal payload's byte length rides as a plain integer.
                bytes.extend_from_slice(imm_opcode);
                bytes.extend(
                    i32::try_from(length)
                        .map_err(|_| {
                            Diagnostic::error("X86_64 call byte-length argument exceeds i32")
                        })?
                        .to_le_bytes(),
                );
            } else {
                let argument = immediate_imm32(operands, arg_start + index, "call argument")?;
                bytes.extend_from_slice(imm_opcode);
                bytes.extend(argument.to_le_bytes());
            }
        } else {
            let stack_offset = match planned_location {
                Some(Win64ArgumentLocation::Stack(stack_offset)) => stack_offset,
                Some(Win64ArgumentLocation::Register(register)) => {
                    return Err(Diagnostic::error(format!(
                        "Microsoft x64 import parameter {index} could not marshal planned register {register:?}"
                    )));
                }
                None => (WIN64_STACK_ARG_HOME + 8 * (index - 4)) as u32,
            };
            let stack_disp8 = u8::try_from(stack_offset)
                .ok()
                .filter(|_| stack_offset <= 127)
                .ok_or_else(|| Diagnostic::error("X86_64 call supports at most 16 arguments"))?;
            if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend([0x49, 0x8b, 0x83]); // mov rax, [r11+disp32]
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
                bytes.extend([0x48, 0x89, 0x44, 0x24, stack_disp8]); // mov [rsp+o], rax
            } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend([0x49, 0x8d, 0x83]); // lea rax, [r11+disp32]
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
                bytes.extend([0x48, 0x89, 0x44, 0x24, stack_disp8]); // mov [rsp+o], rax
            } else if operand.data_address().is_some() {
                bytes.extend([0x48, 0xb8]); // mov rax, imm64 (relocated Absolute64)
                bytes.extend(0u64.to_le_bytes());
                bytes.extend([0x48, 0x89, 0x44, 0x24, stack_disp8]); // mov [rsp+o], rax
            } else if let Some(length) = operand.byte_length() {
                bytes.extend([0x48, 0xc7, 0x44, 0x24, stack_disp8]); // mov qword [rsp+o], imm32
                bytes.extend(
                    i32::try_from(length)
                        .map_err(|_| {
                            Diagnostic::error("X86_64 call byte-length argument exceeds i32")
                        })?
                        .to_le_bytes(),
                );
            } else {
                let argument = immediate_imm32(operands, arg_start + index, "call argument")?;
                bytes.extend([0x48, 0xc7, 0x44, 0x24, stack_disp8]); // mov qword [rsp+o], imm32
                bytes.extend(argument.to_le_bytes());
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Win64ArgumentLocation {
    Register(MachineRegister),
    Stack(u32),
}

fn win64_argument_location(
    placement: &ValuePlacement,
    index: usize,
) -> Result<Win64ArgumentLocation, Diagnostic> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == placement.shape.byte_size => {
            Ok(Win64ArgumentLocation::Register(*register))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == placement.shape.byte_size => {
            Ok(Win64ArgumentLocation::Stack(*stack_byte_offset))
        }
        locations => Err(Diagnostic::error(format!(
            "Microsoft x64 import parameter {index} has unsupported fragmented placement {locations:?}"
        ))),
    }
}

fn win64_argument_register_slot(register: MachineRegister) -> Option<usize> {
    match register {
        MachineRegister::X86Rcx => Some(0),
        MachineRegister::X86Rdx => Some(1),
        MachineRegister::X86R8 => Some(2),
        MachineRegister::X86R9 => Some(3),
        _ => None,
    }
}

#[cfg(test)]
fn encode_win64_import_call<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
) -> Result<Vec<u8>, Diagnostic> {
    encode_win64_import_call_for_plan(
        operands,
        returns_value,
        dereferences_result,
        HostCallPlan::CompatibilityOracle,
    )
}

fn encode_win64_import_call_for_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
    plan_source: HostCallPlan<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    if returns_value && operands.is_empty() {
        return Err(Diagnostic::error(
            "cannot encode X86_64 import call: the result storage place did not lower to a \
             runtime scalar operand",
        ));
    }
    let arg_start = usize::from(returns_value);
    let plan = match plan_source {
        HostCallPlan::Authoritative(plan) => {
            validate_win64_encoder_plan(plan)?;
            validate_win64_plan_operand_shapes(plan, operands, returns_value)?;
            plan.clone()
        }
        HostCallPlan::CompatibilityOracle => normalized_win64_import_plan(operands, returns_value)?,
    };
    let indirect_result = plan.result.as_ref().is_some_and(win64_result_is_indirect);
    let result_register = if indirect_result {
        None
    } else {
        normalized_win64_result_register(&plan, returns_value)?
    };
    let reserve = win64_import_reserve_for_plan(&plan);
    let mut bytes = Vec::with_capacity(win64_import_call_width(
        operands,
        returns_value,
        dereferences_result,
    ));
    append_sub_rsp(&mut bytes, reserve);
    if indirect_result {
        append_win64_indirect_result_address(
            &mut bytes,
            &operands[0],
            plan.result.as_ref().expect("indirect result placement"),
        )?;
    }
    append_win64_call_arguments(&mut bytes, operands, arg_start, Some(&plan.parameters))?;
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 (relocated)
    append_add_rsp(&mut bytes, reserve);
    if dereferences_result {
        if result_register != Some(MachineRegister::X86Rax) {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 pointer-result dereference requires rax, got {result_register:?}"
            )));
        }
        // The callee returned a POINTER to the real result (`_errno()` returns
        // `&errno`); deref once so the store tail writes the integer.
        bytes.extend([0x8b, 0x00]); // mov eax, [rax]
    }
    if returns_value && !indirect_result {
        append_win64_result_store(
            &mut bytes,
            &operands[0],
            "import call",
            plan.result.as_ref().expect("direct result placement"),
        )?;
    }
    debug_assert_eq!(
        bytes.len(),
        win64_import_call_width(operands, returns_value, dereferences_result)
    );
    Ok(bytes)
}

#[derive(Debug)]
struct SysvImportLayout {
    bytes: Vec<u8>,
    relocation_sites: Vec<X86_64RelocationSite>,
}

/// Encode a SysV AMD64 indirect call through a function pointer field on the
/// receiver. The receiver is the first wire argument and therefore must be
/// placed in `rdi` by the normalized plan.
pub fn encode_sysv_vtable_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    Ok(sysv_field_call_layout_for_plan(
        operands,
        byte_offset,
        result_present,
        true,
        HostCallPlan::Authoritative(authoritative_plan),
    )?
    .bytes)
}

pub fn sysv_vtable_call_width_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> usize {
    sysv_field_call_layout_for_plan(
        operands,
        byte_offset,
        result_present,
        true,
        HostCallPlan::Authoritative(authoritative_plan),
    )
    .map(|layout| layout.bytes.len())
    .unwrap_or(0)
}

pub fn sysv_vtable_call_data_relocation_byte_offset_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    operand_index: usize,
    authoritative_plan: &CallPlan,
) -> usize {
    sysv_field_call_layout_for_plan(
        operands,
        byte_offset,
        result_present,
        true,
        HostCallPlan::Authoritative(authoritative_plan),
    )
    .ok()
    .and_then(|layout| {
        layout
            .relocation_sites
            .into_iter()
            .find(|site| site.operand_index == Some(operand_index))
    })
    .map(|site| site.byte_offset)
    .unwrap_or(0)
}

/// Encode a SysV AMD64 service-table call. The table operand is used only to
/// find the callee; it is deliberately excluded from the wire signature.
pub fn encode_sysv_table_function_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    Ok(sysv_field_call_layout_for_plan(
        operands,
        byte_offset,
        result_present,
        false,
        HostCallPlan::Authoritative(authoritative_plan),
    )?
    .bytes)
}

pub fn sysv_table_function_call_width_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> usize {
    sysv_field_call_layout_for_plan(
        operands,
        byte_offset,
        result_present,
        false,
        HostCallPlan::Authoritative(authoritative_plan),
    )
    .map(|layout| layout.bytes.len())
    .unwrap_or(0)
}

pub fn sysv_table_function_call_data_relocation_byte_offset_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    operand_index: usize,
    authoritative_plan: &CallPlan,
) -> usize {
    sysv_field_call_layout_for_plan(
        operands,
        byte_offset,
        result_present,
        false,
        HostCallPlan::Authoritative(authoritative_plan),
    )
    .ok()
    .and_then(|layout| {
        layout
            .relocation_sites
            .into_iter()
            .find(|site| site.operand_index == Some(operand_index))
    })
    .map(|site| site.byte_offset)
    .unwrap_or(0)
}

fn sysv_field_call_layout_for_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    passes_receiver: bool,
    plan_source: HostCallPlan<'_>,
) -> Result<SysvImportLayout, Diagnostic> {
    let result_index = result_present.then_some(0);
    let dispatch_index = usize::from(result_present);
    let argument_start = if passes_receiver {
        dispatch_index
    } else {
        dispatch_index + 1
    };
    if operands.len() <= dispatch_index {
        return Err(Diagnostic::error(if passes_receiver {
            "cannot encode SysV AMD64 vtable call without its receiver"
        } else {
            "cannot encode SysV AMD64 table-function call without its dispatch table"
        }));
    }
    if !passes_receiver
        && !matches!(
            operands[dispatch_index].runtime_scalar_integer(),
            Some((_, _, 8))
        )
    {
        return Err(Diagnostic::error(
            "SysV AMD64 table-function dispatch table must be an eight-byte runtime scalar",
        ));
    }

    let signature = CallSignature {
        parameters: operands[argument_start..]
            .iter()
            .map(sysv_operand_shape)
            .collect::<Result<Vec<_>, _>>()?,
        result: result_index
            .map(|index| sysv_operand_shape(&operands[index]))
            .transpose()?,
    };
    let plan = match plan_source {
        HostCallPlan::Authoritative(plan) => {
            validate_call_plan(plan, &signature).map_err(|error| {
                Diagnostic::error(format!(
                    "source-selected SysV AMD64 field-call plan does not match the lowered signature: {error}"
                ))
            })?;
            plan.clone()
        }
        HostCallPlan::CompatibilityOracle => {
            evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature).map_err(|error| {
                Diagnostic::error(format!(
                    "cannot evaluate SysV AMD64 field-call plan: {error}"
                ))
            })?
        }
    };
    validate_sysv_import_plan(&plan)?;

    let receiver_register = if passes_receiver {
        match plan
            .parameters
            .first()
            .map(|placement| placement.locations.as_slice())
        {
            Some(
                [
                    ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size: 8,
                    },
                ],
            ) => Some(*register),
            _ => {
                return Err(Diagnostic::error(
                    "SysV AMD64 vtable call requires one full-width register receiver",
                ));
            }
        }
    } else {
        None
    };

    let stack_bytes = plan
        .parameters
        .iter()
        .flat_map(|placement| placement.locations.iter())
        .filter_map(|location| match location {
            ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } => Some(usize::try_from(*stack_byte_offset).ok()? + usize::from(*byte_size)),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let reserve = sysv_import_reserve(stack_bytes);
    let mut bytes = Vec::new();
    let mut relocation_sites = Vec::new();
    append_sub_rsp(&mut bytes, reserve);
    if let (Some(result_index), Some(result)) = (result_index, plan.result.as_ref())
        && sysv_result_is_indirect(result)
    {
        append_sysv_indirect_result_address(
            &mut bytes,
            &mut relocation_sites,
            &operands[result_index],
            result_index,
            result,
        )?;
    }
    for (parameter_index, placement) in plan.parameters.iter().enumerate() {
        let operand_index = argument_start + parameter_index;
        append_sysv_parameter(
            &mut bytes,
            &mut relocation_sites,
            &operands[operand_index],
            operand_index,
            placement,
        )?;
    }

    let field_disp = i32::try_from(byte_offset)
        .map_err(|_| Diagnostic::error("indirect field offset exceeds an imm32"))?;
    if passes_receiver {
        append_sysv_load_rax_from_base(
            &mut bytes,
            receiver_register.expect("validated receiver register"),
            field_disp,
        )?;
    } else {
        let (_, table_slot_offset, _) = operands[dispatch_index]
            .runtime_scalar_integer()
            .expect("validated table operand");
        append_sysv_runtime_base(&mut bytes, &mut relocation_sites, dispatch_index);
        bytes.extend([0x49, 0x8b, 0x83]); // mov rax, [r11 + disp32]
        bytes.extend(disp32(table_slot_offset)?.to_le_bytes());
        bytes.extend([0x48, 0x8b, 0x80]); // mov rax, [rax + disp32]
        bytes.extend(field_disp.to_le_bytes());
    }
    append_call_register(&mut bytes, 0);
    append_add_rsp(&mut bytes, reserve);

    if let Some(result_index) = result_index
        && !plan.result.as_ref().is_some_and(sysv_result_is_indirect)
    {
        append_sysv_result(
            &mut bytes,
            &mut relocation_sites,
            &operands[result_index],
            plan.result.as_ref().ok_or_else(|| {
                Diagnostic::error("SysV AMD64 field-call plan omitted its required result")
            })?,
        )?;
    }
    Ok(SysvImportLayout {
        bytes,
        relocation_sites,
    })
}

/// The normalized SysV AMD64 import slice. Provides-authored calls may carry
/// four/eight-byte integer or float scalars, pointers, and pure-INTEGER records
/// of at most two eightbytes whose fragments are four/eight bytes. The
/// evaluated plan owns the independent GPR/XMM banks, whole-value stack
/// rollback, and `rax`/`rdx`/`xmm0` results; this encoder only realizes those
/// locations. Vector and mixed-class aggregate cases stay closed.
#[cfg(test)]
fn sysv_import_layout<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Result<SysvImportLayout, Diagnostic> {
    sysv_import_layout_for_plan(operands, returns_value, HostCallPlan::CompatibilityOracle)
}

fn sysv_import_layout_for_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    plan_source: HostCallPlan<'_>,
) -> Result<SysvImportLayout, Diagnostic> {
    if returns_value && operands.is_empty() {
        return Err(Diagnostic::error(
            "cannot encode SysV AMD64 import call without its result storage operand",
        ));
    }
    let arg_start = usize::from(returns_value);
    let plan = match plan_source {
        HostCallPlan::Authoritative(plan) => {
            validate_sysv_import_plan(plan)?;
            validate_sysv_plan_operand_shapes(plan, operands, returns_value)?;
            plan.clone()
        }
        HostCallPlan::CompatibilityOracle => normalized_sysv_import_plan(operands, returns_value)?,
    };
    let stack_bytes = plan
        .parameters
        .iter()
        .flat_map(|placement| placement.locations.iter())
        .filter_map(|location| match location {
            ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } => Some(usize::try_from(*stack_byte_offset).ok()? + usize::from(*byte_size)),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let reserve = sysv_import_reserve(stack_bytes);
    let mut bytes = Vec::new();
    let mut relocation_sites = Vec::new();
    append_sub_rsp(&mut bytes, reserve);

    if returns_value
        && let Some(result) = plan.result.as_ref()
        && sysv_result_is_indirect(result)
    {
        append_sysv_indirect_result_address(
            &mut bytes,
            &mut relocation_sites,
            &operands[0],
            0,
            result,
        )?;
    }

    for (parameter_index, placement) in plan.parameters.iter().enumerate() {
        append_sysv_parameter(
            &mut bytes,
            &mut relocation_sites,
            &operands[arg_start + parameter_index],
            arg_start + parameter_index,
            placement,
        )?;
    }

    relocation_sites.push(X86_64RelocationSite {
        operand_index: None,
        byte_offset: bytes.len() + 1,
        byte_width: 4,
        kind: X86_64RelocationSiteKind::Relative32,
    });
    bytes.extend([0xe8, 0, 0, 0, 0]);
    append_add_rsp(&mut bytes, reserve);

    if returns_value && !plan.result.as_ref().is_some_and(sysv_result_is_indirect) {
        append_sysv_result(
            &mut bytes,
            &mut relocation_sites,
            &operands[0],
            plan.result.as_ref().ok_or_else(|| {
                Diagnostic::error("SysV AMD64 import plan omitted its required result")
            })?,
        )?;
    }

    Ok(SysvImportLayout {
        bytes,
        relocation_sites,
    })
}

fn sysv_import_reserve(stack_bytes: usize) -> usize {
    // Emitted Omega call sites enter with rsp == 8 (mod 16). Reserve the
    // smallest area that covers every outgoing stack slot and leaves rsp
    // 16-byte aligned immediately before CALL.
    (stack_bytes + 8).next_multiple_of(16) - 8
}

fn append_sysv_parameter<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    operand_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    if let Some((_, byte_offset, byte_count, _, sse_eightbytes)) =
        operand.runtime_system_v_aggregate()
    {
        if byte_count != usize::from(placement.shape.byte_size)
            || !matches!(sse_eightbytes, 0b01 | 0b10 | 0b11)
        {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 classified aggregate operand {operand_index} disagrees with its plan"
            )));
        }
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => {
                    let source_offset = byte_offset + usize::from(value_byte_offset);
                    if matches!(register, MachineRegister::X86Xmm(_)) {
                        append_x86_load_float_from_r11(
                            bytes,
                            register,
                            source_offset,
                            usize::from(byte_size),
                        )?;
                    } else {
                        append_sysv_load_register_from_r11(
                            bytes,
                            register,
                            source_offset,
                            byte_size,
                        )?;
                    }
                }
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => {
                    append_sysv_load_rax_from_r11(
                        bytes,
                        byte_offset + usize::from(value_byte_offset),
                        byte_size,
                    )?;
                    append_sysv_store_rax_to_rsp(bytes, stack_byte_offset)?;
                }
                ValueLocation::Indirect { .. } => {
                    return Err(Diagnostic::error(
                        "SysV AMD64 classified aggregate received an indirect placement",
                    ));
                }
            }
        }
        return Ok(());
    }
    if let Some((_, byte_offset, member_byte_count, members)) =
        operand.runtime_homogeneous_float_aggregate()
    {
        if member_byte_count * usize::from(members) != usize::from(placement.shape.byte_size) {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 float aggregate operand {operand_index} disagrees with its plan width"
            )));
        }
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => append_x86_load_float_from_r11(
                    bytes,
                    register,
                    byte_offset + usize::from(value_byte_offset),
                    usize::from(byte_size),
                )?,
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => {
                    append_sysv_load_rax_from_r11(
                        bytes,
                        byte_offset + usize::from(value_byte_offset),
                        byte_size,
                    )?;
                    append_sysv_store_rax_to_rsp(bytes, stack_byte_offset)?;
                }
                ValueLocation::Indirect { .. } => {
                    return Err(Diagnostic::error(
                        "SysV AMD64 float aggregate received an indirect placement",
                    ));
                }
            }
        }
        return Ok(());
    }
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() {
        let [location] = placement.locations.as_slice() else {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 float operand {operand_index} has fragmented placement {:?}",
                placement.locations
            )));
        };
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        return match *location {
            ValueLocation::Register { register, .. } => {
                append_x86_load_float_from_r11(bytes, register, byte_offset, byte_count)
            }
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => {
                append_sysv_load_rax_from_r11(
                    bytes,
                    byte_offset,
                    u16::try_from(byte_count)
                        .map_err(|_| Diagnostic::error("SysV AMD64 float width exceeds u16"))?,
                )?;
                append_sysv_store_rax_to_rsp(bytes, stack_byte_offset)
            }
            ValueLocation::Indirect { .. } => Err(Diagnostic::error(
                "SysV AMD64 scalar float import received an indirect placement",
            )),
        };
    }
    if let Some((_, byte_offset, byte_count, _)) = operand
        .runtime_small_aggregate()
        .or_else(|| operand.runtime_large_aggregate())
    {
        if byte_count != usize::from(placement.shape.byte_size) {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 aggregate operand {operand_index} width {byte_count} disagrees with plan width {}",
                placement.shape.byte_size
            )));
        }
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => append_sysv_load_register_from_r11(
                    bytes,
                    register,
                    byte_offset + usize::from(value_byte_offset),
                    byte_size,
                )?,
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => {
                    append_sysv_load_rax_from_r11(
                        bytes,
                        byte_offset + usize::from(value_byte_offset),
                        byte_size,
                    )?;
                    append_sysv_store_rax_to_rsp(bytes, stack_byte_offset)?;
                }
                ValueLocation::Indirect { .. } => {
                    return Err(Diagnostic::error(
                        "SysV AMD64 small-aggregate import received an indirect placement",
                    ));
                }
            }
        }
        return Ok(());
    }

    let [location] = placement.locations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "SysV AMD64 scalar import operand {operand_index} has fragmented placement {:?}",
            placement.locations
        )));
    };
    match *location {
        ValueLocation::Register { register, .. } => append_sysv_scalar_to_register(
            bytes,
            relocation_sites,
            operand,
            operand_index,
            register,
        ),
        ValueLocation::Stack {
            stack_byte_offset, ..
        } => append_sysv_scalar_to_stack(
            bytes,
            relocation_sites,
            operand,
            operand_index,
            stack_byte_offset,
        ),
        ValueLocation::Indirect { .. } => Err(Diagnostic::error(
            "SysV AMD64 scalar import received an indirect placement",
        )),
    }
}

fn append_sysv_scalar_to_register<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    operand_index: usize,
    register: MachineRegister,
) -> Result<(), Diagnostic> {
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_integer() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        return append_sysv_load_register_from_r11(
            bytes,
            register,
            byte_offset,
            u16::try_from(byte_count)
                .map_err(|_| Diagnostic::error("SysV AMD64 scalar width exceeds u16"))?,
        );
    }
    if let Some((_, byte_offset)) = operand.runtime_storage_address() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        return append_sysv_lea_register_from_r11(bytes, register, byte_offset);
    }
    if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        return if operand.runtime_string_is_bounded_buffer() {
            append_sysv_lea_register_from_r11(bytes, register, byte_offset + 8)
        } else {
            append_sysv_load_register_from_r11(bytes, register, byte_offset, 8)
        };
    }
    if operand.data_address().is_some() {
        relocation_sites.push(X86_64RelocationSite {
            operand_index: Some(operand_index),
            byte_offset: bytes.len() + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        return append_sysv_mov_register_imm64(bytes, register, 0);
    }
    if let Some(value) = operand.immediate_integer().or_else(|| {
        operand
            .byte_length()
            .and_then(|value| i64::try_from(value).ok())
    }) {
        return append_sysv_mov_register_imm64(bytes, register, value as u64);
    }
    Err(Diagnostic::error(format!(
        "SysV AMD64 import operand {operand_index} has no supported integer representation"
    )))
}

fn append_sysv_scalar_to_stack<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    operand_index: usize,
    stack_byte_offset: u32,
) -> Result<(), Diagnostic> {
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_integer() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        append_sysv_load_rax_from_r11(
            bytes,
            byte_offset,
            u16::try_from(byte_count)
                .map_err(|_| Diagnostic::error("SysV AMD64 scalar width exceeds u16"))?,
        )?;
        return append_sysv_store_rax_to_rsp(bytes, stack_byte_offset);
    }
    if let Some((_, byte_offset)) = operand.runtime_storage_address() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        append_sysv_lea_register_from_r11(bytes, MachineRegister::X86Rax, byte_offset)?;
        return append_sysv_store_rax_to_rsp(bytes, stack_byte_offset);
    }
    if operand.data_address().is_some() {
        relocation_sites.push(X86_64RelocationSite {
            operand_index: Some(operand_index),
            byte_offset: bytes.len() + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        append_mov_rax_imm64(bytes, 0);
        return append_sysv_store_rax_to_rsp(bytes, stack_byte_offset);
    }
    if let Some(value) = operand.immediate_integer().or_else(|| {
        operand
            .byte_length()
            .and_then(|value| i64::try_from(value).ok())
    }) {
        append_mov_rax_imm64(bytes, value as u64);
        return append_sysv_store_rax_to_rsp(bytes, stack_byte_offset);
    }
    Err(Diagnostic::error(format!(
        "SysV AMD64 stack operand {operand_index} has no supported integer representation"
    )))
}

fn append_sysv_result<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    if let Some((_, byte_offset, byte_count, _, sse_eightbytes)) =
        operand.runtime_system_v_aggregate()
    {
        if byte_count != usize::from(placement.shape.byte_size)
            || !matches!(sse_eightbytes, 0b01 | 0b10 | 0b11)
        {
            return Err(Diagnostic::error(
                "SysV AMD64 classified aggregate result disagrees with its plan",
            ));
        }
        append_sysv_runtime_base(bytes, relocation_sites, 0);
        for location in &placement.locations {
            let ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } = *location
            else {
                return Err(Diagnostic::error(
                    "SysV AMD64 classified aggregate result is not register-resident",
                ));
            };
            let destination_offset = byte_offset + usize::from(value_byte_offset);
            if matches!(register, MachineRegister::X86Xmm(_)) {
                append_x86_store_float_to_r11(
                    bytes,
                    register,
                    destination_offset,
                    usize::from(byte_size),
                )?;
            } else {
                append_sysv_store_result_register_to_r11(
                    bytes,
                    register,
                    destination_offset,
                    byte_size,
                )?;
            }
        }
        return Ok(());
    }
    if let Some((_, byte_offset, member_byte_count, members)) =
        operand.runtime_homogeneous_float_aggregate()
    {
        if member_byte_count * usize::from(members) != usize::from(placement.shape.byte_size) {
            return Err(Diagnostic::error(
                "SysV AMD64 float aggregate result disagrees with its plan width",
            ));
        }
        append_sysv_runtime_base(bytes, relocation_sites, 0);
        for location in &placement.locations {
            let ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } = *location
            else {
                return Err(Diagnostic::error(
                    "SysV AMD64 float aggregate result is not register-resident",
                ));
            };
            append_x86_store_float_to_r11(
                bytes,
                register,
                byte_offset + usize::from(value_byte_offset),
                usize::from(byte_size),
            )?;
        }
        return Ok(());
    }
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() {
        if byte_count != usize::from(placement.shape.byte_size) {
            return Err(Diagnostic::error(
                "SysV AMD64 float result storage disagrees with the normalized result width",
            ));
        }
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = placement.locations.as_slice()
        else {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 float result has unsupported placement {:?}",
                placement.locations
            )));
        };
        append_sysv_runtime_base(bytes, relocation_sites, 0);
        return append_x86_store_float_to_r11(
            bytes,
            *register,
            byte_offset,
            usize::from(*byte_size),
        );
    }
    let (byte_offset, byte_count, aggregate) =
        if let Some((_, offset, count, _)) = operand.runtime_small_aggregate() {
            (offset, count, true)
        } else if let Some((_, offset, count)) = operand.runtime_scalar_integer() {
            (offset, count, false)
        } else {
            return Err(Diagnostic::error(
                "SysV AMD64 import result did not lower to integer runtime storage",
            ));
        };
    if byte_count != usize::from(placement.shape.byte_size) {
        return Err(Diagnostic::error(
            "SysV AMD64 import result storage disagrees with the normalized result width",
        ));
    }
    if !aggregate && placement.locations.len() != 1 {
        return Err(Diagnostic::error(
            "SysV AMD64 scalar import result has fragmented placement",
        ));
    }
    append_sysv_runtime_base(bytes, relocation_sites, 0);
    for location in &placement.locations {
        let ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } = *location
        else {
            return Err(Diagnostic::error(
                "SysV AMD64 import result is not register-resident",
            ));
        };
        append_sysv_store_result_register_to_r11(
            bytes,
            register,
            byte_offset + usize::from(value_byte_offset),
            byte_size,
        )?;
    }
    Ok(())
}

fn sysv_result_is_indirect(placement: &ValuePlacement) -> bool {
    matches!(
        placement.locations.as_slice(),
        [ValueLocation::Indirect { .. }]
    )
}

fn append_sysv_indirect_result_address<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    operand_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let [
        ValueLocation::Indirect {
            pointer: omega_calling_conventions::IndirectPointerLocation::Register(register),
            copy_stack_byte_offset: None,
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(Diagnostic::error(
            "SysV AMD64 indirect result has an unsupported pointer placement",
        ));
    };
    let Some((_, byte_offset, operand_byte_size, operand_alignment)) =
        operand.runtime_large_aggregate()
    else {
        return Err(Diagnostic::error(
            "SysV AMD64 indirect result did not lower to large-aggregate runtime storage",
        ));
    };
    if operand_byte_size != usize::from(*byte_size) || operand_alignment != usize::from(*alignment)
    {
        return Err(Diagnostic::error(
            "SysV AMD64 indirect result storage disagrees with the normalized result shape",
        ));
    }
    append_sysv_runtime_base(bytes, relocation_sites, operand_index);
    append_sysv_lea_register_from_r11(bytes, *register, byte_offset)
}

fn append_sysv_load_rax_from_base(
    bytes: &mut Vec<u8>,
    base: MachineRegister,
    displacement: i32,
) -> Result<(), Diagnostic> {
    let (rex, modrm) = match base {
        MachineRegister::X86Rdi => (0x48, 0x87),
        MachineRegister::X86Rsi => (0x48, 0x86),
        MachineRegister::X86Rdx => (0x48, 0x82),
        MachineRegister::X86Rcx => (0x48, 0x81),
        MachineRegister::X86R8 => (0x49, 0x80),
        MachineRegister::X86R9 => (0x49, 0x81),
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 vtable receiver register {base:?} is not encodable"
            )));
        }
    };
    bytes.extend([rex, 0x8b, modrm]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_sysv_runtime_base(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand_index: usize,
) {
    relocation_sites.push(X86_64RelocationSite {
        operand_index: Some(operand_index),
        byte_offset: bytes.len() + 2,
        byte_width: 8,
        kind: X86_64RelocationSiteKind::Absolute64,
    });
    append_mov_r11_imm64(bytes, 0);
}

fn normalized_sysv_import_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Result<CallPlan, Diagnostic> {
    let arg_start = usize::from(returns_value);
    let signature = CallSignature {
        parameters: operands[arg_start..]
            .iter()
            .map(sysv_operand_shape)
            .collect::<Result<Vec<_>, _>>()?,
        result: returns_value
            .then(|| sysv_operand_shape(&operands[0]))
            .transpose()?,
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature).map_err(|error| {
        Diagnostic::error(format!("cannot evaluate SysV AMD64 import plan: {error}"))
    })?;
    validate_sysv_import_plan(&plan)?;
    Ok(plan)
}

fn validate_sysv_plan_operand_shapes<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
    returns_value: bool,
) -> Result<(), Diagnostic> {
    let arg_start = usize::from(returns_value);
    let parameter_shapes = operands
        .get(arg_start..)
        .ok_or_else(|| Diagnostic::error("SysV AMD64 authored import has no arguments"))?
        .iter()
        .map(sysv_operand_shape)
        .collect::<Result<Vec<_>, _>>()?;
    let result_shape = if returns_value {
        Some(sysv_operand_shape(operands.first().ok_or_else(|| {
            Diagnostic::error("SysV AMD64 authored import has no result operand")
        })?)?)
    } else {
        None
    };
    if plan.parameters.len() != parameter_shapes.len()
        || plan
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .ne(parameter_shapes)
        || plan.result.as_ref().map(|placement| placement.shape) != result_shape
    {
        return Err(Diagnostic::error(
            "SysV AMD64 source calling plan does not match the selected authored import operands",
        ));
    }
    Ok(())
}

fn sysv_operand_shape<T: InstructionOperandLike>(operand: &T) -> Result<ValueShape, Diagnostic> {
    if let Some((_, _, byte_count, alignment, sse_eightbytes)) =
        operand.runtime_system_v_aggregate()
    {
        if !matches!(byte_count, 9..=16) || !matches!(sse_eightbytes, 0b01 | 0b10 | 0b11) {
            return Err(Diagnostic::error(
                "SysV AMD64 classified aggregates require 9-16 bytes and at least one SSE eightbyte",
            ));
        }
        let class = |index: u8| {
            if sse_eightbytes & (1u8 << index) == 0 {
                SystemVEightbyteClass::Integer
            } else {
                SystemVEightbyteClass::Sse
            }
        };
        return Ok(ValueShape::system_v_aggregate(
            u16::try_from(byte_count)
                .map_err(|_| Diagnostic::error("SysV AMD64 mixed aggregate width exceeds u16"))?,
            u16::try_from(alignment).map_err(|_| {
                Diagnostic::error("SysV AMD64 mixed aggregate alignment exceeds u16")
            })?,
            class(0),
            class(1),
        ));
    }
    if let Some((_, _, member_byte_count, members)) = operand.runtime_homogeneous_float_aggregate()
    {
        if !matches!(member_byte_count, 4 | 8)
            || !(2..=4).contains(&members)
            || member_byte_count * usize::from(members) > 16
        {
            return Err(Diagnostic::error(
                "SysV AMD64 float aggregates require two to four f32/f64 members totaling at most 16 bytes",
            ));
        }
        return Ok(ValueShape::homogeneous_float_aggregate(
            u16::try_from(member_byte_count)
                .map_err(|_| Diagnostic::error("SysV AMD64 float member width exceeds u16"))?,
            members,
        ));
    }
    if let Some((_, _, byte_count)) = operand.runtime_scalar_float() {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("SysV AMD64 float width exceeds u16"))?;
        return Ok(ValueShape::float(byte_count));
    }
    if let Some((_, _, byte_count, alignment)) = operand
        .runtime_small_aggregate()
        .or_else(|| operand.runtime_large_aggregate())
    {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("SysV AMD64 aggregate width exceeds u16"))?;
        let alignment = u16::try_from(alignment)
            .map_err(|_| Diagnostic::error("SysV AMD64 aggregate alignment exceeds u16"))?;
        if byte_count == 0 {
            return Err(Diagnostic::error(
                "SysV AMD64 aggregate calls require a nonzero value width",
            ));
        }
        return Ok(ValueShape::integer(byte_count, alignment));
    }
    if let Some((_, _, byte_count)) = operand.runtime_scalar_integer() {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("SysV AMD64 integer width exceeds u16"))?;
        return Ok(ValueShape::integer(byte_count, byte_count.max(1)));
    }
    if operand.data_address().is_some()
        || operand.runtime_string_pointer().is_some()
        || operand.runtime_storage_address().is_some()
        || operand.immediate_integer().is_some()
        || operand.byte_length().is_some()
    {
        return Ok(ValueShape::integer(8, 8));
    }
    Err(Diagnostic::error(
        "SysV AMD64 authored import operand has no supported integer/pointer shape",
    ))
}

fn validate_sysv_import_plan(plan: &CallPlan) -> Result<(), Diagnostic> {
    if plan.policy != CallingPolicy::SystemVAMD64
        || plan.entry_control != EntryControl::CallReturn
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
    {
        return Err(Diagnostic::error(format!(
            "SysV AMD64 import encoder cannot realize plan policy={:?}, control={:?}, alignment={}, shadow_bytes={}",
            plan.policy, plan.entry_control, plan.stack_alignment, plan.shadow_bytes
        )));
    }
    for scratch in [MachineRegister::X86Rax, MachineRegister::X86R11] {
        if !plan.ordinary_clobbers.contains(scratch) {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 encoder scratch register {scratch:?} exceeds the plan's ordinary-clobber ceiling"
            )));
        }
    }
    let unsupported_parameter = plan.parameters.iter().any(|placement| {
        !matches!(
            placement.shape.class,
            ValueClass::Integer
                | ValueClass::Float
                | ValueClass::HomogeneousFloatAggregate { members: 2..=4 }
                | ValueClass::SystemVAggregate { .. }
        ) || (placement.shape.byte_size > 16
            && placement
                .locations
                .iter()
                .any(|location| !matches!(location, ValueLocation::Stack { .. })))
            || placement
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Indirect { .. }))
    });
    let unsupported_result = plan.result.as_ref().is_some_and(|placement| {
        !matches!(
            placement.shape.class,
            ValueClass::Integer
                | ValueClass::Float
                | ValueClass::HomogeneousFloatAggregate { members: 2..=4 }
                | ValueClass::SystemVAggregate { .. }
        ) || (placement.shape.byte_size > 16
            && !matches!(
                placement.locations.as_slice(),
                [ValueLocation::Indirect {
                    pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                        MachineRegister::X86Rdi
                    ),
                    copy_stack_byte_offset: None,
                    ..
                }]
            ))
    });
    if unsupported_parameter || unsupported_result {
        return Err(Diagnostic::error(
            "SysV AMD64 import plan contains an unsupported aggregate class or indirect placement",
        ));
    }
    Ok(())
}

fn append_sysv_mov_register_imm64(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    value: u64,
) -> Result<(), Diagnostic> {
    let opcode = match register {
        MachineRegister::X86Rax => [0x48, 0xb8],
        MachineRegister::X86Rcx => [0x48, 0xb9],
        MachineRegister::X86Rdx => [0x48, 0xba],
        MachineRegister::X86Rsi => [0x48, 0xbe],
        MachineRegister::X86Rdi => [0x48, 0xbf],
        MachineRegister::X86R8 => [0x49, 0xb8],
        MachineRegister::X86R9 => [0x49, 0xb9],
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 import cannot materialize argument register {register:?}"
            )));
        }
    };
    bytes.extend(opcode);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_x86_load_float_from_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let MachineRegister::X86Xmm(index @ 0..=7) = register else {
        return Err(Diagnostic::error(format!(
            "X86_64 call cannot load float argument register {register:?}"
        )));
    };
    let prefix = match byte_size {
        4 => 0xf3,
        8 => 0xf2,
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 scalar float width {byte_size} is not encodable"
            )));
        }
    };
    bytes.extend([prefix, 0x41, 0x0f, 0x10, 0x83 | (index << 3)]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_x86_store_float_to_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let MachineRegister::X86Xmm(index @ 0..=7) = register else {
        return Err(Diagnostic::error(format!(
            "X86_64 call cannot store float result register {register:?}"
        )));
    };
    let prefix = match byte_size {
        4 => 0xf3,
        8 => 0xf2,
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 scalar float result width {byte_size} is not encodable"
            )));
        }
    };
    bytes.extend([prefix, 0x41, 0x0f, 0x11, 0x83 | (index << 3)]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_sysv_load_register_from_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_size: u16,
) -> Result<(), Diagnostic> {
    let modrm = match register {
        MachineRegister::X86Rax => 0x83,
        MachineRegister::X86Rcx => 0x8b,
        MachineRegister::X86Rdx => 0x93,
        MachineRegister::X86Rsi => 0xb3,
        MachineRegister::X86Rdi => 0xbb,
        MachineRegister::X86R8 => 0x83,
        MachineRegister::X86R9 => 0x8b,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 import cannot load argument register {register:?}"
            )));
        }
    };
    let rex = match (byte_size, register) {
        (8, MachineRegister::X86R8 | MachineRegister::X86R9) => 0x4d,
        (8, _) => 0x49,
        (4, MachineRegister::X86R8 | MachineRegister::X86R9) => 0x45,
        (4, _) => 0x41,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 integer fragment width {byte_size} is not yet encodable"
            )));
        }
    };
    bytes.extend([rex, 0x8b, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_sysv_lea_register_from_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let modrm = match register {
        MachineRegister::X86Rax => 0x83,
        MachineRegister::X86Rcx => 0x8b,
        MachineRegister::X86Rdx => 0x93,
        MachineRegister::X86Rsi => 0xb3,
        MachineRegister::X86Rdi => 0xbb,
        MachineRegister::X86R8 => 0x83,
        MachineRegister::X86R9 => 0x8b,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 import cannot address argument register {register:?}"
            )));
        }
    };
    let rex = if matches!(register, MachineRegister::X86R8 | MachineRegister::X86R9) {
        0x4d
    } else {
        0x49
    };
    bytes.extend([rex, 0x8d, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_sysv_load_rax_from_r11(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: u16,
) -> Result<(), Diagnostic> {
    append_sysv_load_register_from_r11(bytes, MachineRegister::X86Rax, byte_offset, byte_size)
}

fn append_sysv_store_rax_to_rsp(
    bytes: &mut Vec<u8>,
    stack_byte_offset: u32,
) -> Result<(), Diagnostic> {
    let displacement = i32::try_from(stack_byte_offset)
        .map_err(|_| Diagnostic::error("SysV AMD64 stack offset exceeds i32"))?;
    bytes.extend([0x48, 0x89, 0x84, 0x24]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_sysv_store_result_register_to_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_size: u16,
) -> Result<(), Diagnostic> {
    let modrm = match register {
        MachineRegister::X86Rax => 0x83,
        MachineRegister::X86Rdx => 0x93,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 import cannot store result register {register:?}"
            )));
        }
    };
    let rex = match byte_size {
        8 => 0x49,
        4 => 0x41,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 result fragment width {byte_size} is not yet encodable"
            )));
        }
    };
    bytes.extend([rex, 0x89, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

/// ENT2c compatibility seam: evaluate the Microsoft x64 plan from the selected
/// operands before the general import encoder marshals anything. Register and
/// shadow-relative stack placements are passed into the marshaller verbatim;
/// unsupported vector/fragmented shapes fail closed.
fn normalized_win64_import_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Result<CallPlan, Diagnostic> {
    let arg_start = usize::from(returns_value);
    normalized_win64_call_plan(operands, returns_value.then_some(0), arg_start)
}

fn normalized_win64_call_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_index: Option<usize>,
    arg_start: usize,
) -> Result<CallPlan, Diagnostic> {
    let result = if let Some(result_index) = result_index {
        Some(win64_operand_shape(
            operands.get(result_index).ok_or_else(|| {
                Diagnostic::error("Microsoft x64 call result index is out of range")
            })?,
        )?)
    } else {
        None
    };
    let signature = CallSignature {
        parameters: operands
            .get(arg_start..)
            .ok_or_else(|| Diagnostic::error("Microsoft x64 call argument start is out of range"))?
            .iter()
            .map(win64_operand_shape)
            .collect::<Result<Vec<_>, _>>()?,
        result,
    };
    evaluate_normalized_win64_plan(&signature)
}

fn validate_win64_plan_operand_shapes<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
    returns_value: bool,
) -> Result<(), Diagnostic> {
    validate_win64_call_plan_operand_shapes(
        plan,
        operands,
        returns_value.then_some(0),
        usize::from(returns_value),
    )
}

fn validate_win64_call_plan_operand_shapes<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
    result_index: Option<usize>,
    arg_start: usize,
) -> Result<(), Diagnostic> {
    let arguments = operands
        .get(arg_start..)
        .ok_or_else(|| Diagnostic::error("Microsoft x64 call has no argument slice"))?;
    if arguments.len() != plan.parameters.len() {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 source calling plan has {} parameter(s) for {} selected argument(s)",
            plan.parameters.len(),
            arguments.len()
        )));
    }
    let parameters = arguments
        .iter()
        .zip(&plan.parameters)
        .map(|(operand, placement)| win64_operand_shape_for_plan(operand, placement))
        .collect::<Result<Vec<_>, _>>()?;
    let result = result_index
        .map(|index| {
            operands
                .get(index)
                .ok_or_else(|| Diagnostic::error("Microsoft x64 call result index is out of range"))
                .and_then(win64_operand_shape)
        })
        .transpose()?;
    validate_call_plan(plan, &CallSignature { parameters, result }).map_err(|error| {
        Diagnostic::error(format!(
            "Microsoft x64 source calling plan does not match the selected call operands: {error}"
        ))
    })
}

fn win64_operand_shape_for_plan<T: InstructionOperandLike>(
    operand: &T,
    placement: &ValuePlacement,
) -> Result<ValueShape, Diagnostic> {
    // Literals and compiler-derived byte counts have no independent storage
    // width. Their selected foreign parameter is the type at this ABI seam;
    // the marshaller separately proves that the concrete value fits its imm32
    // encoding. Treating every such operand as an eight-byte scratch value
    // would let compatibility reconstruction override an exact DWORD plan.
    if operand.immediate_integer().is_some() || operand.byte_length().is_some() {
        if matches!(placement.shape.class, ValueClass::Integer)
            && matches!(placement.shape.byte_size, 1 | 2 | 4 | 8)
        {
            return Ok(placement.shape);
        }
        return Err(Diagnostic::error(
            "Microsoft x64 contextual integer operand has a non-scalar planned shape",
        ));
    }
    win64_operand_shape(operand)
}

fn evaluate_normalized_win64_plan(signature: &CallSignature) -> Result<CallPlan, Diagnostic> {
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, signature).map_err(|error| {
        Diagnostic::error(format!("cannot evaluate Microsoft x64 call plan: {error}"))
    })?;
    validate_win64_encoder_plan(&plan)?;
    Ok(plan)
}

fn validate_win64_encoder_plan(plan: &CallPlan) -> Result<(), Diagnostic> {
    if plan.policy != CallingPolicy::MicrosoftX64
        || plan.entry_control != EntryControl::CallReturn
        || plan.stack_alignment != 16
        || plan.shadow_bytes != WIN64_STACK_ARG_HOME as u16
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 import encoder cannot realize plan policy={:?}, control={:?}, alignment={}, shadow_bytes={}",
            plan.policy, plan.entry_control, plan.stack_alignment, plan.shadow_bytes
        )));
    }
    for scratch in [
        MachineRegister::X86Rax,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ] {
        if !plan.ordinary_clobbers.contains(scratch) {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 encoder scratch register {scratch:?} exceeds the plan's ordinary-clobber ceiling"
            )));
        }
    }
    Ok(())
}

fn win64_operand_shape<T: InstructionOperandLike>(operand: &T) -> Result<ValueShape, Diagnostic> {
    if let Some((_, _, byte_count, alignment)) = win64_aggregate_operand(operand) {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("Microsoft x64 aggregate width exceeds u16"))?;
        let alignment = u16::try_from(alignment)
            .map_err(|_| Diagnostic::error("Microsoft x64 aggregate alignment exceeds u16"))?;
        return Ok(ValueShape::integer(byte_count, alignment));
    }
    if let Some((_, _, byte_count)) = operand.runtime_scalar_float() {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("Microsoft x64 float operand width exceeds u16"))?;
        return Ok(ValueShape::float(byte_count));
    }
    if let Some((_, _, byte_count)) = operand.runtime_scalar_integer() {
        return win64_integer_shape(byte_count, "integer operand");
    }
    if operand.data_address().is_some()
        || operand.runtime_string_pointer().is_some()
        || operand.runtime_storage_address().is_some()
        || operand.immediate_integer().is_some()
        || operand.byte_length().is_some()
    {
        return Ok(ValueShape::integer(8, 8));
    }
    Err(Diagnostic::error(
        "Microsoft x64 import operand has no normalized scalar/pointer shape",
    ))
}

fn win64_integer_shape(byte_count: usize, label: &str) -> Result<ValueShape, Diagnostic> {
    let byte_count = u16::try_from(byte_count)
        .map_err(|_| Diagnostic::error(format!("Microsoft x64 {label} width exceeds u16")))?;
    Ok(ValueShape::integer(byte_count, byte_count.max(1)))
}

fn normalized_win64_result_register(
    plan: &CallPlan,
    returns_value: bool,
) -> Result<Option<MachineRegister>, Diagnostic> {
    match (returns_value, plan.result.as_ref()) {
        (false, None) => Ok(None),
        (true, Some(placement)) => match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == placement.shape.byte_size => Ok(Some(*register)),
            locations => Err(Diagnostic::error(format!(
                "Microsoft x64 import result has unsupported placement {locations:?}"
            ))),
        },
        _ => Err(Diagnostic::error(
            "Microsoft x64 import plan/result shape is internally inconsistent",
        )),
    }
}

fn win64_result_is_indirect(placement: &ValuePlacement) -> bool {
    matches!(
        placement.locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: None,
            ..
        }]
    )
}

fn win64_result_pre_call_width(plan: &CallPlan) -> usize {
    usize::from(plan.result.as_ref().is_some_and(win64_result_is_indirect)) * 17
}

fn win64_result_post_call_width(plan: &CallPlan) -> usize {
    match plan.result.as_ref() {
        Some(placement) if matches!(placement.shape.class, ValueClass::Float) => 19,
        Some(placement) if !win64_result_is_indirect(placement) => {
            17 + usize::from(placement.shape.byte_size == 2)
        }
        _ => 0,
    }
}

fn append_win64_indirect_result_address<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let [
        ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: None,
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(Diagnostic::error(
            "Microsoft x64 indirect result does not use the hidden RCX destination",
        ));
    };
    let Some((_, byte_offset, operand_byte_size, operand_alignment)) =
        win64_aggregate_operand(operand)
    else {
        return Err(Diagnostic::error(
            "Microsoft x64 indirect result did not lower to aggregate storage",
        ));
    };
    if operand_byte_size != usize::from(*byte_size) || operand_alignment != usize::from(*alignment)
    {
        return Err(Diagnostic::error(
            "Microsoft x64 indirect result storage disagrees with its normalized shape",
        ));
    }
    append_mov_r11_imm64(bytes, 0); // relocated to the result region base
    bytes.extend([0x49, 0x8d, 0x8b]); // lea rcx, [r11+disp32]
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

/// Relocation sites for a `encode_win64_import_call` sequence: one Absolute64
/// region-base site per staged argument (inside its `mov r11, imm64`), the
/// Relative32 `call rel32` after all marshalling, and (value-returning) the
/// result region base inside the store tail's `mov r11, imm64`.
/// Total byte width of `encode_win64_out_param_call`'s fixed sequence:
/// sub(4) + lea(5) + call(5) + load(5) + add(4) + mov r11,imm64(10) + store(7).
const WIN64_OUT_PARAM_CALL_WIDTH: usize = 40;

/// A 0-arg Win64 import whose RESULT arrives through an OUT-PARAM (std::time
/// rung 5: QueryPerformanceCounter/-Frequency and
/// GetSystemTimePreciseAsFileTime all write a u64 through their pointer
/// argument). Reserve 56 bytes — the 0-arg import reserve (40) + 16 so the
/// out slot at `[rsp+40]` sits ABOVE the callee-owned 32-byte shadow space
/// and rsp keeps the same 16-byte parity — pass the slot's address in RCX,
/// call, load the u64 back into RAX, release, then store through the
/// standard result tail. operands[0] = the result place.
fn encode_win64_out_param_call<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    plan_source: HostCallPlan<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    const RESERVE: usize = 56;
    const SLOT: u8 = 40;
    let Some((_, byte_offset, byte_count)) = operands
        .first()
        .and_then(InstructionOperandLike::runtime_scalar_integer)
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 out-param import call: the result storage place did not lower \
             to a runtime scalar operand",
        ));
    };
    let plan = normalized_win64_out_param_plan(operation_key.operation, plan_source)?;
    match win64_argument_location(&plan.parameters[0], 0)? {
        Win64ArgumentLocation::Register(MachineRegister::X86Rcx) => {}
        location => {
            return Err(Diagnostic::error(format!(
                "Win64 out-parameter encoder requires its pointer in rcx, got {location:?}"
            )));
        }
    }
    let native_result = normalized_win64_result_register(&plan, plan.result.is_some())?;
    if native_result.is_some_and(|register| register != MachineRegister::X86Rax) {
        return Err(Diagnostic::error(format!(
            "Win64 out-parameter encoder cannot ignore planned native result {native_result:?}"
        )));
    }
    let mut bytes = Vec::with_capacity(WIN64_OUT_PARAM_CALL_WIDTH);
    append_sub_rsp(&mut bytes, RESERVE);
    bytes.extend([0x48, 0x8d, 0x4c, 0x24, SLOT]); // lea rcx, [rsp+SLOT]
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 (relocated)
    bytes.extend([0x48, 0x8b, 0x44, 0x24, SLOT]); // mov rax, [rsp+SLOT]
    append_add_rsp(&mut bytes, RESERVE);
    append_mov_r11_imm64(&mut bytes, 0); // relocated to the result region base
    match byte_count {
        4 => bytes.extend([0x41, 0x89, 0x83]), // mov [r11+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x83]), // mov [r11+disp32], rax
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 out-param import call cannot store a {other}-byte result (expected 4 or 8)"
            )));
        }
    }
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(bytes.len(), WIN64_OUT_PARAM_CALL_WIDTH);
    Ok(bytes)
}

fn normalized_win64_out_param_plan(
    operation: HostOperation,
    plan_source: HostCallPlan<'_>,
) -> Result<CallPlan, Diagnostic> {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: match operation {
            HostOperation::MonotonicTicks | HostOperation::MonotonicTicksPerSecond => {
                Some(ValueShape::integer(4, 4))
            }
            HostOperation::WallClockRaw => None,
            operation => {
                return Err(Diagnostic::error(format!(
                    "unsupported Win64 out-parameter operation {operation:?}"
                )));
            }
        },
    };
    selected_win64_composite_plan(&signature, plan_source, "time out-parameter")
}

/// Relocation sites for `encode_win64_out_param_call`: the import-thunk call
/// rel32 at 10 (sub 4 + lea 5 + the call opcode) and the result region base
/// at 25 (14 + load 5 + add 4 + the mov r11,imm64 prefix).
fn win64_out_param_call_relocation_sites() -> Vec<X86_64RelocationSite> {
    vec![
        X86_64RelocationSite {
            operand_index: None,
            byte_offset: 10,
            byte_width: 4,
            kind: X86_64RelocationSiteKind::Relative32,
        },
        X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: 25,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        },
    ]
}

/// Total byte width of `encode_constant_result`'s fixed sequence:
/// mov rax,imm64(10) + mov r15,imm64(10) + store(7).
const CONSTANT_RESULT_WIDTH: usize = 27;

/// A host operation lowered to a per-target CONSTANT (std::time rung 5's
/// wall-clock calibration constants, `PlatformCallData::ConstantResult`): no
/// call at all — materialize the immediate in RAX and run the standard
/// result store tail. operands[0] = the result place, operands[1] = the
/// constant as an immediate operand.
pub fn encode_constant_result<T: InstructionOperandLike>(
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    let Some((_, byte_offset, byte_count)) = operands
        .first()
        .and_then(InstructionOperandLike::runtime_scalar_integer)
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 constant-result host call: the result storage place did not \
             lower to a runtime scalar operand",
        ));
    };
    let Some(value) = operands
        .get(1)
        .and_then(InstructionOperandLike::immediate_integer)
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 constant-result host call: the constant did not lower to an \
             immediate operand",
        ));
    };
    let mut bytes = Vec::with_capacity(CONSTANT_RESULT_WIDTH);
    bytes.extend([0x48, 0xb8]); // mov rax, imm64
    bytes.extend((value as u64).to_le_bytes());
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the result region base
    match byte_count {
        4 => bytes.extend([0x41, 0x89, 0x87]), // mov [r15+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x87]), // mov [r15+disp32], rax
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 constant-result host call cannot store a {other}-byte result (expected 4 \
                 or 8)"
            )));
        }
    }
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(bytes.len(), CONSTANT_RESULT_WIDTH);
    Ok(bytes)
}

/// Exact register footprint of the no-call constant-result sequence.
pub fn constant_host_result_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86R15])
}

/// Relocation sites for `encode_constant_result`: only the result region base
/// at 12 (the mov rax,imm64 + the mov r15,imm64 prefix). No call site.
fn constant_result_relocation_sites() -> Vec<X86_64RelocationSite> {
    vec![X86_64RelocationSite {
        operand_index: Some(0),
        byte_offset: 12,
        byte_width: 8,
        kind: X86_64RelocationSiteKind::Absolute64,
    }]
}

#[cfg(test)]
fn win64_import_call_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
) -> Vec<X86_64RelocationSite> {
    win64_import_call_relocation_sites_for_plan(
        operands,
        returns_value,
        dereferences_result,
        HostCallPlan::CompatibilityOracle,
    )
}

fn win64_import_call_relocation_sites_for_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
    plan_source: HostCallPlan<'_>,
) -> Vec<X86_64RelocationSite> {
    let arg_start = usize::from(returns_value);
    let arg_count = operands.len().saturating_sub(arg_start);
    let plan = match plan_source {
        HostCallPlan::Authoritative(plan) => {
            if validate_win64_encoder_plan(plan).is_err()
                || validate_win64_plan_operand_shapes(plan, operands, returns_value).is_err()
            {
                return Vec::new();
            }
            Some(plan.clone())
        }
        HostCallPlan::CompatibilityOracle => {
            normalized_win64_import_plan(operands, returns_value).ok()
        }
    };
    let reserve = plan
        .as_ref()
        .map(win64_import_reserve_for_plan)
        .unwrap_or_else(|| win64_import_reserve(arg_count));
    let mut sites = Vec::new();
    let mut cursor = rsp_adjust_width(reserve);
    if plan
        .as_ref()
        .and_then(|plan| plan.result.as_ref())
        .is_some_and(win64_result_is_indirect)
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        cursor += 17;
    }
    for index in 0..arg_count {
        if win64_import_arg_is_staged(operands.get(arg_start + index)) {
            sites.push(X86_64RelocationSite {
                operand_index: Some(arg_start + index),
                byte_offset: cursor + 2, // inside mov r11/argreg, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_import_arg_width(
            operands,
            arg_start,
            index,
            plan.as_ref().and_then(|plan| plan.parameters.get(index)),
        );
    }
    sites.push(X86_64RelocationSite {
        operand_index: None,
        byte_offset: cursor + 1, // past the call opcode
        byte_width: 4,
        kind: X86_64RelocationSiteKind::Relative32,
    });
    cursor += 5 + rsp_adjust_width(reserve);
    if dereferences_result {
        cursor += 2; // mov eax, [rax]
    }
    if returns_value
        && plan
            .as_ref()
            .and_then(|plan| plan.result.as_ref())
            .is_some_and(|placement| !win64_result_is_indirect(placement))
        && operands.first().is_some_and(|operand| {
            operand.runtime_scalar_integer().is_some()
                || operand.runtime_scalar_float().is_some()
                || win64_aggregate_operand(operand).is_some()
        })
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2, // inside the result mov r11, imm64
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
/// (legacy void shape), no import thunk, no call relocation (the target is a
/// runtime pointer). The receiver (arg 0) must already sit in RCX -- so it is
/// a plain register arg like any other; the `mov rax, [rcx..]` reads it back.
#[cfg(test)]
fn encode_win64_vtable_call<T: InstructionOperandLike>(
    operands: &[T],
    index: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = normalized_win64_call_plan(operands, None, 0)?;
    encode_win64_vtable_call_with_plan(operands, index, &plan)
}

pub fn encode_win64_vtable_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    index: i64,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let byte_offset = index
        .checked_mul(8)
        .ok_or_else(|| Diagnostic::error("vtable slot index overflows a byte offset"))?;
    encode_win64_vtable_call_at_offset_with_plan(operands, byte_offset, false, authoritative_plan)
}

#[cfg(test)]
fn encode_win64_vtable_call_at_offset<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let arg_start = usize::from(result_present);
    let plan = normalized_win64_call_plan(operands, result_present.then_some(0), arg_start)?;
    encode_win64_vtable_call_at_offset_with_plan(operands, byte_offset, result_present, &plan)
}

/// The result store tail shared by the field-model call encoders (the same
/// shape as the import call's): `mov r11, imm64` relocated to the result
/// region base, then store rax/eax or xmm0 at the result's declared width.
fn append_win64_result_store<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    label: &str,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let result_register = match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == placement.shape.byte_size => *register,
        locations => {
            return Err(Diagnostic::error(format!(
                "X86_64 {label} has unsupported direct result placement {locations:?}"
            )));
        }
    };
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() {
        if result_register != MachineRegister::X86Xmm(0)
            || !matches!(placement.shape.class, ValueClass::Float)
            || usize::from(placement.shape.byte_size) != byte_count
        {
            return Err(Diagnostic::error(format!(
                "X86_64 {label} float result disagrees with its normalized XMM0 placement"
            )));
        }
        append_mov_r11_imm64(bytes, 0); // relocated to the result region base
        return append_x86_store_float_to_r11(bytes, result_register, byte_offset, byte_count);
    }
    if result_register != MachineRegister::X86Rax {
        return Err(Diagnostic::error(format!(
            "X86_64 {label} result store cannot realize planned register {result_register:?}"
        )));
    }
    let result_storage = operand
        .runtime_scalar_integer()
        .map(|(region, offset, size)| (region, offset, size))
        .or_else(|| {
            win64_aggregate_operand(operand).map(|(region, offset, size, _)| (region, offset, size))
        });
    let Some((_, byte_offset, byte_count)) = result_storage else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 {label}: the result storage place did not lower to a \
             runtime scalar or aggregate operand"
        )));
    };
    if usize::from(placement.shape.byte_size) != byte_count {
        return Err(Diagnostic::error(format!(
            "X86_64 {label} result storage disagrees with its normalized shape"
        )));
    }
    append_mov_r11_imm64(bytes, 0); // relocated to the result region base
    match byte_count {
        1 => bytes.extend([0x41, 0x88, 0x83]), // mov [r11+disp32], al
        2 => bytes.extend([0x66, 0x41, 0x89, 0x83]), // mov [r11+disp32], ax
        4 => bytes.extend([0x41, 0x89, 0x83]), // mov [r11+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x83]), // mov [r11+disp32], rax
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 {label} cannot store a direct {other}-byte result (expected 1, 2, 4, or 8)"
            )));
        }
    }
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_win64_vtable_dispatch_load(
    bytes: &mut Vec<u8>,
    receiver: &ValuePlacement,
    byte_offset: i64,
) -> Result<(), Diagnostic> {
    let register = match win64_argument_location(receiver, 0)? {
        Win64ArgumentLocation::Register(register) => register,
        Win64ArgumentLocation::Stack(_) => {
            return Err(Diagnostic::error(
                "Microsoft x64 vtable receiver unexpectedly lowered to the stack",
            ));
        }
    };
    let slot_disp = i32::try_from(byte_offset)
        .map_err(|_| Diagnostic::error("vtable field offset exceeds an imm32"))?;
    match register {
        MachineRegister::X86Rcx => bytes.extend([0x48, 0x8b, 0x81]),
        MachineRegister::X86Rdx => bytes.extend([0x48, 0x8b, 0x82]),
        MachineRegister::X86R8 => bytes.extend([0x49, 0x8b, 0x80]),
        MachineRegister::X86R9 => bytes.extend([0x49, 0x8b, 0x81]),
        other => {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 vtable receiver uses unsupported register {other:?}"
            )));
        }
    }
    bytes.extend(slot_disp.to_le_bytes());
    Ok(())
}

/// The FIELD-MODEL flavor (extern brief SS12.1): the fn-ptr offset comes from
/// the vtable struct's layout, already in bytes -- `mov rax, [rcx + offset];
/// call rax`. The slot flavor above is offset = index * 8. This is the
/// This-call shape: the receiver IS the first wire argument (COM/UEFI
/// protocols). When `result_present`, `operands[0]` is the RESULT place
/// (`let status = ...` prepends one); the receiver and declared arguments
/// follow, and the callee's return value stores through the import-call tail.
pub fn encode_win64_vtable_call_at_offset_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let arg_start = usize::from(result_present);
    if operands.len() <= arg_start {
        return Err(Diagnostic::error(
            "cannot encode X86_64 vtable call: the receiver (arg 0) did not lower to an operand",
        ));
    }
    validate_win64_encoder_plan(authoritative_plan)?;
    validate_win64_call_plan_operand_shapes(
        authoritative_plan,
        operands,
        result_present.then_some(0),
        arg_start,
    )?;
    let plan = authoritative_plan.clone();
    let indirect_result = plan.result.as_ref().is_some_and(win64_result_is_indirect);
    if !indirect_result {
        normalized_win64_result_register(&plan, result_present)?;
    }
    let reserve = win64_import_reserve_for_plan(&plan);
    let mut bytes = Vec::with_capacity(win64_vtable_call_width_with_plan(
        operands,
        byte_offset,
        result_present,
        &plan,
    ));
    append_sub_rsp(&mut bytes, reserve);
    if indirect_result {
        append_win64_indirect_result_address(
            &mut bytes,
            &operands[0],
            plan.result.as_ref().expect("indirect result placement"),
        )?;
    }
    append_win64_call_arguments(&mut bytes, operands, arg_start, Some(&plan.parameters))?;
    // A hidden indirect-result destination occupies RCX, shifting the
    // receiver to RDX. Read the dispatch pointer from its planned register.
    append_win64_vtable_dispatch_load(&mut bytes, &plan.parameters[0], byte_offset)?;
    append_call_register(&mut bytes, 0); // call rax
    append_add_rsp(&mut bytes, reserve);
    if result_present && !indirect_result {
        append_win64_result_store(
            &mut bytes,
            &operands[0],
            "vtable call",
            plan.result.as_ref().expect("direct result placement"),
        )?;
    }
    debug_assert_eq!(
        bytes.len(),
        win64_vtable_call_width_with_plan(operands, byte_offset, result_present, &plan)
    );
    Ok(bytes)
}

#[cfg(test)]
fn win64_vtable_call_width<T: InstructionOperandLike>(
    operands: &[T],
    _index: i64,
    result_present: bool,
) -> usize {
    let arg_start = usize::from(result_present);
    let Ok(plan) = normalized_win64_call_plan(operands, result_present.then_some(0), arg_start)
    else {
        return 0;
    };
    win64_vtable_call_width_with_plan(operands, _index, result_present, &plan)
}

pub fn win64_vtable_call_width_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    _index: i64,
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> usize {
    let arg_start = usize::from(result_present);
    let arg_count = operands.len() - arg_start;
    if validate_win64_encoder_plan(authoritative_plan).is_err()
        || validate_win64_call_plan_operand_shapes(
            authoritative_plan,
            operands,
            result_present.then_some(0),
            arg_start,
        )
        .is_err()
    {
        return 0;
    }
    let plan = authoritative_plan;
    let reserve = win64_import_reserve_for_plan(plan);
    let mut width = rsp_adjust_width(reserve);
    width += win64_result_pre_call_width(plan);
    for index in 0..arg_count {
        width += win64_import_arg_width(operands, arg_start, index, plan.parameters.get(index));
    }
    width += 7; // mov rax, [rcx + disp32]
    width += 2; // call rax (no REX.B for rax)
    width += rsp_adjust_width(reserve);
    width += win64_result_post_call_width(plan);
    width
}

/// A SERVICE-TABLE function call (UEFI BootServices/RuntimeServices): the
/// table pointer is DISPATCH-ONLY -- the declared arguments AFTER it marshal
/// into RCX/RDX/R8/R9/stack (EFI table services take no This), then the
/// callee loads from the table's fn-ptr field: `mov r11, imm64` (relocated
/// to the table's region base), `mov rax, [r11 + slot]`, `mov rax, [rax +
/// field_offset]`, `call rax`. Operand roles: `[result?][table][args...]`.
#[cfg(test)]
fn encode_win64_table_function_call<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let arg_start = usize::from(result_present) + 1;
    let plan = normalized_win64_call_plan(operands, result_present.then_some(0), arg_start)?;
    encode_win64_table_function_call_with_plan(operands, byte_offset, result_present, &plan)
}

pub fn encode_win64_table_function_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let table_index = usize::from(result_present);
    if operands.len() <= table_index {
        return Err(Diagnostic::error(
            "cannot encode X86_64 table-function call: the service table pointer did not \
             lower to an operand",
        ));
    }
    let Some((_, table_slot_offset, _)) = operands[table_index].runtime_scalar_integer() else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 table-function call: the service table pointer must lower \
             to a runtime scalar operand",
        ));
    };
    let arg_start = table_index + 1;
    validate_win64_encoder_plan(authoritative_plan)?;
    validate_win64_call_plan_operand_shapes(
        authoritative_plan,
        operands,
        result_present.then_some(0),
        arg_start,
    )?;
    let plan = authoritative_plan.clone();
    let indirect_result = plan.result.as_ref().is_some_and(win64_result_is_indirect);
    if !indirect_result {
        normalized_win64_result_register(&plan, result_present)?;
    }
    let reserve = win64_import_reserve_for_plan(&plan);
    let mut bytes = Vec::with_capacity(win64_table_function_call_width_with_plan(
        operands,
        byte_offset,
        result_present,
        &plan,
    ));
    append_sub_rsp(&mut bytes, reserve);
    if indirect_result {
        append_win64_indirect_result_address(
            &mut bytes,
            &operands[0],
            plan.result.as_ref().expect("indirect result placement"),
        )?;
    }
    append_win64_call_arguments(&mut bytes, operands, arg_start, Some(&plan.parameters))?;
    // Load the table pointer (dispatch-only, never a wire argument), read the
    // fn-ptr field, call it.
    append_mov_r11_imm64(&mut bytes, 0); // relocated to the table's region base
    bytes.extend([0x49, 0x8b, 0x83]); // mov rax, [r11 + disp32]
    bytes.extend(disp32(table_slot_offset)?.to_le_bytes());
    let field_disp = i32::try_from(byte_offset)
        .map_err(|_| Diagnostic::error("service table field offset exceeds an imm32"))?;
    bytes.extend([0x48, 0x8b, 0x80]); // mov rax, [rax + disp32]
    bytes.extend(field_disp.to_le_bytes());
    append_call_register(&mut bytes, 0); // call rax
    append_add_rsp(&mut bytes, reserve);
    if result_present && !indirect_result {
        append_win64_result_store(
            &mut bytes,
            &operands[0],
            "table-function call",
            plan.result.as_ref().expect("direct result placement"),
        )?;
    }
    debug_assert_eq!(
        bytes.len(),
        win64_table_function_call_width_with_plan(operands, byte_offset, result_present, &plan,)
    );
    Ok(bytes)
}

#[cfg(test)]
fn win64_table_function_call_width<T: InstructionOperandLike>(
    operands: &[T],
    _byte_offset: i64,
    result_present: bool,
) -> usize {
    let arg_start = usize::from(result_present) + 1;
    let Ok(plan) = normalized_win64_call_plan(operands, result_present.then_some(0), arg_start)
    else {
        return 0;
    };
    win64_table_function_call_width_with_plan(operands, _byte_offset, result_present, &plan)
}

pub fn win64_table_function_call_width_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    _byte_offset: i64,
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> usize {
    let arg_start = usize::from(result_present) + 1;
    let arg_count = operands.len().saturating_sub(arg_start);
    if validate_win64_encoder_plan(authoritative_plan).is_err()
        || validate_win64_call_plan_operand_shapes(
            authoritative_plan,
            operands,
            result_present.then_some(0),
            arg_start,
        )
        .is_err()
    {
        return 0;
    }
    let plan = authoritative_plan;
    let reserve = win64_import_reserve_for_plan(plan);
    let mut width = rsp_adjust_width(reserve);
    width += win64_result_pre_call_width(plan);
    for index in 0..arg_count {
        width += win64_import_arg_width(operands, arg_start, index, plan.parameters.get(index));
    }
    width += 10; // mov r11, imm64 (table region base)
    width += 7; // mov rax, [r11 + disp32]
    width += 7; // mov rax, [rax + disp32]
    width += 2; // call rax
    width += rsp_adjust_width(reserve);
    width += win64_result_post_call_width(plan);
    width
}

/// Relocation sites for a vtable call: the staged-argument region bases (no
/// call relocation -- the callee is a runtime pointer read from RCX) and,
/// when a result place leads the operands, the result region base inside the
/// store tail's `mov r11, imm64`.
#[cfg(test)]
fn win64_vtable_call_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
) -> Vec<X86_64RelocationSite> {
    let arg_start = usize::from(result_present);
    let Ok(plan) = normalized_win64_call_plan(operands, result_present.then_some(0), arg_start)
    else {
        return Vec::new();
    };
    win64_vtable_call_relocation_sites_with_plan(operands, result_present, &plan)
}

pub fn win64_vtable_call_relocation_sites_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> Vec<X86_64RelocationSite> {
    let arg_start = usize::from(result_present);
    let arg_count = operands.len() - arg_start;
    if validate_win64_encoder_plan(authoritative_plan).is_err()
        || validate_win64_call_plan_operand_shapes(
            authoritative_plan,
            operands,
            result_present.then_some(0),
            arg_start,
        )
        .is_err()
    {
        return Vec::new();
    }
    let plan = authoritative_plan;
    let reserve = win64_import_reserve_for_plan(plan);
    let mut sites = Vec::new();
    let mut cursor = rsp_adjust_width(reserve);
    if plan.result.as_ref().is_some_and(win64_result_is_indirect) {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        cursor += 17;
    }
    for index in 0..arg_count {
        if win64_import_arg_is_staged(operands.get(arg_start + index)) {
            sites.push(X86_64RelocationSite {
                operand_index: Some(arg_start + index),
                byte_offset: cursor + 2, // inside mov r11, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_import_arg_width(operands, arg_start, index, plan.parameters.get(index));
    }
    cursor += 7 + 2 + rsp_adjust_width(reserve); // fn-ptr read + call rax + add rsp
    if result_present
        && plan
            .result
            .as_ref()
            .is_some_and(|placement| !win64_result_is_indirect(placement))
        && operands.first().is_some_and(|operand| {
            operand.runtime_scalar_integer().is_some()
                || operand.runtime_scalar_float().is_some()
                || win64_aggregate_operand(operand).is_some()
        })
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2, // inside the result mov r11, imm64
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
    }
    sites
}

/// Relocation sites for a table-function call: the staged-argument region
/// bases, the TABLE pointer's region base (inside its dispatch load -- always
/// staged), and (result-present) the result region base in the store tail.
#[cfg(test)]
fn win64_table_function_call_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
) -> Vec<X86_64RelocationSite> {
    let arg_start = usize::from(result_present) + 1;
    let Ok(plan) = normalized_win64_call_plan(operands, result_present.then_some(0), arg_start)
    else {
        return Vec::new();
    };
    win64_table_function_call_relocation_sites_with_plan(operands, result_present, &plan)
}

pub fn win64_table_function_call_relocation_sites_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> Vec<X86_64RelocationSite> {
    let table_index = usize::from(result_present);
    let arg_start = table_index + 1;
    let arg_count = operands.len().saturating_sub(arg_start);
    if validate_win64_encoder_plan(authoritative_plan).is_err()
        || validate_win64_call_plan_operand_shapes(
            authoritative_plan,
            operands,
            result_present.then_some(0),
            arg_start,
        )
        .is_err()
    {
        return Vec::new();
    }
    let plan = authoritative_plan;
    let reserve = win64_import_reserve_for_plan(plan);
    let mut sites = Vec::new();
    let mut cursor = rsp_adjust_width(reserve);
    if plan.result.as_ref().is_some_and(win64_result_is_indirect) {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        cursor += 17;
    }
    for index in 0..arg_count {
        if win64_import_arg_is_staged(operands.get(arg_start + index)) {
            sites.push(X86_64RelocationSite {
                operand_index: Some(arg_start + index),
                byte_offset: cursor + 2, // inside mov r11, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_import_arg_width(operands, arg_start, index, plan.parameters.get(index));
    }
    if win64_import_arg_is_staged(operands.get(table_index)) {
        sites.push(X86_64RelocationSite {
            operand_index: Some(table_index),
            byte_offset: cursor + 2, // inside the table load's mov r11, imm64
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
    }
    cursor += 10 + 7; // table load: mov r11, imm64 + mov rax, [r11+disp32]
    cursor += 7 + 2 + rsp_adjust_width(reserve); // fn-ptr read + call rax + add rsp
    if result_present
        && plan
            .result
            .as_ref()
            .is_some_and(|placement| !win64_result_is_indirect(placement))
        && operands.first().is_some_and(|operand| {
            operand.runtime_scalar_integer().is_some()
                || operand.runtime_scalar_float().is_some()
                || win64_aggregate_operand(operand).is_some()
        })
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2, // inside the result mov r11, imm64
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
    }
    sites
}

#[cfg(test)]
fn host_call_relocation_sites<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    host_call_relocation_sites_for_policy(CallingPolicy::MicrosoftX64, operation_key, operands)
}

#[cfg(test)]
fn host_call_relocation_sites_for_policy<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    host_call_relocation_sites_for_plan(
        policy,
        operation_key,
        operands,
        HostCallPlan::CompatibilityOracle,
    )
}

fn host_call_relocation_sites_for_plan<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    plan_source: HostCallPlan<'_>,
) -> Vec<X86_64RelocationSite> {
    if policy == CallingPolicy::SystemVAMD64
        && matches!(
            operation_key.capability,
            HostCapability::Unknown | HostCapability::Custom(_)
        )
    {
        return sysv_import_layout_for_plan(operands, true, plan_source)
            .map(|layout| layout.relocation_sites)
            .unwrap_or_default();
    }
    if policy != CallingPolicy::MicrosoftX64 {
        return Vec::new();
    }
    match (operation_key.capability, operation_key.operation) {
        (
            HostCapability::Stdin | HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::GetStdHandle,
        ) => match validate_normalized_win64_get_std_handle_plan(plan_source) {
            Ok(()) => win64_import_call_relocation_sites_for_plan(
                operands,
                false,
                false,
                HostCallPlan::CompatibilityOracle,
            ),
            Err(_) => Vec::new(),
        },
        (HostCapability::Process, HostOperation::ExitProcess)
        | (HostCapability::Clock, HostOperation::Sleep) => {
            // Single-u32-arg kernel32 calls now share the plan-driven general
            // import marshaller and therefore its relocation walker.
            win64_import_call_relocation_sites_for_plan(operands, false, false, plan_source)
        }
        (HostCapability::Input, HostOperation::KeyState) => {
            if encode_key_state_call(operands, plan_source).is_err() {
                return Vec::new();
            }
            // Layout: sub(4) + vk marshalling (17 runtime / 5 const) + call(5)
            // + add(4) + movzx(3) + mov r11,imm64(10) + store(7).
            let vk_is_runtime = operands
                .get(1)
                .is_some_and(|operand| operand.runtime_scalar_integer().is_some());
            let vk_width = if vk_is_runtime { 17 } else { 5 };
            let mut sites = Vec::new();
            if vk_is_runtime {
                sites.push(X86_64RelocationSite {
                    operand_index: Some(1),
                    byte_offset: 4 + 2, // inside the vk mov r11, imm64
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
                byte_offset: 4 + vk_width + 5 + 4 + 3 + 2, // inside result mov r11, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
            sites
        }
        (HostCapability::Clock, HostOperation::TickCount) => {
            // 0-arg value-returning call through the general import-call layout
            // (call at 4+1; result-region base at 13+2 -- identical to the
            // original bespoke site list).
            win64_import_call_relocation_sites_for_plan(operands, true, false, plan_source)
        }
        (
            HostCapability::Clock,
            HostOperation::MonotonicTicks
            | HostOperation::MonotonicTicksPerSecond
            | HostOperation::WallClockRaw,
        ) => {
            if normalized_win64_out_param_plan(operation_key.operation, plan_source).is_err() {
                Vec::new()
            } else {
                win64_out_param_call_relocation_sites()
            }
        }
        (
            HostCapability::Clock,
            HostOperation::WallClockUnitsPerSecond | HostOperation::WallClockEpochOffsetSeconds,
        ) => constant_result_relocation_sites(),
        (HostCapability::Gui, _) => {
            // Value-returning general import calls (mirrors the encode arm).
            win64_import_call_relocation_sites_for_plan(operands, true, false, plan_source)
        }
        (HostCapability::Filesystem, _) => {
            // Value-returning general import calls; read_errno's deref shifts
            // the result-store site by 2 (mirrors the encode arm).
            win64_import_call_relocation_sites_for_plan(
                operands,
                true,
                operation_key.dereferences_result(),
                plan_source,
            )
        }
        (HostCapability::Unknown | HostCapability::Custom(_), _) => {
            // Provides-authored imports (mirrors the encode arm).
            win64_import_call_relocation_sites_for_plan(operands, true, false, plan_source)
        }
        (
            HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::Write | HostOperation::WriteFile,
        )
        | (HostCapability::Stdin, HostOperation::ReadFile) => {
            let mut sites = Vec::new();
            if normalized_win64_file_io_layout(plan_source).is_err() {
                return sites;
            }
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

// Console byte ops (std read_byte/write_byte) -- the x86_64 flavors.
// ZII-driven like the aarch64 pair: the ByteRead slot is pre-zeroed (tag 0 =
// Eof = the untouched state), the read lands straight in the payload word,
// and only a count > 0 writes tag 1. r14 holds the relocated region base
// (imm64 at +2, the line-read convention).
// ============================================================================

/// Windows import flavor: GetStdHandle(STD_INPUT_HANDLE) + ReadFile(handle,
/// &payload, 1, &bytes_read, NULL). Fixed width; the two rel32 call fixups
/// sit at [`runtime_byte_read_get_std_handle_offset`] and
/// [`runtime_byte_read_read_file_offset`].
pub fn encode_runtime_byte_read_import(
    target_offset: usize,
    payload_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    validate_normalized_win64_get_std_handle_plan(HostCallPlan::CompatibilityOracle)?;
    let file_layout = normalized_win64_file_io_layout(HostCallPlan::CompatibilityOracle)?;
    let tag_disp = disp32(target_offset)?;
    let payload_disp = disp32(target_offset + payload_offset)?;
    let mut bytes = Vec::with_capacity(runtime_byte_read_import_width());
    bytes.extend([0x49, 0xbe]); // mov r14, imm64 (region, relocated)
    bytes.extend(0u64.to_le_bytes());
    append_zero_dword_r14(&mut bytes, tag_disp); // tag = 0 (Eof)
    append_zero_dword_r14(&mut bytes, payload_disp); // payload = 0
    append_sub_rsp(&mut bytes, file_layout.reserve);
    bytes.push(0xb9); // mov ecx, STD_INPUT_HANDLE
    bytes.extend((-10i32).to_le_bytes());
    bytes.push(0xe8); // call GetStdHandle
    debug_assert_eq!(bytes.len(), runtime_byte_read_get_std_handle_offset());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x48, 0x89, 0xc1]); // mov rcx, rax (handle)
    bytes.extend([0x49, 0x8d, 0x96]); // lea rdx, [r14 + payload]
    bytes.extend(payload_disp.to_le_bytes());
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
    bytes.push(0xe8); // call ReadFile
    debug_assert_eq!(bytes.len(), runtime_byte_read_read_file_offset());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x8b, 0x44, 0x24, file_layout.transferred_disp]);
    bytes.extend([0x85, 0xc0]); // test eax, eax
    bytes.extend([0x74, 0x0b]); // je +11 (skip the tag store: Eof stays)
    append_one_dword_r14(&mut bytes, tag_disp); // tag = 1 (Byte)
    append_add_rsp(&mut bytes, file_layout.reserve);
    debug_assert_eq!(bytes.len(), runtime_byte_read_import_width());
    Ok(bytes)
}

/// Syscall flavor (linux_x64): read(0, &payload, 1) via the number table.
pub fn encode_runtime_byte_read_syscall(
    target_offset: usize,
    payload_offset: usize,
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
    let tag_disp = disp32(target_offset)?;
    let payload_disp = disp32(target_offset + payload_offset)?;
    let mut bytes = Vec::with_capacity(runtime_byte_read_syscall_width());
    bytes.extend([0x49, 0xbe]); // mov r14, imm64 (region, relocated)
    bytes.extend(0u64.to_le_bytes());
    append_zero_dword_r14(&mut bytes, tag_disp);
    append_zero_dword_r14(&mut bytes, payload_disp);
    bytes.extend([0x48, 0x31, 0xff]); // xor rdi, rdi (fd 0)
    bytes.extend([0x49, 0x8d, 0xb6]); // lea rsi, [r14 + payload]
    bytes.extend(payload_disp.to_le_bytes());
    bytes.extend([0xba, 0x01, 0x00, 0x00, 0x00]); // mov edx, 1
    bytes.push(0xb8); // mov eax, number
    bytes.extend(number.to_le_bytes());
    bytes.extend([0x0f, 0x05]); // syscall
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x7e, 0x0b]); // jle +11 (0 = EOF, negative = error: Eof stays)
    append_one_dword_r14(&mut bytes, tag_disp);
    debug_assert_eq!(bytes.len(), runtime_byte_read_syscall_width());
    Ok(bytes)
}

/// Windows import flavor: GetStdHandle(STD_OUTPUT_HANDLE) + WriteFile(handle,
/// &source, 1, &written, NULL).
pub fn encode_runtime_byte_write_import(source_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    validate_normalized_win64_get_std_handle_plan(HostCallPlan::CompatibilityOracle)?;
    let file_layout = normalized_win64_file_io_layout(HostCallPlan::CompatibilityOracle)?;
    let source_disp = disp32(source_offset)?;
    let mut bytes = Vec::with_capacity(runtime_byte_write_import_width());
    bytes.extend([0x49, 0xbe]); // mov r14, imm64 (source region/literal, relocated)
    bytes.extend(0u64.to_le_bytes());
    append_sub_rsp(&mut bytes, file_layout.reserve);
    bytes.push(0xb9); // mov ecx, STD_OUTPUT_HANDLE
    bytes.extend((-11i32).to_le_bytes());
    bytes.push(0xe8); // call GetStdHandle
    debug_assert_eq!(bytes.len(), runtime_byte_write_get_std_handle_offset());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x48, 0x89, 0xc1]); // mov rcx, rax
    bytes.extend([0x49, 0x8d, 0x96]); // lea rdx, [r14 + source]
    bytes.extend(source_disp.to_le_bytes());
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
    bytes.push(0xe8); // call WriteFile
    debug_assert_eq!(bytes.len(), runtime_byte_write_write_file_offset());
    bytes.extend([0, 0, 0, 0]);
    append_add_rsp(&mut bytes, file_layout.reserve);
    debug_assert_eq!(bytes.len(), runtime_byte_write_import_width());
    Ok(bytes)
}

/// Syscall flavor (linux_x64): write(1, &source, 1).
pub fn encode_runtime_byte_write_syscall(
    source_offset: usize,
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
    let source_disp = disp32(source_offset)?;
    let mut bytes = Vec::with_capacity(runtime_byte_write_syscall_width());
    bytes.extend([0x49, 0xbe]); // mov r14, imm64 (source, relocated)
    bytes.extend(0u64.to_le_bytes());
    bytes.extend([0xbf, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1 (stdout)
    bytes.extend([0x49, 0x8d, 0xb6]); // lea rsi, [r14 + source]
    bytes.extend(source_disp.to_le_bytes());
    bytes.extend([0xba, 0x01, 0x00, 0x00, 0x00]); // mov edx, 1
    bytes.push(0xb8); // mov eax, number
    bytes.extend(number.to_le_bytes());
    bytes.extend([0x0f, 0x05]); // syscall
    debug_assert_eq!(bytes.len(), runtime_byte_write_syscall_width());
    Ok(bytes)
}

pub(super) fn validate_composite_linux_syscall_plan(
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<(), Diagnostic> {
    use omega_calling_conventions::MachineRegister::*;
    if parameter_registers != [X86Rdi, X86Rsi, X86Rdx]
        || result_register != X86Rax
        || number_register != X86Rax
        || supervisor_call != 0
    {
        return Err(Diagnostic::error(format!(
            "X86_64 composite runtime-text syscall encoder cannot realize normalized plan parameters={parameter_registers:?}, result={result_register:?}, number={number_register:?}, immediate={supervisor_call}"
        )));
    }
    Ok(())
}

/// `mov dword [r14 + disp32], 0` (11 bytes).
fn append_zero_dword_r14(bytes: &mut Vec<u8>, disp: i32) {
    bytes.extend([0x41, 0xc7, 0x86]);
    bytes.extend(disp.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
}

/// `mov dword [r14 + disp32], 1` (11 bytes).
fn append_one_dword_r14(bytes: &mut Vec<u8>, disp: i32) {
    bytes.extend([0x41, 0xc7, 0x86]);
    bytes.extend(disp.to_le_bytes());
    bytes.extend(1u32.to_le_bytes());
}

pub fn runtime_byte_read_import_width() -> usize {
    104
}
/// rel32 fixup position of the GetStdHandle call inside the import read.
pub fn runtime_byte_read_get_std_handle_offset() -> usize {
    42
}
/// rel32 fixup position of the ReadFile call inside the import read.
pub fn runtime_byte_read_read_file_offset() -> usize {
    77
}
pub fn runtime_byte_read_syscall_width() -> usize {
    70
}
pub fn runtime_byte_write_import_width() -> usize {
    63
}
pub fn runtime_byte_write_get_std_handle_offset() -> usize {
    20
}
pub fn runtime_byte_write_write_file_offset() -> usize {
    55
}
pub fn runtime_byte_write_syscall_width() -> usize {
    34
}
