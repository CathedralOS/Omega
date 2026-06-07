use omega_calling_conventions::{HostOperation, HostOperationKey};
use omega_platform_interface::HostCallPlan;
use omega_target_operations::{TargetOperation, TargetOperationKind};

use crate::{host, remap};

pub(crate) fn translate_instruction(
    host_calls: &HostCallPlan,
    instruction: &omega_abstract_operations::AbstractOperation,
) -> TargetOperation {
    TargetOperation {
        kind: translate_instruction_kind(host_calls, instruction),
        source_key: instruction.source_key,
        source_statement: instruction.source_statement,
    }
}

fn translate_instruction_kind(
    host_calls: &HostCallPlan,
    instruction: &omega_abstract_operations::AbstractOperation,
) -> TargetOperationKind {
    match &instruction.kind {
        omega_abstract_operations::AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands,
        } => {
            let operation_key =
                host::resolve_operation_key(host_calls, instruction, *operation_ordinal);
            TargetOperationKind::HostOperation {
                operation_key,
                operands: remap::operand_span(*operands),
            }
        }
        omega_abstract_operations::AbstractOperationKind::PreparePlatformOutputHandle {
            capability,
            operands,
        } => TargetOperationKind::HostOperation {
            operation_key: HostOperationKey::new(*capability, HostOperation::GetStdHandle),
            operands: remap::operand_span(*operands),
        },
        omega_abstract_operations::AbstractOperationKind::WritePlatformNewline {
            capability,
            use_file_api,
            operands,
        } => TargetOperationKind::HostOperation {
            operation_key: HostOperationKey::new(
                *capability,
                if *use_file_api {
                    HostOperation::WriteFile
                } else {
                    HostOperation::Write
                },
            ),
            operands: remap::operand_span(*operands),
        },
        kind => TargetOperationKind::from(kind),
    }
}
