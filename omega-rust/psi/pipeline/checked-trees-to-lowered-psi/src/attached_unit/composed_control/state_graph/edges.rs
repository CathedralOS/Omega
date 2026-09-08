//! Exact authored successor operands and Boolean fallback pairing.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::{
    TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

pub(super) fn successors(
    state: &CheckedComposedUnitControlStatePlan,
) -> Vec<&CheckedStructuralControlSuccessorPlan> {
    match &state.terminator {
        CheckedComposedUnitControlTerminatorPlan::ReturnUnit => Vec::new(),
        CheckedComposedUnitControlTerminatorPlan::Jump { successor } => vec![successor],
        CheckedComposedUnitControlTerminatorPlan::Conditional {
            when_true,
            when_false,
            ..
        } => vec![when_true, when_false],
        CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. } => Vec::new(),
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
        || edge.scalar_arguments.len() != target.scalar_parameters.len()
    {
        return unsupported("Unit graph successor arity drifted");
    }
    let source_parameters = checked.state_parameters(source);
    let validate_argument =
        |argument_position: u32, source_position: u32| {
            let argument_position = argument_position as usize
                - target
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.is_self && parameter.position < argument_position)
                    .count();
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
        let source_index = match transfer.source {
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
    for (position, (target, transfer)) in target
        .scalar_parameters
        .iter()
        .zip(&edge.scalar_arguments)
        .enumerate()
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
