//! Exact authored successor operands and Boolean fallback pairing.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::{
    StatementNode, TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

pub(super) fn successors(
    state: &CheckedComposedUnitControlStatePlan,
) -> Vec<&CheckedStructuralControlSuccessorPlan> {
    match &state.terminator {
        CheckedComposedUnitControlTerminatorPlan::ReturnUnit
        | CheckedComposedUnitControlTerminatorPlan::Guarded { .. }
        | CheckedComposedUnitControlTerminatorPlan::ReturnCase { .. } => Vec::new(),
        CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. } => Vec::new(),
        CheckedComposedUnitControlTerminatorPlan::Jump { successor } => vec![successor],
        CheckedComposedUnitControlTerminatorPlan::Conditional {
            when_true,
            when_false,
            ..
        } => vec![when_true, when_false],
        CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } => {
            cases.iter().map(|case| &case.successor).collect()
        }
    }
}

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    transition: &TableTransition,
    edge: &CheckedStructuralControlSuccessorPlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    validate_bindings(checked, plan, source, state, transition, edge, ordinal, &[])?;
    validate_cleanup(checked, plan, source, state, edge)
}

/// Case dispatch validates local-result cleanup separately from parameter cleanup.
pub(super) fn validate_bindings(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    transition: &TableTransition,
    edge: &CheckedStructuralControlSuccessorPlan,
    ordinal: usize,
    payloads: &[checked_trees::CheckedClosedSumPayloadTransferPlan],
) -> Result<(), LoweringError> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("Unit graph edge has no named source target");
    };
    let authored_target = if path.symbol == plan.machine {
        plan.states[0].state
    } else {
        path.symbol
    };
    if edge.statement_ordinal as usize != ordinal
        || edge.target_state != authored_target
        || transition.exit != TransitionExit::Ordinary
        || transition.continuation.is_valid()
        || !edge.trivial_affine_discard_parameter_positions.is_empty()
    {
        return unsupported("Unit graph successor disagrees with source control");
    }
    let target = plan
        .states
        .iter()
        .find(|state| state.state == edge.target_state)
        .ok_or(LoweringError::Unsupported(
            "Unit graph successor target is missing",
        ))?;
    let arguments = checked.statement_table.expression_handles(*arguments);
    if arguments.len()
        != target
            .structural_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
            + target.scalar_parameters.len()
        || edge.transfers.len() != target.structural_parameters.len()
        || edge.scalar_arguments.len() + payloads.len() != target.scalar_parameters.len()
    {
        return unsupported("Unit graph successor arity drifted");
    }
    let source_parameters = checked.state_parameters(source);
    let explicit_argument_position = |position: u32| {
        position as usize
            - target
                .structural_parameters
                .iter()
                .filter(|parameter| parameter.is_self && parameter.position < position)
                .count()
    };
    let validate_argument =
        |argument_position: u32, source_position: u32| {
            let argument_position = explicit_argument_position(argument_position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph source argument missing",
                ))?;
            let source = source_parameters.get(source_position as usize).ok_or(
                LoweringError::Unsupported("Unit graph source parameter missing"),
            )?;
            match checked.expression_table.expression(*expression) {
                ExpressionNode::Name(name)
                    if name.symbol == source.symbol
                        && name.head_symbol == source.symbol
                        && checked
                            .expression_table
                            .name_path_members(name.members)
                            .len()
                            == 1 =>
                {
                    Ok(())
                }
                _ => unsupported("Unit graph successor is not the retained parameter binding"),
            }
        };
    for (position, (target, transfer)) in target
        .structural_parameters
        .iter()
        .zip(&edge.transfers)
        .enumerate()
    {
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
            binding_ordinal,
        } = transfer.source
        {
            let mut matching = state
                .operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        result,
                        discard_result_on_return: false,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                        result,
                        discard_result_on_return: false,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        result,
                        discard_result_on_return: false,
                        ..
                    } if result.binding_ordinal == binding_ordinal => Some(result),
                    _ => None,
                });
            let result = matching.next().ok_or(LoweringError::Unsupported(
                "Unit graph transferred result missing",
            ))?;
            if matching.next().is_some()
                || result.statement_index >= edge.statement_ordinal
                || transfer.target_parameter_index as usize != position
                || target.is_self
                || target.access != checked_trees::CheckedStructuralAccess::Owned
                || !target.qualifications.is_empty()
                || target.multiplicity != result.multiplicity
                || target.type_identity != result.type_identity
            {
                return unsupported("Unit graph transferred result custody drifted");
            }
            let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
                .statement_table
                .statements(source.statement_nodes)
                .get(result.statement_index as usize)
            else {
                return unsupported("Unit graph result source local missing");
            };
            let argument_position = explicit_argument_position(target.position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph result argument missing",
                ))?;
            if local.is_mutable
                || !matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(path) if path.symbol == local.symbol && path.head_symbol == local.symbol && checked.expression_table.name_path_members(path.members).len() == 1)
            {
                return unsupported("Unit graph result argument lost its actual local");
            }
            result_custody::validate(checked, plan.machine, source, local, result)?;
            continue;
        }
        let source_index = match transfer.source {
            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                ..
            } => return unsupported("Unit graph result was not independently rejoined"),
            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => index,
            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                parameter_index,
                ..
            } => parameter_index,
        };
        let source = state
            .structural_parameters
            .get(source_index as usize)
            .ok_or(LoweringError::Unsupported(
                "Unit graph borrowed transfer source missing",
            ))?;
        if transfer.target_parameter_index as usize != position
            || source.type_identity != target.type_identity
            || source.access != target.access
            || source.multiplicity != target.multiplicity
        {
            return unsupported("Unit graph borrowed transfer type or order drifted");
        }
        match transfer.source {
            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                ..
            } => return unsupported("Unit graph result was not independently rejoined"),
            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { .. } => {
                if target.is_self {
                    if source != target {
                        return unsupported("Unit graph persistent receiver transfer drifted");
                    }
                } else {
                    validate_argument(target.position, source.position)?;
                }
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                expression,
                ..
            } => {
                if arguments.get(target.position as usize) != Some(&expression) {
                    return unsupported("Unit graph subslice disagrees with its source argument");
                }
                subslices::validate(
                    checked,
                    state,
                    edge.statement_ordinal,
                    target.position,
                    source.position,
                    expression,
                )?;
            }
        }
    }
    for ((position, target), transfer) in target
        .scalar_parameters
        .iter()
        .enumerate()
        .filter(|(position, _)| {
            !payloads
                .iter()
                .any(|payload| payload.target_scalar_parameter_index as usize == *position)
        })
        .zip(&edge.scalar_arguments)
    {
        if transfer.target_scalar_parameter_index as usize != position
            || transfer.argument_ordinal != target.source_position
            || transfer.primitive_type != target.primitive_type
        {
            return unsupported("Unit graph scalar transfer type or order drifted");
        }
        match transfer.source {
            checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index } => {
                let source = state.scalar_parameters.get(index as usize).ok_or(
                    LoweringError::Unsupported("Unit graph scalar transfer source missing"),
                )?;
                if source.primitive_type != target.primitive_type {
                    return unsupported("Unit graph scalar transfer source type drifted");
                }
                validate_argument(target.source_position, source.source_position)?;
            }
            checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                let (binding, _) = checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(
                        state.state,
                        edge.statement_ordinal,
                        CheckedScalarExpressionRole::TransitionArgument {
                            argument_ordinal: transfer.argument_ordinal,
                        },
                    )
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph scalar successor has no checked source expression",
                    ))?;
                crate::scalar_source_custody::validate_pure(
                    checked,
                    binding,
                    terminal_scalar_type(target.primitive_type)?,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_cleanup(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &CheckedStructuralControlSuccessorPlan,
) -> Result<(), LoweringError> {
    for (_, event) in checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == plan.machine
                && event.state_symbol == state.state
                && event.source == language_semantics::PermissionEventSource::StateExit
                && event.kind == language_semantics::PermissionEventKind::AffineDrop
        })
    {
        if checked
            .state_parameters(source)
            .iter()
            .any(|parameter| event.root == facts::PlaceRoot::Symbol(parameter.symbol))
        {
            continue;
        }
        let matching = state.operations.iter().filter_map(result_custody::result).filter(|result| {
            matches!(checked.statement_table.statements(source.statement_nodes).get(result.statement_index as usize), Some(StatementNode::LocalData(local)) if event.root == facts::PlaceRoot::Symbol(local.symbol))
        }).count();
        if matching != 1 {
            return unsupported("Unit graph edge leaves an unaccounted local disposition");
        }
    }
    result_custody::local_discards(checked, plan.machine, source, state, Some(edge))?;
    let cleanup = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_edge(plan.machine, state.state, edge.statement_ordinal)
        .ok_or(LoweringError::Unsupported(
            "Unit graph edge cleanup evidence missing",
        ))?;
    if cleanup.target_state != edge.target_state
        || !cleanup
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return unsupported("Unit graph edge cleanup evidence disagrees");
    }
    Ok(())
}

