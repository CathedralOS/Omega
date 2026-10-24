//! Bind a proved body equation to its exact entry and returned carrier claims.
//!
//! A numeric theorem and a custody transaction are separate judgments. This
//! binding retains the former without granting the latter: an entry claim may
//! already have moved, and two inputs with adjacent geometry may have unrelated
//! backing. Consumers must never treat these rows as establishment receipts.
//! In particular, a returned raw linear carrier already owns its nominal claim;
//! derived qualification must attach authority to that occurrence, not mint it
//! again. The shared outgoing capture owns result discovery, including nested
//! fields and local moves, rather than a constructor-shape recognizer here.

use super::content_paths::{ClaimBindings, content_segments_to_fact_path, entry_claim_identity};
use super::partition_wrappers::ordinary_returns;
use crate::checks::multiplicity::{LinearPlace, ReturnedClaimSource, completed_owned_claims};
use checked_trees::CheckFacts;
use checked_trees::facts::ContentBodyConservationFact;
use language_semantics::content::{
    ContentConservationOwnerKind, ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion,
};

pub(crate) fn observe_body_completion(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    places: &[LinearPlace],
    events: &[checked_trees::FlowPermissionEventFact],
    evaluation_events: &[checked_trees::FlowPermissionEventFact],
) -> Vec<arena::Handle<ContentBodyConservationFact>> {
    let mut observed = Vec::new();
    let entry = program
        .machine_states(machine)
        .first()
        .map(|state| state.symbol);
    let plans = facts
        .qualifications
        .content
        .conservation_plans
        .iter()
        .enumerate()
        .filter(|(_, plan)| {
            plan.owner_kind == ContentConservationOwnerKind::Machine
                && plan.owner == machine.symbol
                && (plan.callable == state.symbol || Some(plan.callable) == entry)
        })
        .map(|(position, plan)| (position, plan.clone()))
        .collect::<Vec<_>>();
    for (plan_index, plan) in plans {
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            continue;
        }
        let Some(plan) = plan_at_state(program, facts, state, plan) else {
            continue;
        };
        let Some((inputs, outputs)) =
            owned_return_sides(plan.equation.left(), plan.equation.right())
        else {
            // Relations involving current parameters or mixed frontiers still
            // have their ordinary numeric meaning, but are not this entry-to-
            // return relation. Do not reinterpret them as owned consumption.
            continue;
        };
        for returned_at in ordinary_returns(program, facts, state) {
            let outgoing = facts.flow.control.outgoing.get(returned_at);
            let outgoing_evaluation = if outgoing.statement_index != statement_index {
                checked_trees::FlowPermissionEvaluation::Statement
            } else if outgoing.transition_target.is_valid() {
                checked_trees::FlowPermissionEvaluation::TransitionTarget(
                    outgoing.transition_target,
                )
            } else {
                checked_trees::FlowPermissionEvaluation::Statement
            };
            if !outgoing.returned_value.is_valid() || outgoing_evaluation != evaluation {
                continue;
            }
            for completed_value in
                completed_producers(program, facts, state, outgoing.returned_value)
            {
                let captured = facts.flow.control.operands.get(completed_value);
                if captured.statement_index != statement_index || captured.expression != expression
                {
                    continue;
                }
                // The equation must hold at this value's completion, not merely
                // at an eventual normal return. The query shares observation
                // order with domain predicates and imports no ensures premise.
                if !validation::completed_value_proves_content_conservation(
                    program,
                    machine,
                    state,
                    captured.statement_index,
                    captured.expression,
                    captured.type_reference,
                    &plan,
                    &facts.qualifications.content.plans,
                ) {
                    continue;
                }
                let returned =
                    completed_owned_claims(program, facts, captured, places, evaluation_events);
                let Ok(returned) = returned else {
                    // Missing/ambiguous custody has its owning checker error;
                    // a theorem cannot fill an absent claim occurrence.
                    continue;
                };
                let mut input_bindings = ClaimBindings::default();
                let mut output_bindings = ClaimBindings::default();
                if input_bindings
                    .extend_term(inputs, |subject| {
                        let ContentPlaceRoot::Parameter { symbol, .. } = subject.root else {
                            return None;
                        };
                        let path = content_segments_to_fact_path(&subject.segments)?;
                        entry_claim_identity(facts, state.symbol, symbol, &path, events.iter())
                    })
                    .is_none()
                    || output_bindings
                        .extend_term(outputs, |subject| {
                            let path = content_segments_to_fact_path(&subject.segments)?;
                            let mut matching = returned.iter().filter(|claim| claim.path == path);
                            let claim = matching.next()?;
                            if matching.next().is_some() {
                                return None;
                            }
                            match claim.source {
                                ReturnedClaimSource::Live(identity) => Some(identity),
                                ReturnedClaimSource::Inactive => None,
                            }
                        })
                        .is_none()
                {
                    continue;
                }
                // Result field annotations ask for these predicates; they do not
                // prove them. Judge each projected value in the same completed
                // frame as the conservation equation, before recording a candidate.
                if !output_bindings.rows().all(|(binding, path)| {
                    validation::completed_value_proves_domain_predicate(
                        program,
                        machine,
                        state,
                        statement_index,
                        expression,
                        binding.domain,
                        typed_trees::expression::ExpressionHandle::invalid(),
                        path,
                    )
                }) {
                    continue;
                }
                let inputs = input_bindings.publish(facts);
                let outputs = output_bindings.publish(facts);
                let conservation = ContentBodyConservationFact {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    returned_at,
                    completed_value,
                    evaluation,
                    plan_index,
                    inputs,
                    outputs,
                    available_inputs: Default::default(),
                };
                let available = crate::checks::multiplicity::observe_body_inputs(
                    program,
                    facts,
                    &conservation,
                    places,
                );
                let Ok(available) = available else {
                    continue;
                };
                let conservation = facts
                    .qualifications
                    .content
                    .body_conservations
                    .insert(conservation);
                crate::checks::multiplicity::publish_body_inputs(facts, conservation, available);
                observed.push(conservation);
            }
        }
    }
    observed
}

