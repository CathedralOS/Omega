//! Select the post-call adapter without replaying its boundary operation.
//! The form keeps one original operation site, including a possible Trap;
//! all inputs remain the exact IEEE payloads completed before this adapter.
use super::{
    Builder, LegalizedScalarInstructionKind, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use selected_instructions::{FloatResultPolicyForm, FloatResultPolicyKind, MachineSemanticKind};
use semantic_vocabulary::ScalarType;
use terminal_psi::FloatResultPolicy;

pub(super) fn emit(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or_else(SelectedInstructionError::custody)?;
    let LegalizedScalarInstructionKind::FloatResultPolicy { format, policy } = operation.kind
    else {
        return Err(SelectedInstructionError::custody());
    };
    let form = FloatResultPolicyForm {
        format,
        policy: match policy {
            FloatResultPolicy::Trapping { .. } => FloatResultPolicyKind::Trapping,
            FloatResultPolicy::Saturating { rule, .. } => FloatResultPolicyKind::Saturating(rule),
        },
    };
    let scalar_type = ScalarType::IeeeFloat(format);
    if result.scalar_type != scalar_type {
        return Err(SelectedInstructionError::custody());
    }
    let values = policy.operands();
    let mut registers = Vec::with_capacity(values.len() + 1);
    for value in &values {
        let (_, register, _, value_type) = state
            .resolve(*value)
            .ok_or_else(SelectedInstructionError::custody)?;
        if value_type != scalar_type {
            return Err(SelectedInstructionError::custody());
        }
        registers.push(register);
    }
    let key = state
        .constraints
        .keys
        .for_semantic(MachineSemanticKind::FloatResultPolicy(form))
        .ok_or_else(SelectedInstructionError::custody)?;
    let output = state.register(result.value, result.definition_site, scalar_type)?;
    registers.push(output);
    state.emit(
        SelectedInstructionKind::FloatResultPolicy { form },
        key,
        &registers,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: values.into_iter().chain([result.value]).collect(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
