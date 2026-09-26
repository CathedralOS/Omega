//! Materialize typed Boolean values before later operations can replace condition state.
use super::{
    IntegerSign, LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use legalized_operations::LegalizedScalarComparison;

pub(super) fn validate(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Replay<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or_else(|| SelectedInstructionError::custody())?;
    if result.scalar_type != ScalarType::Boolean {
        return Err(SelectedInstructionError::custody());
    }
    let (comparison, comparison_key, operands, values, materialize) = match operation.kind {
        LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            left,
            right,
        } => {
            let (_, left_register, _, left_type) = state
                .resolve(left)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            let (_, right_register, _, right_type) = state
                .resolve(right)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            if left_type != operand_type || right_type != left_type {
                return Err(SelectedInstructionError::custody());
            }
            let signed = match operand_type {
                ScalarType::Boolean if predicate == LegalizedScalarComparison::Equal => false,
                ScalarType::Integer(integer)
                    if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                        && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
                {
                    integer.sign() == IntegerSign::Signed
                }
                _ => return Err(SelectedInstructionError::custody()),
            };
            let materialize = match (predicate, signed) {
                (LegalizedScalarComparison::Equal, _) => {
                    SelectedInstructionKind::MaterializeBooleanEqual
                }
                (LegalizedScalarComparison::LessThan, false) => {
                    SelectedInstructionKind::MaterializeBooleanU64LessThan
                }
                (LegalizedScalarComparison::LessThan, true) => {
                    SelectedInstructionKind::MaterializeBooleanI64LessThan
                }
                (LegalizedScalarComparison::LessOrEqual, false) => {
                    SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                }
                (LegalizedScalarComparison::LessOrEqual, true) => {
                    SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                }
            };
            (
                SelectedInstructionKind::CompareI64,
                state.constraints.keys.compare_i64,
                vec![left_register, right_register],
                vec![left, right, result.value],
                materialize,
            )
        }
        LegalizedScalarInstructionKind::BooleanNot { operand } => {
            let (_, input, _, carrier) = state
                .resolve(operand)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            if carrier != ScalarType::Boolean {
                return Err(SelectedInstructionError::custody());
            }
            (
                SelectedInstructionKind::CompareI64Zero,
                state.constraints.keys.compare_i64_zero,
                vec![input],
                vec![operand, result.value],
                SelectedInstructionKind::MaterializeBooleanEqual,
            )
        }
        _ => return Err(SelectedInstructionError::custody()),
    };
    // The source operation and logical fuel occur once, at the comparison.
    state.check_instruction(
        comparison,
        comparison_key,
        &operands,
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values,
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    let output =
        state.result_register(result.value, result.definition_site, ScalarType::Boolean)?;
    state.check_instruction(
        materialize,
        state.constraints.keys.materialize_boolean,
        &[output],
        &SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
