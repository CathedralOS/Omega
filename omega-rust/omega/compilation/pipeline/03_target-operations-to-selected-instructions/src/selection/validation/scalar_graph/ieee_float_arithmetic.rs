//! Independent replay of one float form: rebuild the exact form, its operand
//! count and types, operand registers, result register, constraint row and
//! provenance from the legal instruction; never invoke the producer.
use super::{
    LegalizedScalarInstructionKind, SelectedInstructionKind, SelectedInstructionProvenance,
    VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use selected_instructions::IeeeFloatArithmeticForm;

pub(super) fn validate(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Replay<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or_else(SelectedInstructionError::custody)?;
    let LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
        operation: arithmetic,
        format,
        left,
        right,
        ..
    } = operation.kind
    else {
        return Err(SelectedInstructionError::custody());
    };
    let form = IeeeFloatArithmeticForm {
        operation: arithmetic,
        format,
    };
    let scalar = arithmetic.result_type(format);
    let operand = arithmetic.operand_type(format);
    let values = std::iter::once(left).chain(right).collect::<Vec<_>>();
    if result.scalar_type != scalar || values.len() != form.operand_count() {
        return Err(SelectedInstructionError::custody());
    }
    let mut registers = Vec::with_capacity(values.len() + 1);
    for value in &values {
        let (_, register, _, value_type) = state
            .resolve(*value)
            .ok_or_else(SelectedInstructionError::custody)?;
        if value_type != operand {
            return Err(SelectedInstructionError::custody());
        }
        registers.push(register);
    }
    let keys = &state.constraints.keys;
    let key = if form.operand_count() == 2 {
        keys.ieee_float_arithmetic
    } else {
        keys.ieee_float_unary
    }
    .ok_or_else(SelectedInstructionError::custody)?;
    let output = state.result_register(result.value, result.definition_site, scalar)?;
    registers.push(output);
    state.check_instruction(
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form },
        key,
        &registers,
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: values.into_iter().chain([result.value]).collect(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
