//! One selected compiler float call as one nearest-even IEEE form.
//! Operands (by semantic arity; IEEE payloads or integers) and the
//! result stay in general registers; the arity's constraint row gives
//! the target form its scratch floating registers.
use super::{
    Builder, LegalizedScalarInstructionKind, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use selected_instructions::IeeeFloatArithmeticForm;

pub(super) fn emit(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or_else(SelectedInstructionError::custody)?;
    let LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
        operation: arithmetic,
        format,
        arguments: values,
        ..
    } = &operation.kind
    else {
        return Err(SelectedInstructionError::custody());
    };
    let form = IeeeFloatArithmeticForm {
        operation: *arithmetic,
        format: *format,
    };
    let scalar = arithmetic.result_type(*format);
    let operand = arithmetic.operand_type(*format);
    if !(target_operations::NearestIeeeFloatArithmeticRealization {
        operation: *arithmetic,
        format: *format,
    })
    .supports_target(state.environment.target())
    {
        return Err(SelectedInstructionError::custody());
    }
    if result.scalar_type != scalar || values.len() != form.operand_count() {
        return Err(SelectedInstructionError::custody());
    }
    let mut registers = Vec::with_capacity(values.len() + 1);
    for value in values {
        let (_, register, _, value_type) = state
            .resolve(*value)
            .ok_or_else(SelectedInstructionError::custody)?;
        if value_type != operand {
            return Err(SelectedInstructionError::custody());
        }
        registers.push(register);
    }
    let keys = &state.constraints.keys;
    let key = keys
        .for_semantic(selected_instructions::MachineSemanticKind::NearestIeeeFloatArithmetic(form))
        .ok_or_else(SelectedInstructionError::custody)?;
    let output = state.register(result.value, result.definition_site, scalar)?;
    registers.push(output);
    state.emit(
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form },
        key,
        &registers,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: values.iter().copied().chain([result.value]).collect(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
