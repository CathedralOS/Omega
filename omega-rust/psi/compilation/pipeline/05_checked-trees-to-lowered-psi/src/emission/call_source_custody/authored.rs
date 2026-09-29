//! Authored roots and formal-to-explicit argument mapping for retained calls.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::signature::StateParameter;
use checked_trees::statement::StatementNode;
use checked_trees::types::{PrimitiveType, TypeReferenceHandle};
use checked_trees::{CheckedUnitCallCoordinate, NominalMachineUseSite};
use symbols::SymbolHandle;

pub(crate) mod nested;

pub(crate) struct AuthoredCall {
    pub source_target: SymbolHandle,
    pub source_site: Option<NominalMachineUseSite>,
    pub scalar_arguments: Vec<(ExpressionHandle, PrimitiveType)>,
    /// Positions within `scalar_arguments` the callee strips as proof-only
    /// erased formals. Plan rows carry them on a separate lane.
    pub erased_scalar_positions: Vec<u32>,
    pub structural_arguments: Vec<(u32, ExpressionHandle)>,
    pub boundary: bool,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
}

impl AuthoredCall {
    /// The authored expression passed at formal `position`: the explicit
    /// argument, or for the receiver formal of a call spelled on a place
    /// (`self.element(2)`, `self.samples.element(2)`) that receiver
    /// expression. An explicit receiver argument already sits at its position.
    pub(crate) fn structural_argument(
        &self,
        checked: &CheckedTrees,
        position: u32,
        is_self: bool,
    ) -> Option<ExpressionHandle> {
        self.structural_arguments
            .iter()
            .find_map(|(formal, expression)| (*formal == position).then_some(*expression))
            .or_else(|| {
                let Some(NominalMachineUseSite::Expression(expression)) = self.source_site else {
                    return None;
                };
                match checked.expression_table.expression(expression) {
                    ExpressionNode::Call(call) if is_self && call.receiver.is_valid() => {
                        Some(call.receiver)
                    }
                    _ => None,
                }
            })
    }
}

pub(crate) fn locate(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Result<AuthoredCall, LoweringError> {
    let call = locate_source(checked, caller_state, coordinate)?;
    let (machine, _) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    if machine.symbol != caller_machine
        || call.target_machine != target_machine
        || call.target_state != target_state
    {
        return unsupported("call source custody disagrees with its authored caller or target");
    }
    Ok(call)
}

pub(crate) fn locate_source(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<AuthoredCall, LoweringError> {
    let program = &checked.typed;
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(coordinate.statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "call source custody has no authored statement",
        ))?;
    let direct_root = match statement {
        StatementNode::Call(_) => true,
        StatementNode::LocalData(local) => matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        ),
        // A call assigned whole to a place is the statement's root call, as a
        // local initializer's is; the target is an l-value, not an operand.
        StatementNode::Assignment(assignment) => matches!(
            program.expression_table.expression(assignment.value),
            ExpressionNode::Call(_)
        ),
        StatementNode::Expression(expression) => matches!(
            program.expression_table.expression(*expression),
            ExpressionNode::Call(_)
        ),
        StatementNode::Transition(transition) => {
            matches!(
                program.statement_table.transition_target(transition.target),
                checked_trees::statement::TransitionTargetNode::Named { .. }
            ) && transition.guard == checked_trees::statement::TransitionGuardNode::Always
        }
        _ => false,
    };
    let (source_target, arguments, source_site) = if coordinate.call_ordinal != 0 || !direct_root {
        let expression =
            nested::authored_postorder(checked, caller_state, coordinate.statement_index)?
                .into_iter()
                .find_map(|(ordinal, expression)| {
                    (ordinal == coordinate.call_ordinal).then_some(expression)
                })
                .ok_or(LoweringError::Unsupported(
                    "nested call source custody has no exact authored preorder coordinate",
                ))?;
        super::occurrences::validate(
            checked,
            machine.symbol,
            caller_state,
            coordinate,
            expression,
        )?;
        expression_call(checked, expression)?
    } else {
        match statement {
            StatementNode::Transition(_) if coordinate.call_ordinal == 0 => {
                let (target, arguments) =
                    named_terminal_arguments(checked, caller_state, coordinate.statement_index)?;
                (target, arguments, None)
            }
            StatementNode::Call(call) if coordinate.call_ordinal == 0 => {
                if program
                    .statement_table
                    .expression_handles(call.arguments)
                    .len()
                    != call.arguments.count() as usize
                {
                    return unsupported(
                        "call source custody has an invalid authored argument span",
                    );
                }
                let index = state
                    .statement_nodes
                    .start()
                    .arena_index()
                    .checked_add(coordinate.statement_index)
                    .ok_or(LoweringError::Unsupported(
                        "call source statement identity overflows",
                    ))?;
                (
                    call.target_symbol,
                    program.statement_table.expression_handles(call.arguments),
                    Some(NominalMachineUseSite::Statement(arena::Handle::from_parts(
                        index,
                        state.statement_nodes.start().generation(),
                    ))),
                )
            }
            StatementNode::LocalData(local) if coordinate.call_ordinal == 0 => {
                expression_call(checked, local.initial_value)?
            }
            StatementNode::Assignment(assignment) if coordinate.call_ordinal == 0 => {
                super::occurrences::validate(
                    checked,
                    machine.symbol,
                    caller_state,
                    coordinate,
                    assignment.value,
                )?;
                expression_call(checked, assignment.value)?
            }
            StatementNode::Expression(expression) if coordinate.call_ordinal == 0 => {
                if validation::unit_statement_call_is_supported(
                    program,
                    machine,
                    state,
                    *expression,
                ) {
                    super::occurrences::validate(
                        checked,
                        machine.symbol,
                        state.symbol,
                        coordinate,
                        *expression,
                    )?;
                } else if !state.return_type.is_valid()
                    || matches!(
                        program
                            .type_reference_table
                            .type_reference(state.return_type),
                        checked_trees::types::TypeReferenceNode::Unit
                    )
                {
                    return unsupported(
                        "Unit call source custody has no exact authored Unit statement",
                    );
                }
                expression_call(checked, *expression)?
            }
            _ => return unsupported("call source custody has no supported authored call root"),
        }
    };
    let target = target_signature(checked, machine.symbol, source_target)?;
    let parameters = target.parameters;
    let nonself_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let explicit_self = arguments.len() > nonself_count;
    if arguments.len()
        != if explicit_self {
            parameters.len()
        } else {
            nonself_count
        }
    {
        return unsupported("call source custody disagrees with its authored argument count");
    }
    let mut explicit = 0usize;
    let mut scalar_arguments = Vec::new();
    let mut erased_scalar_positions = Vec::new();
    let mut structural_arguments = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        if parameter.is_self && !explicit_self {
            continue;
        }
        let argument = arguments[explicit];
        explicit += 1;
        let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
            structural_arguments.push((
                u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("call structural source position exceeds u32")
                })?,
                argument,
            ));
            continue;
        };
        if parameter.is_self
            || parameter.is_const
            || (parameter.is_mutable
                && !crate::expression_preparation::source_custody::supported_mutable_parameter(
                    primitive,
                ))
            || !program.expression_table.expression_is_valid(argument)
        {
            return unsupported("call source custody has no supported owned primitive argument");
        }
        if parameter.relevance.is_erased() {
            erased_scalar_positions.push(u32::try_from(scalar_arguments.len()).map_err(|_| {
                LoweringError::Unsupported("call erased source position exceeds u32")
            })?);
        }
        scalar_arguments.push((argument, primitive));
    }
    Ok(AuthoredCall {
        source_target,
        source_site,
        scalar_arguments,
        erased_scalar_positions,
        structural_arguments,
        boundary: target.boundary,
        target_machine: target.machine,
        target_state: target.state,
    })
}

