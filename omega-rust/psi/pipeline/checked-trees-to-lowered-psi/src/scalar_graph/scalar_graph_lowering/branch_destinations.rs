//! Lower checked branch outcomes through the existing selected-arm value path.
use super::super::{ClaimId, PermissionClaimIdentity};
use super::{
    CheckedScalarBranchDestination, CheckedScalarExpressionRole, CheckedTrees, LoweringError,
    PreparedScalarQualifications, QualifiedScalarType, StructuralTypeDeclaration, computations,
    lower_checked_crash_exit, scalar_carriers, storage, structural_values, unsupported,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_graph_lowering::call_lowering::lower_scalar_graph_successor;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};

pub(crate) fn validate_coordinates(
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
    qualifications: &PreparedScalarQualifications,
    machine: symbols::SymbolHandle,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
    states: &[checked_trees::CheckedScalarStateGraph],
    source_state: symbols::SymbolHandle,
    source_value_types: &[QualifiedScalarType],
    destination: &CheckedScalarBranchDestination,
    scalar_bindings: &storage::ScalarBindings,
    result_type: QualifiedScalarType,
    return_sink: Option<usize>,
    computations: &mut computations::Expansion<'_>,
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<
    (
        usize,
        Vec<LoweredDirectExpression>,
        Vec<LoweredDirectExpression>,
    ),
    LoweringError,
> {
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
                structural_parameters: Vec::new(),
                structural_effects: Vec::new(),
                parameter_types: source_value_types.to_vec(),
                erased_formal_types: Vec::new(),
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Crash(crash),
            });
            Ok((
                target,
                computations::parameters(source_value_types),
                Vec::new(),
            ))
        }
        CheckedScalarBranchDestination::Jump(successor) => lower_scalar_graph_successor(
            checked,
            qualifications,
            states,
            source_state,
            source_value_types,
            successor,
            scalar_bindings,
            computations,
            structural_types,
            next_place,
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
            let target = structural_values::exit_target(
                checked,
                source_state,
                scalar_bindings,
                &[result_type],
                &mut Vec::new(),
                target,
                computations,
                structural_types,
                next_place,
            )?;
            if let Some(entry) = computations.return_value(
                source_state,
                *statement_ordinal,
                role,
                scalar_bindings,
                source_value_types,
                result_type,
                target,
            )? {
                return Ok((
                    entry,
                    computations::parameters(source_value_types),
                    Vec::new(),
                ));
            }
            let expression =
                scalar_bindings.expression_at(checked, source_state, *statement_ordinal, role)?;
            if expression.value_type(source_value_types)? != result_type {
                return unsupported(
                    "checked scalar branch return type must match the machine result",
                );
            }
            validate_direct_parameter_types(&expression, &scalar_carriers(source_value_types))?;
            Ok((target, vec![expression], Vec::new()))
        }
    }
}
