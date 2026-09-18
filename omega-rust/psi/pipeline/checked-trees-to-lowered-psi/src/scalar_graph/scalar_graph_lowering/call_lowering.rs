//! Lowering scalar calls, direct call bindings and graph successors.

use crate::emission::expression_validation::validate_direct_parameter_types;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::emission::operation_emission::calls::{LoweredDirectCallBinding, ScalarCallCrashScope};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::bindings as storage;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::expression_preparation::source_custody;
use crate::scalar_graph::scalar_computations as computations;
use crate::scalar_graph::scalar_graph_lowering::structural_values;
use crate::scalar_graph::{
    CheckedScalarExpressionRole, CheckedScalarSuccessor, CheckedTrees, LoweringError,
    QualifiedScalarType, StructuralAccess, StructuralArgument, StructuralPathSegment,
    StructuralTypeDeclaration, scalar_carriers, unsupported,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_checked_direct_call_binding(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    call_ordinal: u32,
    argument_count: u32,
    result_type: QualifiedScalarType,
    caller_value_types: &[QualifiedScalarType],
    scalar_bindings: &storage::ScalarBindings,
) -> Result<LoweredDirectCallBinding, LoweringError> {
    source_custody::direct_calls::validate(
        checked,
        caller_machine,
        caller_state,
        statement_ordinal,
        binding_ordinal,
        target_machine,
        target_state,
        call_ordinal,
        argument_count,
        result_type.scalar_type,
    )?;
    let arguments = (0..argument_count)
        .map(|argument_ordinal| {
            scalar_bindings.expression_at(
                checked,
                caller_state,
                statement_ordinal,
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal,
                    argument_ordinal,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    lower_scalar_call(
        checked,
        qualifications,
        caller_machine,
        caller_state,
        statement_ordinal,
        target_machine,
        target_state,
        call_ordinal,
        result_type,
        caller_value_types,
        arguments,
        Vec::new(),
        ScalarCallCrashScope::CallerValues,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_scalar_call(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    statement_ordinal: u32,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    call_ordinal: u32,
    result_type: QualifiedScalarType,
    caller_value_types: &[QualifiedScalarType],
    arguments: Vec<LoweredDirectExpression>,
    structural_arguments: Vec<StructuralArgument>,
    crash_scope: ScalarCallCrashScope,
) -> Result<LoweredDirectCallBinding, LoweringError> {
    let target = if structural_arguments.is_empty() {
        crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find(
            checked,
            target_machine,
        )?
    } else {
        crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
            checked,
            target_machine,
        )?
    };
    if target.structural_parameters().len() != structural_arguments.len()
        || !target.entry_claims().is_empty()
        || structural_arguments.iter().any(|argument| {
            !argument.path.is_empty()
                && (argument.access != StructuralAccess::SharedBorrow
                    || argument
                        .path
                        .iter()
                        .any(|segment| !matches!(segment, StructuralPathSegment::Field(_))))
        })
    {
        return unsupported("computed scalar call requires exact structural custody");
    }
    let (target_parameter_types, target_result_type) =
        qualifications.scalar_state_types(checked, target_state)?;
    if target.entry_state()? != target_state {
        return unsupported("direct scalar call must target the callee entry state");
    }
    if target_result_type != result_type {
        return unsupported("direct scalar call result type must match its local binding");
    }
    if arguments.len() != target_parameter_types.len() {
        return unsupported("direct scalar call argument count must match the callee signature");
    }
    for (expression, target_type) in arguments.iter().zip(&target_parameter_types) {
        if expression.value_type(caller_value_types)? != *target_type {
            return unsupported(
                "checked scalar call argument type must match its callee parameter",
            );
        }
        validate_direct_parameter_types(expression, &scalar_carriers(caller_value_types))?;
    }
    let checked_call = checked
        .facts
        .contract_plans
        .for_machine(caller_machine)
        .and_then(|plan| {
            plan.crash
                .checked_call_at(caller_state, statement_ordinal, call_ordinal)
        })
        .ok_or(LoweringError::Unsupported(
            "direct scalar call has no matching checked crash-refinement row",
        ))?;
    if checked_call.target_machine() != target_machine
        || checked_call.target_state() != target_state
    {
        return unsupported("checked scalar call target disagrees with crash refinement");
    }
    let target_contract = checked
        .facts
        .contract_plans
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked contract plan",
        ))?;
    if checked_call.target_contract_report_fingerprint() != target_contract.report_fingerprint
        || checked_call.target_contract_commitment() != target_contract.commitment
    {
        return unsupported("checked scalar call target contract identity disagrees");
    }
    // Computed arguments have already become values. Bind the pinned callee
    // routes to those values, as ordinary staged calls do, rather than trying
    // to turn their effectful source expressions into pure caller predicates.
    // The pinned ceiling is the callee's effective crash routes — authored
    // buckets for a published ceiling or the union of retained body evidence
    // for an inferred contract — in the callee's parameter namespace, exactly
    // the ceiling the emitted machine contract publishes and the verifier
    // substitutes back at this call.
    let target_routes = crate::unit::effective_crash_routes(checked, target_machine)?;
    let crash_continuations = match crash_scope {
        ScalarCallCrashScope::CallerValues => checked_call.surviving_buckets().to_vec(),
        ScalarCallCrashScope::Arguments => target_routes.clone(),
    };
    if crash_continuations.iter().any(|bucket| {
        bucket.alternative_guards().iter().any(|guard| {
            matches!(guard, checked_trees::CrashRouteGuard::Predicate(predicate)
                if predicate.scalar_expression().is_none())
        })
    }) {
        return unsupported("direct scalar call crash continuation lacks a checked scalar term");
    }
    Ok(LoweredDirectCallBinding {
        source_coordinate: SourceCallCoordinate {
            state: caller_state,
            statement_index: usize::try_from(statement_ordinal).map_err(|_| {
                LoweringError::Unsupported("scalar call statement ordinal exceeds usize")
            })?,
            call_ordinal: usize::try_from(call_ordinal)
                .map_err(|_| LoweringError::Unsupported("scalar call ordinal exceeds usize"))?,
        },
        target_machine,
        result_type,
        arguments,
        structural_arguments,
        // The selected body owns storage even when its public signature has
        // only scalars. Graph and ordered-body callers use the same decision.
        uses_structural_frame: target.requires_structural_frame(),
        crash_continuations,
        parameter_relative_crash_routes: target_routes,
    })
}

pub(crate) fn lower_scalar_graph_successor(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    states: &[checked_trees::CheckedScalarStateGraph],
    source_state: symbols::SymbolHandle,
    source_value_types: &[QualifiedScalarType],
    successor: &CheckedScalarSuccessor,
    scalar_bindings: &storage::ScalarBindings,
    computations: &mut computations::Expansion<'_>,
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<(usize, Vec<LoweredDirectExpression>), LoweringError> {
    source_custody::validate_successor(checked, source_state, successor)?;
    let target = states
        .iter()
        .position(|candidate| candidate.state == successor.target)
        .ok_or(LoweringError::Unsupported(
            "scalar graph successor must belong to the selected machine",
        ))?;
    let (target_parameter_types, _) =
        qualifications.scalar_state_types(checked, states[target].state)?;
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let scalar_arguments = plans
        .scalar_arguments
        .span(successor.scalar_arguments)
        .ok_or(LoweringError::Unsupported(
            "scalar successor argument span is stale",
        ))?;
    if scalar_arguments.len() != target_parameter_types.len() {
        return unsupported(
            "scalar graph successor bindings must match the target parameter count",
        );
    }
    let source = states
        .iter()
        .find(|state| state.state == source_state)
        .ok_or(LoweringError::Unsupported(
            "scalar successor lost its source state",
        ))?;
    let mut structural_arguments = plans
        .structural_transfers
        .span(successor.structural_transfers)
        .ok_or(LoweringError::Unsupported(
            "scalar successor transfer span is stale",
        ))?
        .iter()
        .map(|transfer| {
            let checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } =
                transfer.source
            else {
                return unsupported("scalar successor requires a whole owned parameter transfer");
            };
            let parameter = source.structural_parameters.get(index as usize).ok_or(
                LoweringError::Unsupported("scalar successor transfer parameter is absent"),
            )?;
            scalar_bindings.owned_argument(&checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: index,
                },
                path: Vec::new(),
                type_identity: parameter.type_identity.clone(),
                access: parameter.access,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let target = structural_values::exit_target(
        checked,
        source_state,
        scalar_bindings,
        &target_parameter_types,
        &mut structural_arguments,
        target,
        computations,
        structural_types,
        next_place,
    )?;
    if let Some(entry) = computations.successor(
        source_state,
        successor,
        scalar_bindings,
        source_value_types,
        target,
        &target_parameter_types,
        &structural_arguments,
    )? {
        return Ok((entry, computations::parameters(source_value_types)));
    }
    let arguments = scalar_arguments
        .iter()
        .map(|argument| argument.argument_ordinal)
        .zip(&target_parameter_types)
        .map(|(argument_ordinal, target_type)| {
            let expression = scalar_bindings.expression_at(
                checked,
                source_state,
                successor.statement_ordinal,
                if successor.is_continuation {
                    CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
                } else {
                    CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                },
            )?;
            validate_direct_parameter_types(&expression, &scalar_carriers(source_value_types))?;
            (expression.value_type(source_value_types)? == *target_type)
                .then_some(expression)
                .ok_or(LoweringError::Unsupported(
                    "checked scalar successor expression type must match its target",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, arguments))
}
