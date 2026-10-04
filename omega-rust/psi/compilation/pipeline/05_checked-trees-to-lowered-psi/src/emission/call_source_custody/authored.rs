//! Authored roots and formal-to-explicit argument mapping for retained calls.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::types::PrimitiveType;
use checked_trees::{CheckedUnitCallCoordinate, NominalMachineUseSite};
use symbols::SymbolHandle;

pub(crate) mod nested;

pub(crate) struct AuthoredCall {
    pub source_target: SymbolHandle,
    pub source_site: Option<NominalMachineUseSite>,
    /// Original explicit source order, before partitioning scalar and structural lanes.
    pub explicit_arguments: Vec<ExpressionHandle>,
    pub scalar_arguments: Vec<(ExpressionHandle, PrimitiveType)>,
    pub structural_arguments: Vec<(u32, ExpressionHandle)>,
    /// The receiver a call expression spells before its name; the zero
    /// handle for every other site.
    pub receiver: ExpressionHandle,
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
        position: u32,
        is_self: bool,
    ) -> Option<ExpressionHandle> {
        self.structural_arguments
            .iter()
            .find_map(|(formal, expression)| (*formal == position).then_some(*expression))
            .or_else(|| (is_self && self.receiver.is_valid()).then_some(self.receiver))
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
        nested::AuthoredOccurrence::Expression => {
            unsupported("terminal call occurrence names a guard or operand expression")
        }
    }
}

/// The invocation stage 04 bound at `coordinate`. Its row names the callee as
/// spelled, the explicit arguments and the syntax that spells the call, so
/// lowering never walks the statement again to find the occurrence.
fn planned_call<'checked>(
    checked: &'checked CheckedTrees,
    state: &checked_trees::state::State,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<PlannedCall<'checked>, LoweringError> {
    let control = &checked.facts.flow.control;
    let call = control
        .call_at(
            caller_state,
            coordinate.statement_index as usize,
            coordinate.call_ordinal as usize,
        )
        .ok_or(LoweringError::Unsupported(
            "call coordinate has no checked flow occurrence",
        ))?;
    let site = match call.authored_site {
        checked_trees::FlowCallSite::Expression => {
            Some(NominalMachineUseSite::Expression(call.authored_expression))
        }
        checked_trees::FlowCallSite::Statement => {
            let index = state
                .statement_nodes
                .start()
                .arena_index()
                .checked_add(coordinate.statement_index)
                .ok_or(LoweringError::Unsupported(
                    "call source statement identity overflows",
                ))?;
            Some(NominalMachineUseSite::Statement(arena::Handle::from_parts(
                index,
                state.statement_nodes.start().generation(),
            )))
        }
        checked_trees::FlowCallSite::NamedTransition => None,
        checked_trees::FlowCallSite::Unbound => {
            return unsupported("call coordinate has no bound authored site");
        }
    };
    Ok(PlannedCall {
        target: call.authored_target,
        arguments: control
            .call_arguments
            .span_or_empty(call.authored_arguments),
        receiver: call.authored_receiver,
        site,
    })
}

struct PlannedCall<'checked> {
    target: SymbolHandle,
    arguments: &'checked [ExpressionHandle],
    receiver: ExpressionHandle,
    site: Option<NominalMachineUseSite>,
}

pub(crate) fn locate_source(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<AuthoredCall, LoweringError> {
    let program = &checked.typed;
    let (machine, state) = crate::declarations::state_declaration(checked, caller_state)?;
    let PlannedCall {
        target: source_target,
        arguments,
        receiver,
        site: source_site,
    } = planned_call(checked, state, caller_state, coordinate)?;
    let target = crate::declarations::target_signature(checked, machine.symbol, source_target)?;
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
        scalar_arguments.push((argument, primitive));
    }
    Ok(AuthoredCall {
        source_target,
        source_site,
        explicit_arguments: arguments.to_vec(),
        scalar_arguments,
        structural_arguments,
        receiver,
        boundary: target.boundary,
        target_machine: target.machine,
        target_state: target.state,
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
    Ok(
        crate::declarations::target_signature(checked, machine.symbol, target)?.machine
            != machine.symbol,
    )
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
        && crate::declarations::target_signature(checked, machine.symbol, source_target)?.machine
            == machine.symbol
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
    let target = crate::declarations::target_signature(checked, caller_machine, source_target)?;
    if target.machine == caller_machine && path.symbol != caller_machine {
        return unsupported("internal state transfer is not a terminal machine invocation");
    }
    let explicit = checked.statement_table.expression_handles(*arguments);
    if explicit.len() != arguments.count() as usize {
        return unsupported("named call custody has an invalid authored argument span");
    }
    Ok((source_target, explicit))
}
