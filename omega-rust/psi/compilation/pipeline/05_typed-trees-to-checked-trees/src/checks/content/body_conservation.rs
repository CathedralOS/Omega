//! Bind a completed body equation to its exact entry and returned carrier claims.
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
use crate::checks::multiplicity::observe_body_inputs;
use crate::checks::multiplicity::publish_body_inputs;
use crate::checks::multiplicity::{
    CompletedClaimLocation, LinearPlace, ReturnedClaimSource, completed_owned_claims,
};
use crate::flow::SelectionConditions;
use crate::flow::state_exit_parameter_origins;
use checked_trees::CheckFacts;
use checked_trees::facts::ContentBodyConservationFact;
use language_semantics::content::{
    ContentConservationOwnerKind, ContentConservationTerm as ConservationTerm, ContentPlaceRoot,
    ContentPlaceVersion,
};

mod terminal_demands;

pub(crate) fn observe_body_completion(
    program: &dyn validation::CheckingInput,
    facts: &mut checked_trees::CheckFactsWalk<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    places: &[LinearPlace],
    events: &[checked_trees::FlowPermissionEventFact],
    evaluation_events: &[checked_trees::FlowPermissionEventFact],
    location: CompletedClaimLocation,
    candidates: &[ConservationCandidate],
) -> Vec<arena::Handle<ContentBodyConservationFact>> {
    if facts.qualifications.content.conservation_plans.is_empty() {
        return Vec::new();
    }
    let mut observed = Vec::new();
    // The immutable site preparation already transported terminal demands.
    // Reuse those plans instead of rediscovering the call graph at every
    // completed operand during the sequential custody walk.
    let mut plans = Vec::new();
    for candidate in candidates {
        if !plans.iter().any(|(plan_index, plan)| {
            *plan_index == candidate.plan_index && *plan == &candidate.plan
        }) {
            plans.push((candidate.plan_index, &candidate.plan));
        }
    }
    for (plan_index, plan) in plans {
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            continue;
        }
        let Some((inputs, outputs)) =
            owned_return_sides(plan.equation.left(), plan.equation.right())
        else {
            // Relations involving current parameters or mixed frontiers still
            // have their ordinary numeric meaning, but are not this entry-to-
            // return relation. Do not reinterpret them as owned consumption.
            continue;
        };
        for returned_at in ordinary_returns(program, facts.flow.control.outgoing.iter(), state) {
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
                completed_producers(program, facts.view(), state, outgoing.returned_value)
            {
                let captured = facts.flow.control.operands.get(completed_value);
                if captured.statement_index != statement_index || captured.expression != expression
                {
                    continue;
                }
                // The equation must hold at this value's completion, not merely
                // at an eventual normal return. The query shares observation
                // order with domain predicates. Conditional call laws remain
                // provisional here; the rooted publication pass checks their
                // exact sources before any callable theorem can be consumed.
                let prepared = candidates.iter().find(|candidate| {
                    candidate.plan_index == plan_index
                        && candidate.plan == *plan
                        && candidate.returned_at == returned_at
                        && candidate.completed_value == completed_value
                });
                // The prepared answers only save time; an unprepared query is
                // asked here exactly as preparation would have asked it.
                let admitted = prepared.map_or_else(
                    || {
                        has_conservation_candidate(
                            program,
                            facts.view(),
                            machine,
                            state,
                            completed_value,
                            &plan,
                        )
                    },
                    |candidate| candidate.admitted,
                );
                if !admitted {
                    continue;
                }
                let mut conditions = SelectionConditions::new();
                let returned = completed_owned_claims(
                    program,
                    facts.view(),
                    completed_value,
                    location,
                    places,
                    evaluation_events,
                    &mut conditions,
                );
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
                        entry_claim_identity(
                            facts.view(),
                            state.symbol,
                            symbol,
                            &path,
                            events.iter(),
                        )
                    })
                    .is_none()
                    || output_bindings
                        .extend_term(outputs, |subject| {
                            let path = content_segments_to_fact_path(&subject.segments)?;
                            let mut matching = returned.iter().filter(|claim| claim.path == path);
                            let first = matching.next()?;
                            let ReturnedClaimSource::Live(identity) = first.source else {
                                return None;
                            };
                            let mut coverage = first.condition;
                            for claim in matching {
                                if claim.source != first.source {
                                    return None;
                                }
                                coverage = conditions.any(coverage, claim.condition);
                            }
                            conditions
                                .covers(facts.flow.control, coverage)
                                .then_some(identity)
                        })
                        .is_none()
                {
                    continue;
                }
                // Result field annotations ask for these predicates; they do not
                // prove them. Judge each projected value in the same completed
                // frame as the conservation equation, before recording a candidate.
                if !output_bindings.rows().all(|(binding, path)| {
                    prepared
                        .and_then(|candidate| candidate.proven_domain(binding.domain, path))
                        .unwrap_or_else(|| {
                            proves_domain(
                                program,
                                machine,
                                state,
                                (statement_index, expression),
                                binding.domain,
                                path,
                            )
                        })
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
                    body_plan_index: facts.qualifications.content.body_conservation_plans.len(),
                    inputs,
                    outputs,
                    available_inputs: Default::default(),
                };
                let available = observe_body_inputs(program, facts.view(), &conservation, places);
                let Ok(available) = available else {
                    continue;
                };
                facts
                    .qualifications
                    .content
                    .body_conservation_plans
                    .push(plan.clone());
                let conservation = facts
                    .qualifications
                    .content
                    .body_conservations
                    .insert(conservation);
                publish_body_inputs(facts, conservation, available);
                observed.push(conservation);
            }
        }
    }
    observed
}