/// A machine guarantee may be consumed in another state only after the shared
/// arrival analysis proves the exact parameter origin on every predecessor.
/// Rebase its subjects, never import the guarantee as an entry assumption.
fn plan_at_state(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    mut plan: language_semantics::content::ContentConservationPlan,
) -> Option<language_semantics::content::ContentConservationPlan> {
    if plan.callable == state.symbol {
        return Some(plan);
    }
    let flow = facts.flow.control.state(plan.owner, state.symbol)?;
    let origins = facts
        .flow
        .control
        .exits
        .span_or_empty(flow.exits)
        .iter()
        .flat_map(|exit| {
            facts
                .flow
                .control
                .exit_parameter_origins
                .span_or_empty(exit.parameter_origins)
        })
        .collect::<Vec<_>>();
    fn rebase(
        term: &mut ContentConservationTerm,
        origins: &[&checked_trees::FlowExitParameterOrigin],
        parameters: &[typed_trees::signature::StateParameter],
    ) -> Option<()> {
        match term {
            ContentConservationTerm::Separate(children) => {
                for child in children {
                    rebase(child, origins, parameters)?;
                }
            }
            ContentConservationTerm::Projection { subject, .. } => {
                if let ContentPlaceRoot::Parameter {
                    position,
                    symbol,
                    name,
                } = &mut subject.root
                {
                    let mut matching = origins
                        .iter()
                        .filter(|origin| origin.entry_parameter == *symbol);
                    let target = matching.next()?.state_parameter;
                    if !target.is_valid() || matching.any(|origin| origin.state_parameter != target)
                    {
                        return None;
                    }
                    let (ordinal, parameter) = parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.symbol == target)?;
                    *position = u32::try_from(ordinal).ok()?;
                    *symbol = target;
                    *name = parameter.name.as_str().to_owned();
                }
            }
        }
        Some(())
    }
    let mut left = plan.equation.left().clone();
    let mut right = plan.equation.right().clone();
    let parameters = program.state_parameters(state);
    rebase(&mut left, &origins, parameters)?;
    rebase(&mut right, &origins, parameters)?;
    plan.callable = state.symbol;
    plan.equation = language_semantics::content::ContentConservationEquation::new(left, right);
    plan.report_fingerprint =
        language_semantics::content::conservation_report_fingerprint(&plan.algebra, &plan.equation);
    Some(plan)
}

/// Follow exact produced values through unchanged resident storage. This
/// discovers where a return demand may be proved; each candidate still needs
/// its own completion-time theorem and live custody. Never use retention
/// profiles (which describe loans), copyability, or a declaration initializer
/// as a substitute for the completed store. Alternatives stay distinct.
fn completed_producers(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    returned: arena::Handle<checked_trees::FlowOperandFact>,
) -> Vec<arena::Handle<checked_trees::FlowOperandFact>> {
    let mut pending = vec![(returned, Vec::new())];
    let mut visited = Vec::new();
    let mut completed = Vec::new();
    while let Some((handle, projection)) = pending.pop() {
        if !facts.flow.control.operands.is_valid(handle)
            || visited.contains(&(handle, projection.clone()))
        {
            continue;
        }
        visited.push((handle, projection.clone()));
        let capture = facts.flow.control.operands.get(handle);
        if capture.state_symbol != state.symbol {
            continue;
        }
        if projection.is_empty() && !completed.contains(&handle) {
            completed.push(handle);
        }
        if let Some(predecessors) =
            crate::flow::completed_values::predecessors(program, facts, handle, &projection)
        {
            pending.extend(predecessors);
        }
    }
    completed
}

fn side_has_role(term: &ContentConservationTerm, entry: bool) -> bool {
    match term {
        ContentConservationTerm::Projection { subject, .. } => {
            if entry {
                subject.version == ContentPlaceVersion::Entry
                    && matches!(subject.root, ContentPlaceRoot::Parameter { .. })
            } else {
                subject.version == ContentPlaceVersion::Current
                    && subject.root == ContentPlaceRoot::Result
            }
        }
        ContentConservationTerm::Separate(children) => {
            !children.is_empty() && children.iter().all(|child| side_has_role(child, entry))
        }
    }
}

/// Equation canonicalization may reverse the authored sides. Classify by
/// exact root/revision, not spelling or serialized side order. Empty separate
/// terms provide no owned input or returned occurrence.
pub(super) fn owned_return_sides<'term>(
    left: &'term ContentConservationTerm,
    right: &'term ContentConservationTerm,
) -> Option<(
    &'term ContentConservationTerm,
    &'term ContentConservationTerm,
)> {
    if side_has_role(left, true) && side_has_role(right, false) {
        Some((left, right))
    } else if side_has_role(right, true) && side_has_role(left, false) {
        Some((right, left))
    } else {
        None
    }
}
