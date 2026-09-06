//! Rejoin scalar plans to authored destinations and declaration namespaces.

use super::*;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

pub(super) mod direct_calls;

pub(super) struct SourceRoot {
    pub machine: symbols::SymbolHandle,
    pub expression: ExpressionHandle,
    pub destination: symbols::SymbolHandle,
    pub primitive_type: PrimitiveType,
}

pub(crate) fn authored_state(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
) -> Result<
    (
        &checked_trees::machine::Machine,
        &checked_trees::state::State,
    ),
    LoweringError,
> {
    let program = &checked.typed;
    let mut states = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter_map(move |candidate| {
                (state.is_valid() && candidate.symbol == state).then_some((machine, candidate))
            })
    });
    let (machine, state) = states.next().ok_or(LoweringError::Unsupported(
        "scalar source custody has no authored state",
    ))?;
    if states.next().is_some() {
        return unsupported("scalar source custody has ambiguous state ownership");
    }
    Ok((machine, state))
}

pub(super) fn locate(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    role: CheckedScalarExpressionRole,
) -> Result<SourceRoot, LoweringError> {
    let program = &checked.typed;
    let (machine, state) = authored_state(checked, state)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let authored = statements
        .get(statement as usize)
        .ok_or(LoweringError::Unsupported(
            "scalar source custody has no authored statement",
        ))?;
    let preceding = &statements[..statement as usize];
    let immutable_count = preceding
        .iter()
        .filter(|statement| {
            matches!(statement, StatementNode::LocalData(local)
            if !local.is_mutable && local.initial_value.is_valid()
                && program.primitive_type_reference(local.type_reference).is_some())
        })
        .count();
    let absent = symbols::SymbolHandle::invalid();
    let selected = match (authored, role) {
        (
            StatementNode::LocalData(local),
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
        ) if !local.is_mutable
            && usize::try_from(binding_ordinal).ok() == Some(immutable_count) =>
        {
            program
                .primitive_type_reference(local.type_reference)
                .map(|primitive| (local.initial_value, local.symbol, primitive))
        }
        (StatementNode::LocalData(local), CheckedScalarExpressionRole::StorageInitializer)
            if local.is_mutable =>
        {
            program
                .primitive_type_reference(local.type_reference)
                .map(|primitive| (local.initial_value, local.symbol, primitive))
        }
        (StatementNode::Assignment(assignment), CheckedScalarExpressionRole::AssignmentValue) => {
            match program.expression_table.expression(assignment.target) {
                ExpressionNode::Name(path)
                    if path.symbol.is_valid()
                        && path.symbol == path.head_symbol
                        && program
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            == 1 =>
                {
                    preceding.iter().find_map(|statement| match statement {
                        StatementNode::LocalData(local)
                            if local.is_mutable && local.symbol == path.symbol =>
                        {
                            program
                                .primitive_type_reference(local.type_reference)
                                .map(|primitive| (assignment.value, local.symbol, primitive))
                        }
                        _ => None,
                    })
                }
                _ => None,
            }
        }
        (StatementNode::Expression(expression), CheckedScalarExpressionRole::Return) => program
            .primitive_type_reference(state.return_type)
            .map(|primitive| (*expression, absent, primitive)),
        (StatementNode::Transition(transition), CheckedScalarExpressionRole::Guard) => {
            match transition.guard {
                TransitionGuardNode::When(expression) => {
                    Some((expression, absent, PrimitiveType::Bool))
                }
                TransitionGuardNode::Always => None,
            }
        }
        (
            StatementNode::Transition(transition),
            CheckedScalarExpressionRole::Return
            | CheckedScalarExpressionRole::ContinuationReturn
            | CheckedScalarExpressionRole::TransitionArgument { .. }
            | CheckedScalarExpressionRole::TransitionContinuationArgument { .. },
        ) if transition.exit == TransitionExit::Ordinary => {
            let target = if matches!(
                role,
                CheckedScalarExpressionRole::ContinuationReturn
                    | CheckedScalarExpressionRole::TransitionContinuationArgument { .. }
            ) {
                transition.continuation
            } else {
                transition.target
            };
            if !program.statement_table.transition_target_is_valid(target) {
                return unsupported("scalar source custody has no live transition target");
            }
            match (program.statement_table.transition_target(target), role) {
                (
                    TransitionTargetNode::Value(expression),
                    CheckedScalarExpressionRole::Return
                    | CheckedScalarExpressionRole::ContinuationReturn,
                ) => program
                    .primitive_type_reference(state.return_type)
                    .map(|primitive| (*expression, absent, primitive)),
                (
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    },
                    CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                    | CheckedScalarExpressionRole::TransitionContinuationArgument {
                        argument_ordinal,
                    },
                ) => program
                    .machine_states(machine)
                    .iter()
                    .find(|target| target.symbol == path.symbol)
                    .and_then(|target| {
                        let parameters = program.state_parameters(target);
                        let parameter = parameters.get(argument_ordinal as usize)?;
                        if parameter.is_self {
                            return None;
                        }
                        let explicit = parameters[..argument_ordinal as usize]
                            .iter()
                            .filter(|parameter| !parameter.is_self)
                            .count();
                        let expression = *program
                            .statement_table
                            .expression_handles(*arguments)
                            .get(explicit)?;
                        Some((
                            expression,
                            parameter.symbol,
                            program.primitive_type_reference(parameter.type_reference)?,
                        ))
                    }),
                _ => None,
            }
        }
        (
            StatementNode::LocalData(_),
            CheckedScalarExpressionRole::CallArgument {
                binding_ordinal,
                argument_ordinal,
            },
        ) => {
            let call = direct_calls::locate(checked, state.symbol, statement, binding_ordinal)?;
            call.arguments
                .get(argument_ordinal as usize)
                .zip(call.parameters.get(argument_ordinal as usize))
                .and_then(|(argument, parameter)| {
                    Some((
                        *argument,
                        absent,
                        program.primitive_type_reference(parameter.type_reference)?,
                    ))
                })
        }
        _ => None,
    };
    let (expression, destination, primitive_type) = selected.ok_or(LoweringError::Unsupported(
        "scalar source custody disagrees with its authored destination role",
    ))?;
    if !program.expression_table.expression_is_valid(expression) {
        return unsupported("scalar source custody has no live authored expression");
    }
    Ok(SourceRoot {
        machine: machine.symbol,
        expression,
        destination,
        primitive_type,
    })
}