pub(super) fn validate_fallback(
    checked: &CheckedTrees,
    when_true: &TableTransition,
    when_false: &TableTransition,
) -> Result<(), LoweringError> {
    if when_false.guard == TransitionGuardNode::Always {
        return Ok(());
    }
    let (TransitionGuardNode::When(first), TransitionGuardNode::When(second)) =
        (when_true.guard, when_false.guard)
    else {
        return unsupported("Unit graph has no exact Boolean fallback");
    };
    let subject = |expression, expected| {
        let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
            return None;
        };
        (binary.operator == checked_trees::expression::BinaryOperator::Equal
            && matches!(checked.expression_table.expression(binary.right), ExpressionNode::Boolean(value) if *value == expected))
            .then_some(binary.left)
    };
    let (Some(first), Some(second)) = (subject(first, true), subject(second, false)) else {
        return unsupported("Unit graph fallback is not the inverse source label");
    };
    if !checked
        .expression_table
        .expressions_structurally_equal(first, second)
    {
        return unsupported("Unit graph branch labels inspect different source values");
    }
    Ok(())
}

/// Reconstruct the remaining whole-parameter drops at an ordinary return.
/// Call consumption and return disposal are separate uses of the same source root.
pub(super) fn return_discards(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<Vec<usize>, LoweringError> {
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
        PermissionProvenance,
    };
    let parameters = checked.state_parameters(source);
    let local_discards = if matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ReturnUnit
    ) {
        result_custody::local_discards(checked, machine, source, state, None)?
    } else {
        Vec::new()
    };
    let mut drops = Vec::new();
    for (_, event) in checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state.state
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
        })
    {
        if state.operations.iter().filter_map(result_custody::result).any(|result| {
            local_discards.contains(&result.binding_ordinal)
                && matches!(checked.statement_table.statements(source.statement_nodes).get(result.statement_index as usize),
                    Some(StatementNode::LocalData(local)) if event.root == facts::PlaceRoot::Symbol(local.symbol))
        }) {
            continue;
        }
        let index = state
            .structural_parameters
            .iter()
            .position(|parameter| {
                parameters
                    .get(parameter.position as usize)
                    .is_some_and(|source| event.root == facts::PlaceRoot::Symbol(source.symbol))
            })
            .ok_or(LoweringError::Unsupported(
                "Unit return has an unaccounted local drop",
            ))?;
        let parameter = &state.structural_parameters[index];
        if parameter.access != checked_trees::CheckedStructuralAccess::Owned
            || parameter.multiplicity != Multiplicity::Affine
            || event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != PermissionProvenance::Unknown
            || event.obligation_live
            || !event.segments.is_empty()
            || drops.contains(&index)
        {
            return unsupported("Unit return parameter cleanup custody drifted");
        }
        drops.push(index);
    }
    if matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ReturnUnit
    ) {
        // The Unit graph's source plan retains the complete parameter exit
        // roster before call consumption below. Preserve this admission check;
        // deleting a source drop is not authority to silently omit cleanup.
        let expected = state
            .structural_parameters
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, parameter)| {
                parameter.access == checked_trees::CheckedStructuralAccess::Owned
                    && parameter.multiplicity == Multiplicity::Affine
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if drops != expected {
            return unsupported("Unit return parameter cleanup roster drifted");
        }
    }
    // Structural return events instead describe residual custody after the
    // authored value transfer. Returning a parameter or moving it into a
    // constructor can remove its exit drop. Source value replay checks those
    // transfers; Terminal independently requires the remaining live frontier,
    // including canonical reverse order, before publication.
    for operation in &state.operations {
        let arguments = match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                structural_arguments,
                ..
            } => structural_arguments.as_slice(),
            _ => &[],
        };
        for argument in arguments
            .iter()
            .filter(|argument| argument.access == checked_trees::CheckedStructuralAccess::Owned)
        {
            if let Some(index) = argument.source_parameter_index() {
                if !argument.path.is_empty() {
                    return unsupported("Unit return has a partial parameter move");
                }
                if let Some(position) = drops
                    .iter()
                    .position(|candidate| *candidate == index as usize)
                {
                    drops.remove(position);
                }
            }
        }
    }
    Ok(drops)
}
