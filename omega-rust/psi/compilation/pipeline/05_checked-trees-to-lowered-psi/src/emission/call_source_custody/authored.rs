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
    /// Original explicit source order, before partitioning scalar and structural lanes.
    pub explicit_arguments: Vec<ExpressionHandle>,
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
    let (machine, _) = crate::declarations::state_declaration(checked, caller_state)?;
    if machine.symbol != caller_machine
        || call.target_machine != target_machine
        || call.target_state != target_state
    {
        return unsupported("call source custody disagrees with its authored caller or target");
    }
    Ok(call)
}

/// Select the exact named transition occurrence at a call coordinate. Guard
/// and operand calls can name the same callee; their ordinals do not identify
/// the arm's invocation unless the authored walk retains its target handle.
pub(crate) fn transition_target_at_call(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<checked_trees::statement::TransitionTargetHandle, LoweringError> {
    let occurrences =
        nested::authored_occurrences(checked, caller_state, coordinate.statement_index)?;
    let mut selected = occurrences
        .into_iter()
        .filter(|(ordinal, _)| *ordinal == coordinate.call_ordinal);
    let (_, occurrence) = selected.next().ok_or(LoweringError::Unsupported(
        "terminal call has no exact authored occurrence",
    ))?;
    if selected.next().is_some() {
        return unsupported("terminal call has duplicate authored occurrence coordinates");
    }
    match occurrence {
        nested::AuthoredOccurrence::TransitionTarget(target) => Ok(target),
        nested::AuthoredOccurrence::StatementRoot => {
            // An unconditional named invocation owns the statement root in the
            // same occurrence walk. Recover its handle from that exact statement.
            let (_, state) = crate::declarations::state_declaration(checked, caller_state)?;
            match checked
                .statement_table
                .statements(state.statement_nodes)
                .get(coordinate.statement_index as usize)
            {
                Some(StatementNode::Transition(transition))
                    if transition.guard
                        == checked_trees::statement::TransitionGuardNode::Always =>
                {
                    Ok(transition.target)
                }
                _ => unsupported("terminal call occurrence names a non-transition root"),
            }
        }
        nested::AuthoredOccurrence::Expression(_) => {
            unsupported("terminal call occurrence names a guard or operand expression")
        }
    }
}

pub(crate) fn locate_source(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<AuthoredCall, LoweringError> {
    let program = &checked.typed;
    let (machine, state) = crate::declarations::state_declaration(checked, caller_state)?;
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
        // A named target owns preorder zero when nothing precedes it: an
        // unconditional arm has no guard, and a guarded arm whose statement
        // records one call row makes no guard or operand call before it.
        StatementNode::Transition(transition) => {
            matches!(
                program.statement_table.transition_target(transition.target),
                checked_trees::statement::TransitionTargetNode::Named { .. }
            ) && (transition.guard == checked_trees::statement::TransitionGuardNode::Always
                || sole_call_occurrence(checked, caller_state, coordinate.statement_index))
        }
        _ => false,
    };
    // The first arm of a closed-case dispatch over a call evaluates that call
    // as the statement's root, the way a local initializer does.
    let dispatch_subject =
        crate::expression_preparation::source_custody::dispatch_subjects::evaluated_case_subject(
            checked,
            program.statement_table.statements(state.statement_nodes),
            coordinate.statement_index as usize,
        );
    let direct_root = direct_root || dispatch_subject.is_some();
    let (source_target, arguments, source_site) = if coordinate.call_ordinal != 0 || !direct_root {
        let occurrence =
            nested::authored_occurrences(checked, caller_state, coordinate.statement_index)?
                .into_iter()
                .find_map(|(ordinal, occurrence)| {
                    (ordinal == coordinate.call_ordinal).then_some(occurrence)
                })
                .ok_or(LoweringError::Unsupported(
                    "nested call source custody has no exact authored preorder coordinate",
                ))?;
        match occurrence {
            nested::AuthoredOccurrence::Expression(expression) => {
                expression_call(checked, expression)?
            }
            nested::AuthoredOccurrence::TransitionTarget(target) => {
                let (target, arguments) = named_target_arguments(checked, machine.symbol, target)?;
                (target, arguments, None)
            }
            nested::AuthoredOccurrence::StatementRoot => {
                return unsupported("nested call coordinate names the statement root");
            }
        }
    } else {
        match statement {
            StatementNode::Transition(_) if let Some(subject) = dispatch_subject => {
                expression_call(checked, subject)?
            }
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
                expression_call(checked, assignment.value)?
            }
            StatementNode::Expression(expression) if coordinate.call_ordinal == 0 => {
                if validation::unit_statement_call_is_supported(
                    program,
                    machine,
                    state,
                    *expression,
                ) {
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
        explicit_arguments: arguments.to_vec(),
        scalar_arguments,
        erased_scalar_positions,
        structural_arguments,
        boundary: target.boundary,
        target_machine: target.machine,
        target_state: target.state,
    })
}

/// Whether the state's flow roster records exactly one live call row at
/// `statement_index`. A transition's named target always records its own row,
/// so a sole row is that invocation.
fn sole_call_occurrence(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: u32,
) -> bool {
    let control = &checked.facts.flow.control;
    control
        .states
        .iter()
        .find_map(|(_, state)| (state.state_symbol == caller_state).then_some(state.calls))
        .and_then(|calls| control.calls.span(calls))
        .is_some_and(|calls| {
            calls
                .iter()
                .filter(|call| {
                    call.statement_index == statement_index as usize
                        && !control.is_retired(caller_state, call)
                })
                .count()
                == 1
        })
}

/// Whether `statement_index` is the state's unconditional final transition
/// invoking another machine: the only transition call a linear operation
/// roster may hold. A guarded arm's invocation runs in its arm's own
/// completion block instead (Terminal calls in transition arms,
/// state_contracts.md), so a linear roster that holds one hoisted it.
pub(crate) fn linear_terminal_invocation(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: u32,
) -> Result<bool, LoweringError> {
    use checked_trees::statement::TransitionGuardNode;
    let (machine, state) = crate::declarations::state_declaration(checked, caller_state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    // A paired `Always` arm is the fallthrough of a guarded tail: its call is
    // an arm's, even though it is final and unguarded.
    if !matches!(statements.get(statement_index as usize), Some(StatementNode::Transition(transition))
            if transition.guard == TransitionGuardNode::Always)
        || statement_index as usize + 1 != statements.len()
        || statements[..statement_index as usize]
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        return Ok(false);
    }
    let Ok((target, _)) = named_terminal_arguments(checked, caller_state, statement_index) else {
        return Ok(false);
    };
    // Naming the enclosing machine re-enters its entry state: a state-graph
    // edge the checker never sequences as a call.
    Ok(target_signature(checked, machine.symbol, target)?.machine != machine.symbol)
}

/// Reconstruct a named terminal call from source, never from the operation
/// claiming it. No expression handle exists for this syntax: the statement
/// coordinate owns the invocation, and the explicit arguments keep theirs.
/// The invocation is an arm of the state's closing transition run: the
/// unconditional final arm a linear roster sequences, or a guarded arm whose
/// call runs only in that arm's completion block. Each plan owner rejoins
/// which placement it holds; this reconstruction checks only the source.
pub(super) fn named_terminal_arguments(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: u32,
) -> Result<(SymbolHandle, &[ExpressionHandle]), LoweringError> {
    use checked_trees::statement::{TransitionExit, TransitionGuardNode};
    let (machine, state) = crate::declarations::state_declaration(checked, caller_state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let Some(StatementNode::Transition(transition)) = statements.get(statement_index as usize)
    else {
        return unsupported("named call custody has no authored transition");
    };
    let unconditional_final = statement_index as usize == statements.len().saturating_sub(1)
        && transition.guard == TransitionGuardNode::Always;
    let closing_arm = statements[statement_index as usize..]
        .iter()
        .all(|statement| matches!(statement, StatementNode::Transition(_)));
    if !(unconditional_final || closing_arm)
        || transition.continuation.is_valid()
        || transition.exit != TransitionExit::Ordinary
        || !checked
            .statement_table
            .transition_target_is_valid(transition.target)
    {
        return unsupported("named call custody requires a terminal invocation arm");
    }
    let (source_target, explicit) =
        named_target_arguments(checked, machine.symbol, transition.target)?;
    // Naming the enclosing machine from a guarded arm re-enters its entry
    // state: a state-graph edge, never an arm's invocation.
    if !unconditional_final
        && target_signature(checked, machine.symbol, source_target)?.machine == machine.symbol
    {
        return unsupported("machine re-entry in a guarded arm is a state edge");
    }
    Ok((source_target, explicit))
}

/// Read a selected named invocation at its exact source target. The caller's
/// occurrence walk owns guard/target/continuation ordering independently of
/// whether that invocation is terminal or conditional.
fn named_target_arguments(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    target: checked_trees::statement::TransitionTargetHandle,
) -> Result<(SymbolHandle, &[ExpressionHandle]), LoweringError> {
    if !checked.statement_table.transition_target_is_valid(target) {
        return unsupported("named call custody has a stale target");
    }
    let checked_trees::statement::TransitionTargetNode::Named {
        path,
        arguments,
        static_machine_parameter,
        ..
    } = checked.statement_table.transition_target(target)
    else {
        return unsupported("named call custody has no named target");
    };
    if static_machine_parameter.is_valid() {
        return unsupported("named call custody has an unselected static target");
    }
    let source_target = path.symbol;
    let target = target_signature(checked, caller_machine, source_target)?;
    if target.machine == caller_machine && path.symbol != caller_machine {
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
    // A named `Namespace::requirement(...)` call keeps an invalid authored
    // leaf; the live target identity is its selected boundary declaration —
    // the same identity the flow occurrence and the emitted call record carry.
    let source_target = typed_trees::operator::selected_boundary_operator(&checked.typed, call)
        .map_or(call.target_symbol, |operator| operator.symbol);
    Ok((
        source_target,
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
    parameter_coordinates: ParameterCoordinates,
}

enum ParameterCoordinates {
    Authored,
    ServiceOperands,
}

impl TargetSignature<'_> {
    /// Service selection consumes its receiver before the boundary operand
    /// parameter list is numbered. Other retained rosters keep authored positions,
    /// including gaps left by erased or ambient parameters.
    pub(crate) fn authored_parameter_position(&self, retained_position: u32) -> Option<u32> {
        let retained_position = usize::try_from(retained_position).ok()?;
        let position = match self.parameter_coordinates {
            ParameterCoordinates::Authored => {
                self.parameters.get(retained_position)?;
                retained_position
            }
            ParameterCoordinates::ServiceOperands => {
                self.parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| !parameter.is_self)
                    .nth(retained_position)?
                    .0
            }
        };
        u32::try_from(position).ok()
    }
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
    // Captured calls retain the authored machine head, while execution names
    // its entry state. Resolve only the signature here: replacing the source
    // identity would break exact flow and borrow occurrence custody.
    let target_state = program
        .machine_by_symbol(source_target)
        .and_then(|machine| program.machine_states(machine).first())
        .map_or(source_target, |state| state.symbol);
    let mut states = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter_map(move |state| (state.symbol == target_state).then_some((machine, state)))
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
            parameter_coordinates: ParameterCoordinates::Authored,
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
            parameter_coordinates: ParameterCoordinates::Authored,
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
        parameter_coordinates: ParameterCoordinates::ServiceOperands,
    })
}
