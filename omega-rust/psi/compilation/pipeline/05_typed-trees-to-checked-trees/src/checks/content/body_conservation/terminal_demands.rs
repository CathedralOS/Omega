//! A terminal callee must prove the caller's exact returned-content demand.
//!
//! Transport substitutes only unchanged, typed incoming values captured by the
//! actual invocation. The resulting equation is still proved from the callee's
//! own entry contract at each raw return; no caller fact or custody is imported.

use std::collections::{BTreeMap, HashSet};

use arena::Handle;
use checked_trees::{CheckFactsRef, FlowOperandFact, FlowStateOutgoingFact};
use language_semantics::content::{
    ContentConservationEquation, ContentConservationPlan, ContentConservationTerm,
    ContentPlaceRoot, conservation_report_fingerprint,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;
use validation::completed_storage::{StoredObservation, StoredOperand};

/// Shared across topology discovery and completed-value correspondence. An
/// expanding cycle declines transport rather than manufacturing a finite law.
const TRANSPORT_WORK: usize = 65_536;

pub(super) fn at_state(
    program: &dyn validation::CheckingInput,
    facts: CheckFactsRef<'_>,
    source: &ContentConservationPlan,
    machine: &Machine,
    state: &State,
) -> Vec<ContentConservationPlan> {
    if source.owner == machine.symbol
        || super::owned_return_sides(source.equation.left(), source.equation.right()).is_none()
    {
        return Vec::new();
    }
    let Some(owner) = program.machine_by_symbol(source.owner) else {
        return Vec::new();
    };
    let Some(entry) = program.machine_states(owner).first() else {
        return Vec::new();
    };
    let mut pending = Vec::new();
    for initial in program.machine_states(owner) {
        if (initial.symbol == source.callable || entry.symbol == source.callable)
            && let Some(plan) = super::plan_at_state(program, facts, initial, source.clone())
        {
            pending.push(plan);
        }
    }
    let mut remaining = TRANSPORT_WORK;
    let mut visited = BTreeMap::<_, Vec<ContentConservationPlan>>::new();
    let mut demands = Vec::new();
    while let Some(plan) = pending.pop() {
        if !spend(&mut remaining, 1) {
            break;
        }
        let Some((current_machine, current_state)) = validation::find_state(program, plan.callable)
        else {
            continue;
        };
        let known = visited
            .entry((identity(current_state.symbol), plan.report_fingerprint))
            .or_default();
        if !spend(&mut remaining, 1 + known.len()) {
            break;
        }
        // A report hash narrows this query-local comparison, never decides
        // semantic identity or discards a distinct substitution on collision.
        if known.contains(&plan) {
            continue;
        }
        known.push(plan.clone());
        if plan.owner == machine.symbol && plan.callable == state.symbol {
            if plan != *source {
                demands.push(plan);
            }
            continue;
        }
        let Some(flow) = facts
            .flow
            .control
            .unique_state(current_machine.symbol, current_state.symbol)
        else {
            continue;
        };
        for outgoing in facts.flow.control.outgoing.span_or_empty(flow.outgoing) {
            if !spend(&mut remaining, 1) {
                return demands;
            }
            if outgoing.cannot_return_normally {
                continue;
            }
            if outgoing.transition_target.is_valid() {
                let Some(statement) = program
                    .statement_table
                    .statement_at(current_state, outgoing.statement_index)
                    .and_then(|statement| statement.as_transition())
                else {
                    continue;
                };
                if statement.exit != typed_trees::statement::TransitionExit::Ordinary {
                    continue;
                }
                for invocation in validation::completing_transition_invocations(
                    program,
                    outgoing.transition_target,
                ) {
                    if let Some(next) = transfer(
                        program,
                        facts,
                        current_state,
                        outgoing,
                        invocation.target,
                        invocation.expression,
                        invocation.arguments,
                        &plan,
                        &mut remaining,
                    ) {
                        pending.push(next);
                    }
                }
            } else if let Some(alternatives) = validation::completed_storage::terminal_alternatives(
                facts.flow.control,
                outgoing.returned_value,
            ) {
                for returned in alternatives {
                    let capture = facts.flow.control.operands.get(returned);
                    if let Some(invocation) =
                        validation::completing_call_transfer(program, capture.expression)
                        && let Some(next) = transfer(
                            program,
                            facts,
                            current_state,
                            outgoing,
                            invocation.target,
                            invocation.expression,
                            invocation.arguments,
                            &plan,
                            &mut remaining,
                        )
                    {
                        pending.push(next);
                    }
                }
            }
        }
    }
    demands
}

#[allow(clippy::too_many_arguments)]
fn transfer(
    program: &dyn validation::CheckingInput,
    facts: CheckFactsRef<'_>,
    state: &State,
    outgoing: &FlowStateOutgoingFact,
    target: SymbolHandle,
    expression: ExpressionHandle,
    arguments: &[ExpressionHandle],
    plan: &ContentConservationPlan,
    remaining: &mut usize,
) -> Option<ContentConservationPlan> {
    if !spend(remaining, 1) {
        return None;
    }
    let next_state = validation::call_target_state(program, target)?;
    let next_machine = program.machine_holding_state(next_state.symbol)?;
    if next_machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !same_type(program, state.return_type, next_state.return_type)
    {
        return None;
    }
    let flow = facts.flow.control.unique_state(plan.owner, state.symbol)?;
    if !spend(remaining, flow.calls.len()) {
        return None;
    }
    let mut calls = facts
        .flow
        .control
        .calls
        .span(flow.calls)?
        .iter()
        .filter(|call| {
            call.statement_index == outgoing.statement_index
                && call.authored_target == target
                && call.authored_expression == expression
                && !call.authored_receiver.is_valid()
                && call.authored_source_custody_valid
                && facts
                    .flow
                    .control
                    .call_arguments
                    .span(call.authored_arguments)
                    == Some(arguments)
                && facts
                    .flow
                    .control
                    .operands
                    .span(call.argument_operands())
                    .is_some_and(|operands| {
                        operands.iter().all(|operand| {
                            !outgoing.transition_target.is_valid()
                                || operand.evaluation
                                    == checked_trees::FlowPermissionEvaluation::TransitionTarget(
                                        outgoing.transition_target,
                                    )
                        })
                    })
        });
    let call = calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    let parameters = program.state_parameters(next_state);
    if parameters.len() != call.argument_operands().len() {
        return None;
    }
    let mut bindings = Vec::new();
    for (parameter, operand) in parameters.iter().zip(call.argument_operands().handles()) {
        if !spend(remaining, 1) {
            return None;
        }
        if parameter.is_mutable || parameter.is_const {
            continue;
        }
        if let Some(source) = incoming_parameter(
            program,
            facts,
            state,
            operand,
            parameter.type_reference,
            remaining,
        ) {
            bindings.push((source, parameter));
        }
    }
    let mut plan = plan.clone();
    let mut left = plan.equation.left().clone();
    let mut right = plan.equation.right().clone();
    rebase(&mut left, parameters, &bindings)?;
    rebase(&mut right, parameters, &bindings)?;
    plan.owner = next_machine.symbol;
    plan.callable = next_state.symbol;
    plan.equation = ContentConservationEquation::new(left, right);
    plan.report_fingerprint = conservation_report_fingerprint(&plan.algebra, &plan.equation);
    Some(plan)
}

/// Every predecessor must be the same unchanged complete entry value. A local
/// read may follow its retained store, but never a declaration initializer.
fn incoming_parameter(
    program: &dyn validation::CheckingInput,
    facts: CheckFactsRef<'_>,
    state: &State,
    operand: Handle<FlowOperandFact>,
    reference: TypeReferenceHandle,
    remaining: &mut usize,
) -> Option<SymbolHandle> {
    // Inline storage preserves descriptors, not the contents they borrow.
    // Without a complete empty loan frontier, the parent's entry content
    // cannot become the child's entry snapshot through this value relation.
    if !validation::borrow_contracts::borrow_carrying_owner_paths(program, reference)?.is_empty() {
        return None;
    }
    let mut pending = vec![(operand, Vec::new(), false)];
    let mut active = HashSet::new();
    let mut completed = HashSet::new();
    let mut incoming = None;
    while let Some((operand, projection, leaving)) = pending.pop() {
        if !spend(remaining, 1 + projection.len()) || !facts.flow.control.operands.is_valid(operand)
        {
            return None;
        }
        let key = (
            (operand.arena_index(), operand.generation()),
            projection.clone(),
        );
        if leaving {
            active.remove(&key);
            completed.insert(key);
            continue;
        }
        if completed.contains(&key) {
            continue;
        }
        if !active.insert(key.clone()) {
            return None;
        }
        let capture = facts.flow.control.operands.get(operand);
        let actual_type = validation::project_type_reference_from_segments(
            program,
            capture.type_reference,
            &projection,
        )?;
        if capture.state_symbol != state.symbol
            || !same_type(program, actual_type, reference)
            || validation::reference_shell(program, actual_type).is_some()
        {
            return None;
        }
        let source = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            capture.statement_index,
            capture.expression,
        )
        .map(|mut source| {
            source.segments.extend_from_slice(&projection);
            source
        });
        if let Some(source) = source
            && let Some(StoredOperand::Incoming(source)) =
                validation::completed_storage::at_snapshot(
                    program,
                    facts.flow.control,
                    capture.storage_snapshot,
                    &source,
                    StoredObservation::InlineContents,
                )
            && source.segments.is_empty()
            && let facts::PlaceRoot::Symbol(symbol) = source.root
            && let Some(parameter) = program.state_parameter_by_symbol(state, symbol)
            && !parameter.is_mutable
            && !parameter.is_const
            && same_type(program, parameter.type_reference, reference)
        {
            if incoming.is_some_and(|previous| previous != symbol) {
                return None;
            }
            incoming = Some(symbol);
            active.remove(&key);
            completed.insert(key);
            continue;
        }
        let predecessors =
            crate::flow::completed_values::predecessors(program, facts, operand, &projection)?;
        if predecessors.is_empty() {
            return None;
        }
        pending.push((operand, projection, true));
        pending.extend(
            predecessors
                .into_iter()
                .map(|(operand, projection)| (operand, projection, false)),
        );
    }
    incoming
}

