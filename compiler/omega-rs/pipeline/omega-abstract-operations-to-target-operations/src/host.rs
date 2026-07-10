use omega_abstract_operations::{AbstractOperation, AbstractOperationKind, AbstractOperationPlan};
use omega_calling_conventions::{HostAbiPlan, HostOperationKey};
use omega_platform_interface::HostCallPlan;
use omega_target_operations::{
    RuntimeTextReadSource, TargetHostBinding, TargetOperationCode, TargetOperationKind,
};

use crate::remap;

pub(crate) fn copy_runtime_text_host_bindings(
    host_abi: &HostAbiPlan,
    abstract_operations: &AbstractOperationPlan,
    code: &mut TargetOperationCode,
) {
    for (instruction_key, instruction) in abstract_operations.code.instructions.iter() {
        if !matches!(
            &instruction.kind,
            AbstractOperationKind::ReadRuntimeTextLine { .. }
                | AbstractOperationKind::ReadRuntimeByte { .. }
                | AbstractOperationKind::WriteRuntimeByte { .. }
        ) {
            continue;
        }

        let (TargetOperationKind::ReadRuntimeTextLine {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        }
        | TargetOperationKind::ReadRuntimeByte {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        }
        | TargetOperationKind::WriteRuntimeByte {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        }) = &code
            .instructions
            .get(remap::instruction_handle(instruction_key))
            .kind
        else {
            continue;
        };

        if target_host_binding(code, *operation_key).is_some() {
            continue;
        }

        if let Some((_, binding)) = host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == *operation_key)
        {
            code.host_bindings.insert(binding.clone());
        }
    }
}

fn target_host_binding(
    code: &TargetOperationCode,
    operation_key: HostOperationKey,
) -> Option<&TargetHostBinding> {
    code.host_bindings
        .iter()
        .find(|(_, binding)| binding.operation_key == operation_key)
        .map(|(_, binding)| binding)
}

pub(crate) fn resolve_operation_key(
    host_calls: &HostCallPlan,
    instruction: &AbstractOperation,
    operation_ordinal: u16,
) -> HostOperationKey {
    let Some((_, host_call)) = host_calls.calls.iter().find(|(_, host_call)| {
        host_call.source_key == instruction.source_key
            && host_call.statement_index == instruction.source_statement
    }) else {
        panic!(
            "missing host call for abstract host operation at {:?} statement {}",
            instruction.source_key, instruction.source_statement
        );
    };

    let Some(operations) = host_calls.operations.span(host_call.operations) else {
        panic!(
            "missing lowered host operations for abstract host operation at {:?} statement {}",
            instruction.source_key, instruction.source_statement
        );
    };

    let ordinal = usize::from(operation_ordinal);
    let Some(operation) = operations.get(ordinal) else {
        panic!(
            "host operation ordinal {} out of range at {:?} statement {}",
            operation_ordinal, instruction.source_key, instruction.source_statement
        );
    };

    operation.operation_key
}
