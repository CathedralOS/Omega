//! Constant host-result footprint derivation.

use omega_calling_conventions::{
    MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_state_footprint,
};

/// Derive the exact scratch footprint of per-target constant host results.
/// These rows materialize a value directly into runtime storage and never
/// cross a foreign-call boundary.
pub fn derive_boundary_compiler_body_constant_host_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::PlatformCallData;

    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        if !matches!(host_call.data, PlatformCallData::ConstantResult { .. }) {
            continue;
        }
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        if !operation.operation_key.lowers_to_constant_result() {
            continue;
        }
        let Some(omega_abstract_operations::InstructionOperand {
            kind:
                InstructionOperandKind::RuntimeScalarInteger {
                    byte_offset,
                    byte_count,
                    ..
                },
        }) = operands
            .span(*operand_span)
            .and_then(|operands| operands.first())
        else {
            continue;
        };
        let clobbers = match architecture {
            omega_target::Architecture::X86_64 => omega_isa_x86_64::constant_host_result_clobbers(),
            omega_target::Architecture::Aarch64 => {
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
            }
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}
