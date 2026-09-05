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
        CheckedScalarBranchDestination::Crash { statement_ordinal } => (*statement_ordinal, false),
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
    machine: symbols::SymbolHandle,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
    states: &[checked_trees::CheckedScalarStateGraph],
    source_state: symbols::SymbolHandle,
    source_value_types: &[ScalarType],
    destination: &CheckedScalarBranchDestination,
    scalar_bindings: &storage::ScalarBindings,
    result_type: ScalarType,
    return_sink: Option<usize>,
    computations: &mut computations::Expansion<'_>,
) -> Result<(usize, Vec<LoweredDirectExpression>), LoweringError> {
    match destination {
        CheckedScalarBranchDestination::Crash { statement_ordinal } => {
            let crash = lower_checked_crash_exit(
                checked,
                machine,
                source_state,
                *statement_ordinal,
                source_claims,
            )?;
            let target = computations.push(LoweredScalarBranchState {
                parameter_types: source_value_types.to_vec(),
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Crash(crash),
            });
            Ok((target, computations::parameters(source_value_types)))
        }
        CheckedScalarBranchDestination::Jump(successor) => lower_scalar_graph_successor(
            checked,
            states,
            source_state,
            source_value_types,
            successor,
            scalar_bindings,
            computations,
        ),
        CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } => {
            let role = if *is_continuation {
                CheckedScalarExpressionRole::ContinuationReturn
            } else {
                CheckedScalarExpressionRole::Return
            };
            let target = return_sink.ok_or(LoweringError::Unsupported(
                "checked scalar branch return has no prepared return destination",
            ))?;
            if let Some(entry) = computations.return_value(
                source_state,
                *statement_ordinal,
                role,
                scalar_bindings,
                source_value_types,
                result_type,
                target,
            )? {
                return Ok((entry, computations::parameters(source_value_types)));
            }
            let expression =
                scalar_bindings.expression_at(checked, source_state, *statement_ordinal, role)?;
            if expression.scalar_type() != result_type {
                return unsupported(
                    "checked scalar branch return type must match the machine result",
                );
            }
            validate_direct_parameter_types(&expression, source_value_types)?;
            Ok((target, vec![expression]))
        }
    }
}