/// The proof answers one conservation site needs: whether its completed value
/// is a conservation candidate and, if so, which result-field domains it
/// proves. Both are proof searches over the program, flow control rows and
/// content plans, none of which the permission walk writes, so the walk
/// prepares every site of a state on the checking pool before its sequential
/// custody replay reaches the completions that ask.
#[derive(Debug, Clone)]
pub(crate) struct ConservationCandidate {
    plan_index: usize,
    plan: language_semantics::content::ContentConservationPlan,
    returned_at: arena::Handle<checked_trees::FlowStateOutgoingFact>,
    completed_value: arena::Handle<checked_trees::FlowOperandFact>,
    admitted: bool,
    /// Each output projection's domain and claim path with its answer; empty
    /// for a site that is not admitted, which never asks.
    proven_domains: Vec<(symbols::SymbolHandle, Vec<facts::PlaceSegment>, bool)>,
}

impl ConservationCandidate {
    fn proven_domain(
        &self,
        domain: symbols::SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<bool> {
        self.proven_domains
            .iter()
            .find(|(proven, proven_path, _)| *proven == domain && proven_path == path)
            .map(|(_, _, answer)| *answer)
    }
}

/// The conservation sites one state's completions can ask about: every
/// returned producer of every machine conservation plan the state owns, with
/// the plan's output projections. Enumerating them is cheap; answering them
/// is a proof search per site.
#[derive(Default)]
pub(crate) struct ConservationSites {
    plans: Vec<(
        language_semantics::content::ContentConservationPlan,
        Vec<(symbols::SymbolHandle, Vec<facts::PlaceSegment>)>,
    )>,
    /// `(plan slot, plan index, returned_at, completed_value)`.
    sites: Vec<(
        usize,
        usize,
        arena::Handle<checked_trees::FlowStateOutgoingFact>,
        arena::Handle<checked_trees::FlowOperandFact>,
    )>,
    /// Site positions grouped by completed value, in first-appearance order.
    groups: Vec<Vec<usize>>,
}

impl ConservationSites {
    /// How many completed values the sites ask about.
    pub(crate) fn groups(&self) -> usize {
        self.groups.len()
    }