/// Reconstruct a named terminal call from source, never from the operation
/// claiming it. No expression handle exists for this syntax: the statement
/// coordinate owns the invocation, and the explicit arguments keep theirs.
pub(super) fn named_terminal_arguments(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: u32,
) -> Result<(SymbolHandle, &[ExpressionHandle]), LoweringError> {
    use checked_trees::statement::{TransitionExit, TransitionGuardNode, TransitionTargetNode};
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let Some(StatementNode::Transition(transition)) = statements.get(statement_index as usize)
    else {
        return unsupported("named call custody has no authored transition");
    };
    if statement_index as usize != statements.len().saturating_sub(1)
        || transition.guard != TransitionGuardNode::Always
        || transition.continuation.is_valid()
        || transition.exit != TransitionExit::Ordinary
        || !checked
            .statement_table
            .transition_target_is_valid(transition.target)
    {
        return unsupported("named call custody requires an unconditional terminal invocation");
    }
    let TransitionTargetNode::Named {
        path,
        arguments,
        static_machine_parameter,
        ..
    } = checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("named call custody has no named target");
    };
    if static_machine_parameter.is_valid() {
        return unsupported("named call custody has an unselected static target");
    }
    let source_target = checked
        .typed
        .machine_by_symbol(path.symbol)
        .and_then(|owner| checked.typed.machine_states(owner).first())
        .map_or(path.symbol, |entry| entry.symbol);
    let target = target_signature(checked, machine.symbol, source_target)?;
    if target.machine == machine.symbol && path.symbol != machine.symbol {
        return unsupported("internal state transfer is not a terminal machine invocation");
    }
    let explicit = checked.statement_table.expression_handles(*arguments);
    if explicit.len() != arguments.count() as usize {
        return unsupported("named call custody has an invalid authored argument span");
    }
    Ok((source_target, explicit))
}

fn expression_call(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<
    (
        SymbolHandle,
        &[ExpressionHandle],
        Option<NominalMachineUseSite>,
    ),
    LoweringError,
> {
    if !checked
        .typed
        .expression_table
        .expression_is_valid(expression)
    {
        return unsupported("call source custody has no live authored expression");
    }
    let ExpressionNode::Call(call) = checked.typed.expression_table.expression(expression) else {
        return unsupported("call source custody has no bare authored call");
    };
    if checked
        .expression_table
        .expression_handles(call.arguments)
        .len()
        != call.arguments.count() as usize
    {
        return unsupported("call source custody has an invalid authored argument span");
    }
    Ok((
        call.target_symbol,
        checked
            .typed
            .expression_table
            .expression_handles(call.arguments),
        Some(NominalMachineUseSite::Expression(expression)),
    ))
}

/// A borrowed view of one exact authored callable.
pub(crate) struct TargetSignature<'source> {
    pub parameters: &'source [StateParameter],
    pub return_type: TypeReferenceHandle,
    pub boundary: bool,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
}

