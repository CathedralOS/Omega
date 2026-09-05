//! Lower checked branch outcomes through the existing selected-arm value path.

use super::*;

pub(super) fn validate_coordinates(
    guard: u32,
    when_true: &CheckedScalarBranchDestination,
    when_false: &CheckedScalarBranchDestination,
) -> Result<(), LoweringError> {
    let coordinate = |destination: &CheckedScalarBranchDestination| match destination {
        CheckedScalarBranchDestination::Jump(successor) => {
            (successor.statement_ordinal, successor.is_continuation)
        }
        CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } => (*statement_ordinal, *is_continuation),
    };
    let (false_ordinal, is_continuation) = coordinate(when_false);
    if coordinate(when_true) != (guard, false)
        || if is_continuation {
            false_ordinal != guard
        } else {
            Some(false_ordinal) != guard.checked_add(1)
        }
    {
        return unsupported("scalar branch value coordinates do not match the selected guard arms");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_destination(
    checked: &CheckedTrees,
    states: &[checked_trees::CheckedScalarStateGraph],
    source_state: symbols::SymbolHandle,
    source_value_types: &[ScalarType],
    destination: &CheckedScalarBranchDestination,
    scalar_bindings: &storage::ScalarBindings,
    result_type: ScalarType,
    return_sink: Option<usize>,
) -> Result<(usize, Vec<LoweredDirectExpression>), LoweringError> {
    match destination {
        CheckedScalarBranchDestination::Jump(successor) => lower_scalar_graph_successor(
            checked,
            states,
            source_state,
            source_value_types,
            successor,
            scalar_bindings,
        ),
        CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } => {
            let expression = scalar_bindings.expression_at(
                checked,
                source_state,
                *statement_ordinal,
                if *is_continuation {
                    CheckedScalarExpressionRole::ContinuationReturn
                } else {
                    CheckedScalarExpressionRole::Return
                },
            )?;
            if expression.scalar_type() != result_type {
                return unsupported(
                    "checked scalar branch return type must match the machine result",
                );
            }
            validate_direct_parameter_types(&expression, source_value_types)?;
            let target = return_sink.ok_or(LoweringError::Unsupported(
                "checked scalar branch return has no prepared return destination",
            ))?;
            Ok((target, vec![expression]))
        }
    }
}