pub(super) fn validate_pure(
    checked: &CheckedTrees,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    scalar_type: ScalarType,
) -> Result<(), LoweringError> {
    let source = locate(
        checked,
        binding.state,
        binding.statement_ordinal,
        binding.role,
    )?;
    if source.expression != binding.expression
        || source.destination != binding.destination
        || terminal_scalar_type(source.primitive_type)? != scalar_type
    {
        return unsupported(
            "pure scalar plan disagrees with its authored expression or destination",
        );
    }
    validate_namespace(checked, binding)
}

pub(crate) fn validate_namespace(
    checked: &CheckedTrees,
    binding: &checked_trees::CheckedScalarExpressionBindings,
) -> Result<(), LoweringError> {
    let program = &checked.typed;
    let (_, state) = authored_state(checked, binding.state)?;
    let preceding = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..binding.statement_ordinal as usize)
        .ok_or(LoweringError::Unsupported(
            "scalar source custody has no authored statement prefix",
        ))?;
    let expected = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .map(|parameter| parameter.symbol)
        .chain(preceding.iter().filter_map(|statement| {
            match statement {
                StatementNode::LocalData(local)
                    if !local.is_mutable
                        && local.initial_value.is_valid()
                        && program
                            .primitive_type_reference(local.type_reference)
                            .is_some() =>
                {
                    Some(local.symbol)
                }
                _ => None,
            }
        }));
    let retained = checked
        .facts
        .values
        .scalar_expressions
        .binding_symbols
        .span(binding.symbols)
        .ok_or(LoweringError::Unsupported(
            "pure scalar plan has an invalid declaration namespace",
        ))?;
    if !expected.eq(retained.iter().copied()) {
        return unsupported("pure scalar plan disagrees with its authored declaration namespace");
    }
    Ok(())
}

/// A successor selects the authored target even when it carries no values.
pub(super) fn validate_successor(
    checked: &CheckedTrees,
    source_state: symbols::SymbolHandle,
    successor: &CheckedScalarSuccessor,
) -> Result<(), LoweringError> {
    let program = &checked.typed;
    let (machine, state) = authored_state(checked, source_state)?;
    let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(successor.statement_ordinal as usize)
    else {
        return unsupported("scalar successor has no authored transition");
    };
    let target = if successor.is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    if transition.exit != TransitionExit::Ordinary
        || !program.statement_table.transition_target_is_valid(target)
    {
        return unsupported("scalar successor has no live ordinary transition target");
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(target)
    else {
        return unsupported("scalar successor is not an authored state transfer");
    };
    if path.symbol != successor.target
        || !program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == successor.target)
        || program.statement_table.expression_handles(*arguments).len()
            != successor.argument_count as usize
    {
        return unsupported("scalar successor target disagrees with its authored state transfer");
    }
    Ok(())
}