fn rebase(
    term: &mut ContentConservationTerm,
    parameters: &[typed_trees::signature::StateParameter],
    bindings: &[(SymbolHandle, &typed_trees::signature::StateParameter)],
) -> Option<()> {
    match term {
        ContentConservationTerm::Separate(children) => {
            for child in children {
                rebase(child, parameters, bindings)?;
            }
        }
        ContentConservationTerm::Projection { subject, .. } => {
            if let ContentPlaceRoot::Parameter { symbol, .. } = subject.root {
                let mut targets = bindings
                    .iter()
                    .filter_map(|(source, target)| (*source == symbol).then_some(*target));
                let target = targets.next()?;
                if targets.any(|other| other.symbol != target.symbol) {
                    return None;
                }
                let position = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == target.symbol)?;
                subject.root = ContentPlaceRoot::Parameter {
                    position: u32::try_from(position).ok()?,
                    symbol: target.symbol,
                    name: target.name.as_str().to_owned(),
                };
            }
        }
    }
    Some(())
}

fn identity(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

fn same_type(
    program: &dyn validation::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    left.is_valid()
        && right.is_valid()
        && program.normalized_type_identity(left) == program.normalized_type_identity(right)
}

fn spend(remaining: &mut usize, work: usize) -> bool {
    let Some(next) = remaining.checked_sub(work) else {
        return false;
    };
    *remaining = next;
    true
}
