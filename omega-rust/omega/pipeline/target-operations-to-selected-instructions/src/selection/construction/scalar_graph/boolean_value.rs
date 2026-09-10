//! Materialize typed Boolean values before later operations can replace condition state.
use super::*;
use legalized_operations::LegalizedScalarComparison;

pub(super) fn emit(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let result = operation.result.ok_or_else(invalid)?;
    if result.scalar_type != ScalarType::Boolean {
        return Err(invalid());
    }
    let (comparison, comparison_key, operands, values, materialize) = match operation.kind {
        LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            left,
            right,
        } => {
            let (_, left_register, _, left_type) = state.resolve(left).ok_or_else(invalid)?;
            let (_, right_register, _, right_type) = state.resolve(right).ok_or_else(invalid)?;
            if left_type != operand_type || right_type != left_type {
                return Err(invalid());
            }
            let signed = match operand_type {
                ScalarType::Boolean if predicate == LegalizedScalarComparison::Equal => false,
                ScalarType::Integer(integer)
                    if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                        && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
                {
                    integer.sign() == IntegerSign::Signed
                }
                _ => return Err(invalid()),
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
            let (_, input, _, carrier) = state.resolve(operand).ok_or_else(invalid)?;
            if carrier != ScalarType::Boolean {
                return Err(invalid());
            }
            (
                SelectedInstructionKind::CompareI64Zero,
                state.constraints.keys.compare_i64_zero,
                vec![input],
                vec![operand, result.value],
                SelectedInstructionKind::MaterializeBooleanEqual,
            )
        }
        _ => return Err(invalid()),
    };
    // The source operation and logical fuel occur once, at the comparison.
    state.emit(
        comparison,
        comparison_key,
        &operands,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values,
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    let output = state.register(result.value, result.definition_site, ScalarType::Boolean)?;
    state.emit(
        materialize,
        state.constraints.keys.materialize_boolean,
        &[output],
        SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}

pub(super) fn emit_branch_comparison(
    function: usize,
    source: &LegalizedScalarFunction,
    block: &legalized_operations::LegalizedScalarBlock,
    operation_index: usize,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let operation = &block.instructions[operation_index];
    let result = operation.result.ok_or_else(invalid)?;
    let scalar_type = result.scalar_type;
    let LegalizedScalarInstructionKind::Compare {
        predicate,
        operand_type,
        left,
        right,
    } = &operation.kind
    else {
        return Err(invalid());
    };
    if !matches!(*operand_type, ScalarType::Integer(integer)
        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed && matches!(integer.bits(), 8 | 16 | 32 | 64))
        && !(*operand_type == ScalarType::Boolean
            && *predicate == legalized_operations::LegalizedScalarComparison::Equal)
    {
        return Err(invalid());
    }
    if !control::branch_suffix(source, block, operation_index) {
        return Err(invalid());
    }
    if let Some(zero) = zero_compare::folded_zero(source, block, operation_index) {
        let input = if *left == zero.result.ok_or_else(invalid)?.value {
            *right
        } else {
            *left
        };
        let (_, register, _, actual_type) = builder.resolve(input).ok_or_else(invalid)?;
        if actual_type != *operand_type || scalar_type != ScalarType::Boolean {
            return Err(invalid());
        }
        builder.emit(
            SelectedInstructionKind::CompareI64Zero,
            builder.constraints.keys.compare_i64_zero,
            &[register],
            SelectedInstructionProvenance {
                operations: vec![zero.operation, operation.operation],
                values: vec![input, zero.result.ok_or_else(invalid)?.value, result.value],
                fuel: zero.fuel.iter().chain(&operation.fuel).copied().collect(),
                ..Default::default()
            },
        )?;
        return Ok(());
    }
    let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
    let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
    if left_type != *operand_type || right_type != left_type || scalar_type != ScalarType::Boolean {
        return Err(invalid());
    }
    let operands = if matches!(
        predicate,
        legalized_operations::LegalizedScalarComparison::LessOrEqual
    ) {
        [right_register, left_register]
    } else {
        [left_register, right_register]
    };
    builder.emit(
        SelectedInstructionKind::CompareI64,
        builder.constraints.keys.compare_i64,
        &operands,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: vec![*left, *right, result.value],
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