pub(crate) fn target_signature(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    source_target: SymbolHandle,
) -> Result<TargetSignature<'_>, LoweringError> {
    let program = &checked.typed;
    if !source_target.is_valid() {
        return unsupported("call source custody has no live target identity");
    }
    // A bodyless external leaf's call source is its exact boundary
    // requirement; the leaf declares no body to supply one.
    let external_leaf =
        validation::exact_external_leaf_boundary_requirement(program, source_target);
    let mut states = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter_map(move |state| (state.symbol == source_target).then_some((machine, state)))
    });
    if external_leaf.is_none()
        && let Some((machine, state)) = states.next()
    {
        if states.next().is_some() || !machine.symbol.is_valid() {
            return unsupported("call source custody disagrees with its authored callee owner");
        }
        return Ok(TargetSignature {
            parameters: program.state_parameters(state),
            return_type: state.return_type,
            // A bodied `boundary machine` is a checked adapter: callers reach
            // its authored body as an ordinary callee; only the bodyless
            // declaration is a boundary edge.
            boundary: machine.supply_mode.is_boundary_declaration() && !machine.body_is_present,
            machine: machine.symbol,
            state: state.symbol,
        });
    }
    if let Some(operator) = program
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == source_target)
    {
        return Ok(TargetSignature {
            parameters: program.operator_parameters(operator),
            return_type: operator.return_type,
            boundary: true,
            machine: operator.symbol,
            state: operator.symbol,
        });
    }
    let requirement = if let Some((requirement, _)) = external_leaf {
        requirement
    } else if let Some((owner, signature)) = program.machine_parameter_signature(source_target) {
        if owner.symbol != caller_machine {
            return unsupported("call source custody uses another machine's callable parameter");
        }
        signature.symbol
    } else {
        source_target
    };
    let mut signatures = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| program.trait_machine_signatures(definition).iter())
        .filter(|signature| signature.symbol == requirement);
    let signature = signatures.next().ok_or(LoweringError::Unsupported(
        "call source custody has no authored boundary requirement",
    ))?;
    if signatures.next().is_some() || !signature.symbol.is_valid() {
        return unsupported("call source custody disagrees with its authored boundary requirement");
    }
    Ok(TargetSignature {
        parameters: program.state_signature_parameters(signature),
        return_type: signature.return_type,
        boundary: true,
        machine: signature.symbol,
        state: signature.symbol,
    })
}

#[cfg(test)]
mod named_terminal_tests {
    use super::*;

    #[test]
    fn named_terminal_call_retains_target_and_operand_coordinates() {
        let checked = crate::front_end::checked_program(
            r#"
            data Counter { value: i32; }
            machine first(left: i32, right: i32) -> i32 { left }
            machine Counter::finish(&mut self, value: i32) { self.value = value; }
            machine Counter::main(&mut self) {
                self.value = 4;
                transition { _ -> self.finish(first(1, 2)) }
            }
        "#,
        );
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Counter::main")
            .unwrap();
        let state = checked.typed.machine_states(machine)[0].symbol;
        let coordinate = CheckedUnitCallCoordinate {
            statement_index: 1,
            call_ordinal: 0,
        };
        let outer = locate_source(&checked, state, coordinate).expect("named outer call");
        let nested = locate_source(
            &checked,
            state,
            CheckedUnitCallCoordinate {
                call_ordinal: 1,
                ..coordinate
            },
        )
        .expect("operand keeps preorder one");
        assert_ne!(outer.target_state, nested.target_state);
        assert_eq!(outer.scalar_arguments.len(), 1);
        assert_eq!(nested.scalar_arguments.len(), 2);
        assert_eq!(
            nested.source_site,
            Some(NominalMachineUseSite::Expression(
                outer.scalar_arguments[0].0
            ))
        );
        for forged in [
            CheckedUnitCallCoordinate {
                statement_index: 0,
                ..coordinate
            },
            CheckedUnitCallCoordinate {
                call_ordinal: 2,
                ..coordinate
            },
        ] {
            assert!(locate_source(&checked, state, forged).is_err());
        }
    }

    #[test]
    fn internal_transfer_cannot_be_replayed_as_a_named_machine_call() {
        let checked = crate::front_end::checked_program(
            r#"
            machine main() {
                transition { _ -> done() }
                state done() {}
            }
        "#,
        );
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "main")
            .unwrap();
        let state = checked.typed.machine_states(machine)[0].symbol;
        assert!(
            locate_source(
                &checked,
                state,
                CheckedUnitCallCoordinate {
                    statement_index: 0,
                    call_ordinal: 0
                }
            )
            .is_err()
        );
    }
}
