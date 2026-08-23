mod byte_io;
mod context;
mod host_operation;
mod internal_calls;
mod queries;
mod runtime_storage;
mod runtime_storage_addresses;
mod runtime_storage_compares;
mod runtime_storage_copies;
mod runtime_storage_strings;
mod runtime_storage_writes;
mod runtime_text;
mod runtime_text_append;
mod runtime_text_compare;
mod runtime_text_materialize;
mod runtime_text_read;
mod runtime_text_write;
mod runtime_values;
mod wire_decode;
mod wire_encode;

use crate::RelocationPlanningInput;
use context::InstructionRelocationContext;
use omega_calling_conventions::{
    HostAbiPlan, HostBindingMechanism, HostOperation, HostOperationKey,
};
use omega_object_file::{ObjectSymbolHandle, RelocationPlan};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::{
    RuntimeTextReadSource, SelectedInstruction, SelectedInstructionKind,
};
use psi_diagnostics::Diagnostic;

pub(super) fn collect_instruction_relocations(
    input: RelocationPlanningInput<'_>,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    selected_text_offset: usize,
    selected_text_width: usize,
    instruction: &SelectedInstruction,
    relocation_plan: &mut RelocationPlan,
) -> Result<(), Diagnostic> {
    validate_runtime_host_plans(input.target, input.host_abi, &instruction.kind)?;
    // Foreign-control trampolines live outside the architecture-specific call
    // program. Existing relocation walkers continue to describe that inner
    // program; rebasing its instruction origin keeps every call/data fixup in
    // lockstep without teaching each specialized layout about the envelope.
    let selected_text_offset = selected_text_offset
        + instruction
            .kind
            .host_operation_key()
            .and_then(|operation_key| crate::lookups::find_host_binding(input, operation_key))
            .filter(|binding| binding.mechanism.requires_float_control_restore())
            .map(|_| {
                omega_instruction_selection::foreign_float_control_prefix_width(
                    input.target.architecture,
                )
            })
            .unwrap_or(0);
    let mut context = InstructionRelocationContext {
        input,
        function_symbol_handle,
        selected_instruction_index,
        selected_text_offset,
        selected_text_width,
        relocation_plan,
    };

    if internal_calls::collect_internal_call_relocation(&mut context, &instruction.kind)? {
    } else if host_operation::collect_host_operation_relocations(&mut context, &instruction.kind)? {
    } else if runtime_storage::collect_runtime_storage_relocations(&mut context, &instruction.kind)
    {
    } else if wire_encode::collect_wire_encode_relocations(&mut context, &instruction.kind) {
    } else if wire_decode::collect_wire_decode_relocations(&mut context, &instruction.kind) {
    } else {
        runtime_text::collect_runtime_text_relocations(&mut context, &instruction.kind);
    }
    Ok(())
}

fn validate_runtime_host_plans(
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    instruction: &SelectedInstructionKind,
) -> Result<(), Diagnostic> {
    let operation_key = match instruction {
        SelectedInstructionKind::ReadRuntimeTextLine {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        }
        | SelectedInstructionKind::ReadRuntimeByte {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        }
        | SelectedInstructionKind::WriteRuntimeByte {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        } => *operation_key,
        _ => return Ok(()),
    };
    let binding = host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == operation_key).then_some(binding))
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "runtime host relocation has no binding for {}.{}",
                operation_key.capability_name(),
                operation_key.operation_name()
            ))
        })?;
    if target.architecture != Architecture::X86_64 || target.object_format != ObjectFormat::Coff {
        return Ok(());
    }
    if !matches!(binding.mechanism, HostBindingMechanism::Import { .. }) {
        return Ok(());
    }
    let get_std_handle_key =
        HostOperationKey::new(operation_key.capability, HostOperation::GetStdHandle);
    let get_std_handle = host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == get_std_handle_key).then_some(binding))
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "Win64 runtime adapter relocation for {}.{} has no GetStdHandle binding",
                operation_key.capability_name(),
                operation_key.operation_name()
            ))
        })?;
    if !matches!(
        get_std_handle.mechanism,
        HostBindingMechanism::Import { .. }
    ) {
        return Err(Diagnostic::error(format!(
            "Win64 runtime adapter relocation for {}.{} requires an imported GetStdHandle binding",
            operation_key.capability_name(),
            operation_key.operation_name()
        )));
    }
    let get_std_handle_plan = get_std_handle.call_plan();
    let file_io_plan = binding.call_plan();
    omega_isa_x86_64::validate_win64_runtime_file_adapter_plans(get_std_handle_plan, file_io_plan)
}

#[cfg(test)]
mod plan_tests {
    use super::*;
    use omega_calling_conventions::{HostCapability, build_host_abi_plan};
    use omega_target_operations::RuntimeStorageRegion;

    fn stdin_byte_read(operation: HostOperation) -> SelectedInstructionKind {
        SelectedInstructionKind::ReadRuntimeByte {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: 0,
            payload_offset: 8,
            source: RuntimeTextReadSource::HostOperation {
                operation_key: HostOperationKey::new(HostCapability::Stdin, operation),
            },
        }
    }

    #[test]
    fn win64_runtime_adapter_relocations_require_an_imported_handle_binding() {
        let mut host_abi = build_host_abi_plan(NativeTarget::windows_x64());
        let instruction = stdin_byte_read(HostOperation::ReadFile);
        validate_runtime_host_plans(NativeTarget::windows_x64(), &host_abi, &instruction)
            .expect("complete retained composite plans");

        let get_std_handle_key =
            HostOperationKey::new(HostCapability::Stdin, HostOperation::GetStdHandle);
        let handle = host_abi
            .bindings
            .iter()
            .find_map(|(handle, binding)| {
                (binding.operation_key == get_std_handle_key).then_some(handle)
            })
            .expect("stdin GetStdHandle binding");
        host_abi.bindings.get_mut(handle).mechanism = HostBindingMechanism::Syscall {
            name: "invalid_get_std_handle".into(),
            number: 0,
        };

        let error =
            validate_runtime_host_plans(NativeTarget::windows_x64(), &host_abi, &instruction)
                .expect_err("a non-import GetStdHandle binding must reject");
        assert!(
            error
                .message
                .contains("requires an imported GetStdHandle binding")
        );
    }

    #[test]
    fn non_windows_runtime_relocations_do_not_require_win64_subplans() {
        let host_abi = build_host_abi_plan(NativeTarget::linux_x64());
        validate_runtime_host_plans(
            NativeTarget::linux_x64(),
            &host_abi,
            &stdin_byte_read(HostOperation::Read),
        )
        .expect("Linux syscall relocation has no GetStdHandle subcall");
    }

    #[test]
    fn runtime_host_relocations_require_the_selected_binding_on_every_target() {
        let host_abi = build_host_abi_plan(NativeTarget::linux_x64());
        let instruction = SelectedInstructionKind::ReadRuntimeByte {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: 0,
            payload_offset: 8,
            source: RuntimeTextReadSource::HostOperation {
                operation_key: HostOperationKey::new(
                    HostCapability::Unknown,
                    HostOperation::Unknown,
                ),
            },
        };
        let error = validate_runtime_host_plans(NativeTarget::linux_x64(), &host_abi, &instruction)
            .expect_err("runtime host relocation cannot omit its selected binding");
        assert!(error.message.contains("has no binding"));
    }
}
