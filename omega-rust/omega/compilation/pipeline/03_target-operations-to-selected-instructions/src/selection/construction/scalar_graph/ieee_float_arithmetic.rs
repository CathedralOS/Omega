//! One selected compiler float call as one nearest-even IEEE form.
//! Operands (by semantic arity; IEEE payloads or integers) and the
//! result stay in general registers; the arity's constraint row gives
//! the target form its scratch floating registers.
use super::{Builder, LegalizedScalarInstructionKind, SelectedInstructionKind, VirtualRegisterId};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::operation_provenance;
use selected_instructions::IeeeFloatArithmeticForm;

pub(super) fn emit(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or(SelectedInstructionError::custody())?;
    let LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
        realization,
        arguments: values,
        ..
    } = &operation.kind
    else {
        return Err(SelectedInstructionError::custody());
    };
    let (arithmetic, format) = (&realization.operation, &realization.format);
    let form = IeeeFloatArithmeticForm {
        operation: *arithmetic,
        format: *format,
    };
    let scalar = arithmetic.result_type(*format);
    let operand = arithmetic.operand_type(*format);
    // The legalized row carries the admitted realization; an x86 FMA is
    // supported only with its AVX+FMA3 custody for this target.
    if !realization.supports_target(state.environment.target()) {
        return Err(SelectedInstructionError::custody());
    }
    if result.scalar_type != scalar || values.len() != form.operand_count() {
        return Err(SelectedInstructionError::custody());
    }
    let mut registers = Vec::with_capacity(values.len() + 1);
    for value in values {
        let (_, register, _, value_type) = state
            .resolve(*value)
            .ok_or(SelectedInstructionError::custody())?;
        if value_type != operand {
            return Err(SelectedInstructionError::custody());
        }
        registers.push(register);
    }
    let keys = &state.constraints.keys;
    let key = keys
        .for_semantic(selected_instructions::MachineSemanticKind::NearestIeeeFloatArithmetic(form))
        .ok_or(SelectedInstructionError::custody())?;
    let output = state.register(result.value, result.definition_site, scalar)?;
    registers.push(output);
    state.emit(
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form },
        key,
        &registers,
        operation_provenance(
            operation,
            values.iter().copied().chain([result.value]).collect(),
        ),
    )?;
    Ok(output)
}
