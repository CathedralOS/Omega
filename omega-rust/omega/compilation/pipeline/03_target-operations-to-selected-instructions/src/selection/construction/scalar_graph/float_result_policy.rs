//! Select post-call result adaptation; Psi already established overflow
//! eligibility over exact original operands before the boundary operation.
use super::{Builder, LegalizedScalarInstructionKind, SelectedInstructionKind, VirtualRegisterId};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::operation_provenance;
use selected_instructions::{FloatResultPolicyForm, FloatResultPolicyKind, MachineSemanticKind};
use semantic_vocabulary::ScalarType;
use terminal_psi::FloatResultPolicy;
pub(super) fn emit(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or(SelectedInstructionError::custody())?;
    let LegalizedScalarInstructionKind::FloatResultPolicy { format, policy } = operation.kind
    else {
        return Err(SelectedInstructionError::custody());
    };
    let scalar_type = ScalarType::IeeeFloat(format);
    if result.scalar_type != scalar_type
        || !policy.accepts_operand_types(format, |value| {
            state.resolve(value).map(|(_, _, _, kind)| kind)
        })
    {
        return Err(SelectedInstructionError::custody());
    }
    let form = FloatResultPolicyForm {
        format,
        policy: match policy {
            FloatResultPolicy::Trapping { .. } => FloatResultPolicyKind::Trapping,
            FloatResultPolicy::Saturating { .. } => FloatResultPolicyKind::Saturating,
        },
    };
    let values = policy.operands();
    let mut registers = values
        .iter()
        .map(|value| {
            state
                .resolve(*value)
                .map(|(_, register, _, _)| register)
                .ok_or(SelectedInstructionError::custody())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let key = state
        .constraints
        .keys
        .for_semantic(MachineSemanticKind::FloatResultPolicy(form))
        .ok_or(SelectedInstructionError::custody())?;
    let output = state.register(result.value, result.definition_site, scalar_type)?;
    registers.push(output);
    state.emit(
        SelectedInstructionKind::FloatResultPolicy { form },
        key,
        &registers,
        operation_provenance(
            operation,
            values.into_iter().chain([result.value]).collect(),
        ),
    )?;
    Ok(output)
}