    /// Answer every site of one completed value: each candidate query and,
    /// when admitted, its domains, as `(site position, answer)`. Every plan
    /// and domain asks about the same completed value, so one shared proof
    /// walk answers them together; a question it does not prove is asked
    /// alone, exactly as before.
    pub(crate) fn answer_group(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        group: usize,
    ) -> Vec<(usize, ConservationCandidate)> {
        let sites = &self.groups[group];
        let completed_value = self.sites[sites[0]].3;
        let captured = facts.flow.control.operands.get(completed_value);
        let plans = sites
            .iter()
            .map(|&site| &self.plans[self.sites[site].0].0)
            .collect::<Vec<_>>();
        let mut domains: Vec<(symbols::SymbolHandle, &[facts::PlaceSegment])> = Vec::new();
        for &site in sites {
            for (domain, path) in &self.plans[self.sites[site].0].1 {
                if !domains
                    .iter()
                    .any(|(known, known_path)| known == domain && *known_path == path.as_slice())
                {
                    domains.push((*domain, path));
                }
            }
        }
        let (shared_candidates, shared_domains) = validation::completed_value_conservation_answers(
            program,
            machine,
            state,
            &validation::CompletedConservationQueries {
                statement_index: captured.statement_index,
                expression: captured.expression,
                reference: captured.type_reference,
                plans: &plans,
                projections: &facts.qualifications.content.plans,
                domains: &domains,
            },
        );
        let site = (captured.statement_index, captured.expression);
        sites
            .iter()
            .zip(shared_candidates)
            .map(|(&position, shared)| {
                let (plan, plan_index, returned_at, completed_value) = self.sites[position];
                let (plan, output_projections) = &self.plans[plan];
                let admitted = shared
                    || has_conservation_candidate(
                        program,
                        facts.view(),
                        machine,
                        state,
                        completed_value,
                        plan,
                    );
                let proven_domains = if admitted {
                    output_projections
                        .iter()
                        .map(|(domain, path)| {
                            let shared = domains
                                .iter()
                                .position(|(known, known_path)| {
                                    known == domain && *known_path == path.as_slice()
                                })
                                .is_some_and(|known| shared_domains[known]);
                            let answer = shared
                                || proves_domain(program, machine, state, site, *domain, path);
                            (*domain, path.clone(), answer)
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                (
                    position,
                    ConservationCandidate {
                        plan_index,
                        plan: plan.clone(),
                        returned_at,
                        completed_value,
                        admitted,
                        proven_domains,
                    },
                )
            })
            .collect()
    }
}

/// Enumerate the sites `observe_body_completion` can reach in this state.
pub(crate) fn conservation_sites(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> ConservationSites {
    let mut sites = ConservationSites::default();
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return sites;
    }
    for (plan_index, plan) in state_conservation_plans(program, facts.view(), machine, state) {
        let Some(plan) = plan_at_state(program, facts.view(), state, plan) else {
            continue;
        };
        let Some((_, outputs)) = owned_return_sides(plan.equation.left(), plan.equation.right())
        else {
            continue;
        };
        let mut output_projections = Vec::new();
        let mut pending = vec![outputs];
        while let Some(term) = pending.pop() {
            match term {
                ConservationTerm::Projection {
                    domain, subject, ..
                } => {
                    if let Some(path) = content_segments_to_fact_path(&subject.segments) {
                        output_projections.push((*domain, path));
                    }
                }
                ConservationTerm::Separate(children) => pending.extend(children.iter()),
            }
        }
        let slot = sites.plans.len();
        for returned_at in ordinary_returns(program, facts.flow.control.outgoing.iter(), state) {
            let outgoing = facts.flow.control.outgoing.get(returned_at);
            if !outgoing.returned_value.is_valid() {
                continue;
            }
            for completed_value in
                completed_producers(program, facts.view(), state, outgoing.returned_value)
            {
                sites
                    .sites
                    .push((slot, plan_index, returned_at, completed_value));
            }
        }
        sites.plans.push((plan, output_projections));
    }
    for position in 0..sites.sites.len() {
        let completed_value = sites.sites[position].3;
        match sites
            .groups
            .iter()
            .position(|group| sites.sites[group[0]].3 == completed_value)
        {
            Some(group) => sites.groups[group].push(position),
            None => sites.groups.push(vec![position]),
        }
    }
    sites
}

/// Declared plans and exact terminal callers' demands are proof obligations,
/// never assumptions about the value a body returns.
fn state_conservation_plans(
    program: &dyn validation::CheckingInput,
    facts: checked_trees::CheckFactsRef<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Vec<(usize, language_semantics::content::ContentConservationPlan)> {
    let entry = program
        .machine_states(machine)
        .first()
        .map(|state| state.symbol);
    let mut plans = facts
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
    for (plan_index, plan) in facts
        .qualifications
        .content
        .conservation_plans
        .iter()
        .enumerate()
        .filter(|(_, plan)| plan.owner_kind == ContentConservationOwnerKind::Machine)
    {
        for demanded in terminal_demands::at_state(program, facts, plan, machine, state) {
            if !plans.iter().any(|(existing_index, existing)| {
                *existing_index == plan_index && *existing == demanded
            }) {
                plans.push((plan_index, demanded));
            }
        }
    }
    plans
}

/// Whether the value completed at `site` proves `domain` at `path`, in the
/// same completed frame as its conservation equation.
fn proves_domain(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    (statement_index, expression): (usize, typed_trees::expression::ExpressionHandle),
    domain: symbols::SymbolHandle,
    path: &[facts::PlaceSegment],
) -> bool {
    validation::completed_value_proves_domain_predicate(
        program,
        machine,
        state,
        statement_index,
        expression,
        domain,
        typed_trees::expression::ExpressionHandle::invalid(),
        path,
    )
}

fn has_conservation_candidate(
    program: &dyn validation::CheckingInput,
    facts: checked_trees::CheckFactsRef<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    completed_value: arena::Handle<checked_trees::FlowOperandFact>,
    plan: &language_semantics::content::ContentConservationPlan,
) -> bool {
    let captured = facts.flow.control.operands.get(completed_value);
    validation::completed_value_has_content_conservation_candidate(
        program,
        machine,
        state,
        captured.statement_index,
        captured.expression,
        captured.type_reference,
        plan,
        facts.qualifications.content.plans,
    )
}

/// A machine guarantee may be consumed in another state only after the shared
/// arrival analysis proves the exact parameter origin on every predecessor.
/// Rebase its subjects, never import the guarantee as an entry assumption.
fn plan_at_state(
    program: &dyn validation::CheckingInput,
    facts: checked_trees::CheckFactsRef<'_>,
    state: &typed_trees::state::State,
    mut plan: language_semantics::content::ContentConservationPlan,
) -> Option<language_semantics::content::ContentConservationPlan> {
    if plan.callable == state.symbol {
        return Some(plan);
    }
    let flow = facts.flow.control.state(plan.owner, state.symbol)?;
    let origins = state_exit_parameter_origins(facts, flow);
    fn rebase(
        term: &mut ConservationTerm,
        origins: &[&checked_trees::FlowExitParameterOrigin],
        parameters: &[typed_trees::signature::StateParameter],
    ) -> Option<()> {
        match term {
            ConservationTerm::Separate(children) => {
                for child in children {
                    rebase(child, origins, parameters)?;
                }
            }
            ConservationTerm::Projection { subject, .. } => {
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
    facts: checked_trees::CheckFactsRef<'_>,
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

fn side_has_role(term: &ConservationTerm, entry: bool) -> bool {
    match term {
        ConservationTerm::Projection { subject, .. } => {
            if entry {
                subject.version == ContentPlaceVersion::Entry
                    && matches!(subject.root, ContentPlaceRoot::Parameter { .. })
            } else {
                subject.version == ContentPlaceVersion::Current
                    && subject.root == ContentPlaceRoot::Result
            }
        }
        ConservationTerm::Separate(children) => {
            !children.is_empty() && children.iter().all(|child| side_has_role(child, entry))
        }
    }
}

/// Equation canonicalization may reverse the authored sides. Classify by
/// exact root/revision, not spelling or serialized side order. Empty separate
/// terms provide no owned input or returned occurrence.
pub(super) fn owned_return_sides<'term>(
    left: &'term ConservationTerm,
    right: &'term ConservationTerm,
) -> Option<(&'term ConservationTerm, &'term ConservationTerm)> {
    if side_has_role(left, true) && side_has_role(right, false) {
        Some((left, right))
    } else if side_has_role(right, true) && side_has_role(left, false) {
        Some((right, left))
    } else {
        None
    }
}
