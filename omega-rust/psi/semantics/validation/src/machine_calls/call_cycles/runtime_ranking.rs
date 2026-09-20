//! Runtime call components share an authored ranking. Nonincreasing edges may
//! forward that rank, but must form a DAG: every complete cycle then contains
//! a strict decrease. No source subject or public termination claim is added.

mod comparison;
mod meaning;
mod prefix;
mod projection;

#[cfg(test)]
mod tests;

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

use crate::proof_contracts::contract_entailment::{
    RankingRangeCallEdge, RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
    RankingRangeMeasure, RankingRangePremises, call_member_premise_symbols,
    discover_state_entry_mappings_preferring, mixed_call_endpoints_are_pinned,
    positive_step_amount, prove_ranking_range_call, prove_ranking_range_call_entry,
    ranking_range_required_symbols,
};
use comparison::Comparison;
use projection::{RankOrder, RankProjection};

pub(super) fn extend_runtime_adjacency(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    adjacency: &mut [Vec<usize>],
) {
    for (index, machine) in program.machines().iter().enumerate() {
        if proof_only.is_proof_machine(program, machine) {
            continue;
        }
        let mut targets = Vec::new();
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if !matches!(statement, StatementNode::AssemblyFact(_)) {
                    super::collect_statement_dependency_symbols(program, statement, &mut targets);
                }
            }
        }
        for target in targets {
            if let Some(callee) = target_machine(program, target)
                && callee != index
            {
                adjacency[index].push(callee);
            }
        }
        adjacency[index].sort_unstable();
        adjacency[index].dedup();
    }
}

pub(super) fn admitted_components(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    adjacency: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    super::strongly_connected_components(adjacency)
        .into_iter()
        .filter(|component| {
            component.len() > 1
                && component
                    .iter()
                    .all(|index| !proof_only.is_proof_machine(program, &program.machines()[*index]))
                && check_component(program, adjacency, component).is_ok()
        })
        .collect()
}

pub(super) fn check_component(
    program: &TypedTrees,
    adjacency: &[Vec<usize>],
    component: &[usize],
) -> Result<(), &'static str> {
    let Some(ranks) = component
        .iter()
        .map(|index| RankProjection::resolve(program, &program.machines()[*index]))
        .collect::<Option<Vec<_>>>()
    else {
        return Err("a member lacks a supported exact ranking witness");
    };
    if !ranks.iter().all(|rank| rank.same_order(&ranks[0])) {
        return Err("members do not share the same ranking order");
    }
    let mixed_ranges = ranks.iter().any(|rank| rank.range.is_valid())
        && ranks.iter().any(|rank| !rank.range.is_valid());
    for (rank, index) in ranks.iter().zip(component) {
        if rank.range.is_valid() {
            let machine = &program.machines()[*index];
            if !prove_ranking_range_call_entry(
                program,
                RankingRangeCallMember {
                    machine,
                    subject: rank.subject,
                    paired_subject: rank.paired_subject,
                    range: rank.range,
                },
            ) {
                return Err("a member's initial rank range is unproven");
            }
        }
    }
    let frames = crate::machine_calls::calls::CallFrameResolver::new(program);
    // Internal arrivals are owned by the member's own ranking judgment,
    // which the checked stage applies to every member whose local state
    // graph can still cycle and to every ranged member with internal
    // arrivals. A call issued from a subordinate state still answers to
    // this shared hypothesis, and it reads the member's authored range as
    // that arrival's proven invariant: the site query installs the same
    // membership facts the member's state-edge judgment re-establishes at
    // each arrival, never a requires fact (entry-site evidence only) and
    // never a destination requirement. The member's discovered
    // telescope names the entry role each site formal carries, so
    // authored subjects and endpoints normalize to the atom the site
    // actually holds. The telescope must read the same correspondence the
    // member's own state-edge judgment used, or a call hypothesis could
    // name a different value than that witness proved. Every ranked
    // subject is a preferred carrier -- a bounded distance ranks both --
    // while a slice slot selects its sole collection dependency without a
    // preference. Discovery keeps a contested claim only for a bare
    // forward, plus a computed claimant on an entry the member's own
    // arrival judgment holds equal: ranged members, under the
    // rank-invariant premise set both premise modes share (an entry
    // invariant attempt protects a superset). An unranged member's
    // computed claimants stay role-less rather than borrowing equality
    // evidence no judgment ran for. A record-subject struct view also
    // names its fresh-record carrier: a dependency-free literal arrival
    // into the unique record-typed slot still claims the role the field
    // coordinate reads.
    let member_discovery = component
        .iter()
        .enumerate()
        .map(|(position, index)| {
            let machine = &program.machines()[*index];
            let rank = &ranks[position];
            let preferred = match &rank.order {
                RankOrder::BoundedDistance(_) => [rank.parameter, rank.paired_parameter]
                    .into_iter()
                    .filter(|symbol| symbol.is_valid())
                    .collect::<Vec<_>>(),
                // A bare slice slot prefers no role; a member-chain slice
                // subject is read off the record formal carrying it, so that
                // formal is the preferred carrier.
                RankOrder::SliceLength => [rank.record_subject]
                    .into_iter()
                    .filter(|symbol| symbol.is_valid())
                    .collect(),
                _ => [rank.parameter]
                    .into_iter()
                    .filter(|symbol| symbol.is_valid())
                    .collect(),
            };
            let record_subject = rank.record_subject;
            let required = if rank.range.is_valid() {
                rank.measure.and_then(|measure| {
                    ranking_range_required_symbols(
                        program,
                        machine,
                        rank.range,
                        measure,
                        RankingRangePremises::RankInvariant,
                    )
                })
            } else {
                Some(Vec::new())
            }?;
            let mappings = discover_state_entry_mappings_preferring(
                program,
                machine,
                &preferred,
                record_subject,
                &required,
            )?;
            Some((mappings, required))
        })
        .collect::<Option<Vec<_>>>();
    let Some(member_discovery) = member_discovery else {
        return Err("the ranking needs entry-to-state arrival evidence");
    };
    // A discovered role records DEPENDENCY, not equality: a slot that claimed
    // an entry role through a computed actual denotes a derived value, not
    // the value the role carried at invocation. Binding the authored subject
    // or an endpoint to that slot would let an internal `hold(remaining + 1)`
    // inflate the site carrier beyond the entry rank, so `step(carried - 1)`
    // reads as a strict edge while the component cycle preserves the rank.
    // Consumption therefore keeps a role only when every arrival preserves
    // it: a bare forward of a formal still carrying the role, a
    // direction-correct `carrier - positive`/`carrier + positive` descent for
    // a subject role, or -- for a ranged member's required carriers -- a
    // computed claimant the member's own arrival judgment proved equal to a
    // bare-anchored sibling.
    let mut member_mappings = Vec::with_capacity(member_discovery.len());
    for (position, index) in component.iter().enumerate() {
        let (mappings, required) = &member_discovery[position];
        let bounds = carrier_bounds(
            program,
            &program.machines()[*index],
            mappings,
            &ranks[position],
        );
        let mut filtered = mappings.clone();
        for (state_position, slots) in filtered.iter_mut().enumerate() {
            // Required carriers keep a computed claimant only while a sibling
            // slot still forwards the role bare: the member's own ranged
            // arrival judgment proves such copies equal, so every claim then
            // names the anchor's value.
            let anchored = slots
                .iter()
                .zip(&bounds[state_position])
                .filter(|(claimed, bound)| {
                    claimed.is_valid()
                        && required.contains(claimed)
                        && **bound == CarrierBound::Equal
                })
                .map(|(claimed, _)| *claimed)
                .collect::<Vec<_>>();
            for (slot, role) in slots.iter_mut().enumerate() {
                if !role.is_valid() {
                    continue;
                }
                let kept = bound_allows(&ranks[position], *role, bounds[state_position][slot])
                    || anchored.contains(role);
                if !kept {
                    *role = SymbolHandle::default();
                }
            }
        }
        member_mappings.push(filtered);
    }
    // A prefix store is judged against the premise carriers the call-site
    // judgment actually reads (subjects, endpoints, requires facts, and
    // constrained entries), located in each state through the same telescope,
    // plus the slots whose arrival value the site query or the mixed-range
    // endpoint conservation assumes (see `prefix::preserves_rank`). A member
    // whose witness resolves no scalar premise set keeps every store
    // rejected; the reader fails closed on `None`.
    let member_premises = ranks
        .iter()
        .zip(component)
        .map(|(rank, index)| {
            call_member_premise_symbols(
                program,
                &RankingRangeCallMember {
                    machine: &program.machines()[*index],
                    subject: rank.subject,
                    paired_subject: rank.paired_subject,
                    range: rank.range,
                },
            )
        })
        .collect::<Vec<_>>();
    let mut range_edges = Vec::new();
    let mut weak_edges = vec![Vec::new(); component.len()];
    for (position, index) in component.iter().copied().enumerate() {
        let machine = &program.machines()[index];
        let states = program.machine_states(machine);
        if states.is_empty() {
            return Err("a member has no checked entry state");
        }
        let mappings = &member_mappings[position];
        let mut observed = Vec::new();
        for (state_position, state) in states.iter().enumerate() {
            let mut guards = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::Transition(transition) = statement else {
                    if prefix::preserves_rank(
                        program,
                        machine,
                        state,
                        &ranks[position],
                        member_premises[position].as_deref(),
                        &mappings[state_position],
                        mixed_ranges,
                        statement,
                        frames.as_ref(),
                    ) {
                        continue;
                    }
                    match statement {
                        StatementNode::AssemblyFact(_) => continue,
                        StatementNode::RootBinding(binding)
                            if [binding.receiver, binding.implementation_operand]
                                .into_iter()
                                .all(|expression| {
                                    !expression.is_valid()
                                        || expression_is_inert(program, machine, state, expression)
                                }) =>
                        {
                            continue;
                        }
                        StatementNode::LocalData(local)
                            if expression_is_inert(
                                program,
                                machine,
                                state,
                                local.initial_value,
                            ) =>
                        {
                            continue;
                        }
                        StatementNode::Expression(expression)
                            if expression_is_inert(program, machine, state, *expression) =>
                        {
                            continue;
                        }
                        // Do not replay an entry-relative projection or a guard
                        // across writes, calls, aliases, or unknown effects.
                        _ => {
                            return Err(
                                "a write, call, or alias invalidates the entry-relative ranking",
                            );
                        }
                    }
                };
                if transition.continuation.is_valid() {
                    return Err("a non-tail call edge returns into a continuation");
                }
                let guard = match transition.guard {
                    TransitionGuardNode::Always => ExpressionHandle::invalid(),
                    TransitionGuardNode::When(guard)
                        if expression_is_inert(program, machine, state, guard) =>
                    {
                        guard
                    }
                    TransitionGuardNode::When(_) => {
                        return Err("a guard has effects or non-builtin operator meaning");
                    }
                };
                let mut site_guards = guards.clone();
                if guard.is_valid() {
                    site_guards.push((guard, true));
                    guards.push((guard, false));
                }
                match program.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        let Some(callee) = target_machine(program, path.symbol) else {
                            return Err("a call target has no exact machine identity");
                        };
                        if callee == index {
                            // An internal state arrival belongs to the
                            // member's own ranking judgment -- the checked
                            // stage runs that judgment whenever the member's
                            // local state graph can cycle or a ranged member
                            // has internal arrivals -- so it adds no
                            // hypothesis edge here.
                            continue;
                        }
                        let callee_machine = &program.machines()[callee];
                        let Some(callee_entry) = program.machine_states(callee_machine).first()
                        else {
                            return Err("a call target has no checked entry binding");
                        };
                        if path.symbol != callee_machine.symbol
                            && path.symbol != callee_entry.symbol
                        {
                            return Err("a subordinate-state call needs its own arrival evidence");
                        }
                        let arguments = program.statement_table.expression_handles(*arguments);
                        if !arguments
                            .iter()
                            .all(|argument| expression_is_inert(program, machine, state, *argument))
                        {
                            return Err(
                                "a call argument has effects or non-builtin operator meaning",
                            );
                        }
                        observed.push(callee);
                        let Some(callee_position) =
                            component.iter().position(|index| *index == callee)
                        else {
                            continue;
                        };
                        let parameters = program.state_parameters(callee_entry);
                        if arguments.len()
                            != parameters
                                .iter()
                                .filter(|parameter| !parameter.is_self)
                                .count()
                        {
                            return Err("call arguments do not match the exact entry parameters");
                        }
                        let Some(argument) =
                            arguments.get(ranks[callee_position].argument_position)
                        else {
                            return Err(
                                "the ranked parameter has no corresponding actual argument",
                            );
                        };
                        if matches!(
                            ranks[position].order,
                            projection::RankOrder::Natural(_)
                                | projection::RankOrder::IncreasingTo(_)
                                | projection::RankOrder::BoundedDistance(_)
                                | projection::RankOrder::SliceLength
                                | projection::RankOrder::DeclaredIdentity { .. }
                                | projection::RankOrder::DeclaredComputation { .. }
                                | projection::RankOrder::CustomStructView { .. }
                        ) {
                            match prove_ranking_range_call(
                                program,
                                RankingRangeCallMember {
                                    machine,
                                    subject: ranks[position].subject,
                                    paired_subject: ranks[position].paired_subject,
                                    range: ranks[position].range,
                                },
                                &RankingRangeCallSite {
                                    state,
                                    entry_parameters: &mappings[state_position],
                                },
                                RankingRangeCallMember {
                                    machine: callee_machine,
                                    subject: ranks[callee_position].subject,
                                    paired_subject: ranks[callee_position].paired_subject,
                                    range: ranks[callee_position].range,
                                },
                                &site_guards,
                                arguments,
                            ) {
                                Some(RankingRangeCallProgress::Strict) => {}
                                Some(RankingRangeCallProgress::NonIncreasing) => {
                                    weak_edges[position].push(callee_position)
                                }
                                None => {
                                    return Err(
                                        "rank range membership, pinned endpoints, or nonincrease is unproven at a call site",
                                    );
                                }
                            }
                        } else {
                            // A lexicographic member carries no scalar range
                            // judgment, but its subordinate-site formals still
                            // telescope to their discovered entry roles: the
                            // comparison reads the carried copy as the authored
                            // subject it denotes at this arrival.
                            match comparison::argument_comparison(
                                program,
                                &ranks[position],
                                *argument,
                                &site_guards,
                                &RankingRangeCallSite {
                                    state,
                                    entry_parameters: &mappings[state_position],
                                },
                            ) {
                                Some(Comparison::Strict) => {}
                                Some(Comparison::Equal) => {
                                    weak_edges[position].push(callee_position)
                                }
                                None => {
                                    return Err(
                                        "ranking preservation or strict DECREASE is unproven at a call site",
                                    );
                                }
                            }
                        }
                        if mixed_ranges {
                            // Conservation reads the caller's entry values
                            // through this exact site's telescope, including
                            // endpoints whose carrier is a subordinate formal.
                            range_edges.push(RankingRangeCallEdge {
                                source: position,
                                destination: callee_position,
                                site_state: state,
                                entry_parameters: &member_mappings[position][state_position],
                                arguments,
                                guards: site_guards,
                            });
                        }
                    }
                    TransitionTargetNode::Value(value)
                        if expression_is_inert(program, machine, state, *value) => {}
                    TransitionTargetNode::Terminal => {}
                    TransitionTargetNode::SelfTarget => {}
                    _ => {
                        return Err("a non-tail call or unknown effect prevents ranking admission");
                    }
                }
            }
        }
        // A pair-level strict occurrence cannot hide an unclassified call,
        // and a legacy spelling edge cannot supply missing exact custody.
        if adjacency[index]
            .iter()
            .filter(|target| component.contains(target))
            .any(|target| !observed.contains(target))
        {
            return Err("an internal call occurrence lacks a classified tail edge");
        }
    }
    if mixed_ranges {
        let members = ranks
            .iter()
            .zip(component)
            .map(|(rank, index)| RankingRangeCallMember {
                machine: &program.machines()[*index],
                subject: rank.subject,
                paired_subject: rank.paired_subject,
                range: rank.range,
            })
            .collect::<Vec<_>>();
        if !mixed_call_endpoints_are_pinned(program, &members, &range_edges) {
            return Err(
                "dependent rank endpoints are not conserved through every mixed-component call",
            );
        }
    }
    // Weak edges may exist across every state of a member; their acyclicity
    // is still judged per machine position.
    if weak_edges_are_acyclic(&weak_edges) {
        Ok(())
    } else {
        Err("a preserving cycle has no strict measure DECREASE")
    }
}

fn weak_edges_are_acyclic(adjacency: &[Vec<usize>]) -> bool {
    super::strongly_connected_components(adjacency)
        .iter()
        .all(|component| component.len() == 1 && !adjacency[component[0]].contains(&component[0]))
}

/// The ranged member's consumable invariant at one internal call site: the
/// scalar measure its own ranking judgment selected, the authored range
/// handle, and the discovered telescope for `state`. The ordinary
/// callee-requirement check consumes the membership facts those inputs name
/// -- the same arrival invariant the component-edge judgment installs for a
/// call the coordinator checks -- never a requires fact, which stays
/// entry-site evidence. `None` at the entry state (entry facts already
/// apply there), when the member authors no supported scalar range, or when
/// the two premise modes the member's own state-edge judgment may have run
/// under discover different correspondences: then no single telescope is
/// known proven at this site and the site abstains.
pub(crate) fn ranged_member_site_invariant(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &State,
) -> Option<(RankingRangeMeasure, ExpressionHandle, Vec<SymbolHandle>)> {
    let rank = RankProjection::resolve(program, machine)?;
    let measure = rank.measure?;
    if !rank.range.is_valid() {
        return None;
    }
    let states = program.machine_states(machine);
    let position = states
        .iter()
        .position(|candidate| candidate.symbol == state.symbol)?;
    if position == 0 {
        return None;
    }
    // The preferred carriers mirror `check_component`: the formals the
    // produced rank reads for this measure's order.
    let preferred = match &rank.order {
        RankOrder::BoundedDistance(_) => [rank.parameter, rank.paired_parameter]
            .into_iter()
            .filter(|symbol| symbol.is_valid())
            .collect::<Vec<_>>(),
        RankOrder::SliceLength => [rank.record_subject]
            .into_iter()
            .filter(|symbol| symbol.is_valid())
            .collect(),
        _ => [rank.parameter]
            .into_iter()
            .filter(|symbol| symbol.is_valid())
            .collect(),
    };
    let discover = |premises| {
        let required =
            ranking_range_required_symbols(program, machine, rank.range, measure, premises)?;
        discover_state_entry_mappings_preferring(
            program,
            machine,
            &preferred,
            rank.record_subject,
            &required,
        )
    };
    let mappings = discover(RankingRangePremises::RankInvariant)?;
    if discover(RankingRangePremises::EntryInvariant)? != mappings {
        return None;
    }
    Some((measure, rank.range, mappings.get(position)?.clone()))
}

/// How a slot's arrival value may relate to its recorded entry role while the
/// call judgment still consumes the claim. `Equal` carriers denote the role's
/// value exactly; `AtMost`/`AtLeast` carriers keep the site value bounded by
/// the invocation rank on the side the member's order reads -- a
/// `carrier - positive` step shrinks a descending subject, a
/// `carrier + positive` step grows an increasing one. The lattice meets the
/// bounds of every arrival into the slot.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CarrierBound {
    /// No readable bound: the slot's role cannot be consumed at a call site.
    None,
    AtMost,
    AtLeast,
    Equal,
}

impl CarrierBound {
    fn meet(self, other: CarrierBound) -> CarrierBound {
        use CarrierBound::*;
        match (self, other) {
            (None, _) | (_, None) => None,
            (Equal, bound) | (bound, Equal) => bound,
            (AtMost, AtMost) => AtMost,
            (AtLeast, AtLeast) => AtLeast,
            (AtMost, AtLeast) | (AtLeast, AtMost) => None,
        }
    }
}

/// Whether the computed bound lets the component judgment consume the slot's
/// recorded role. A premise carrier -- endpoints, requires inputs,
/// constrained copies -- must denote the role exactly; a subject carrier may
/// additionally arrive through the direction that order decreases.
fn bound_allows(rank: &RankProjection, role: SymbolHandle, bound: CarrierBound) -> bool {
    match bound {
        CarrierBound::Equal => true,
        CarrierBound::AtMost => carrier_direction(rank, role) == CarrierBound::AtMost,
        CarrierBound::AtLeast => carrier_direction(rank, role) == CarrierBound::AtLeast,
        CarrierBound::None => false,
    }
}

/// The direction a subject carrier may move while the call still transports a
/// non-increasing rank: a natural or produced-scalar rank decreases as its
/// subject does, an increasing-to rank decreases as its subject rises, and a
/// bounded distance decreases as its lower subject rises or its upper
/// subject falls. Every non-subject role stays `Equal`: a premise read
/// through a moved slot names a different fact.
fn carrier_direction(rank: &RankProjection, role: SymbolHandle) -> CarrierBound {
    if role == rank.parameter {
        // A record role names WHICH record the slot holds, never a moved
        // scalar: `carrier - positive` cannot typecheck on a record, and a
        // rebuilt literal still denotes the rank coordinate exactly. That
        // covers a field view's subject formal and a member-chain subject's
        // carrier formal alike.
        if rank.record_subject.is_valid() {
            return CarrierBound::Equal;
        }
        match &rank.order {
            RankOrder::IncreasingTo(_) | RankOrder::BoundedDistance(_) => CarrierBound::AtLeast,
            _ => CarrierBound::AtMost,
        }
    } else if role == rank.paired_parameter && matches!(rank.order, RankOrder::BoundedDistance(_)) {
        // A member-chain upper subject's record role is exact-or-nothing
        // like any other record carrier.
        if rank.paired_record_subject.is_valid() {
            CarrierBound::Equal
        } else {
            CarrierBound::AtMost
        }
    } else {
        CarrierBound::Equal
    }
}

/// Per-slot carrier bounds under the discovered telescope. Bounds shrink
/// along the internal-arrival fixpoint: a slot keeps a bound only while
/// EVERY arrival into it preserves the bound, so an unwitnessed shape fails
/// closed to `None` rather than borrowing an equality no judgment ran for.
fn carrier_bounds(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    mappings: &[Vec<SymbolHandle>],
    rank: &RankProjection,
) -> Vec<Vec<CarrierBound>> {
    let states = program.machine_states(machine);
    let mut bounds = mappings
        .iter()
        .map(|slots| vec![CarrierBound::Equal; slots.len()])
        .collect::<Vec<_>>();
    let arrivals = internal_arrivals(program, machine);
    loop {
        let previous = bounds.clone();
        for (source, destination, arguments) in &arrivals {
            if *destination == 0 {
                // The entry telescope is always the identity mapping; a
                // back-edge does not restate its own formals.
                continue;
            }
            let (Some(slots), Some(source_state), Some(source_slots)) = (
                mappings.get(*destination),
                states.get(*source),
                mappings.get(*source),
            ) else {
                continue;
            };
            for (slot, role) in slots.iter().enumerate() {
                if !role.is_valid() || bounds[*destination][slot] == CarrierBound::None {
                    continue;
                }
                let arrival = carrier_arrival_bound(
                    program,
                    machine,
                    source_state,
                    &bounds[*source],
                    source_slots,
                    arguments.get(slot).copied(),
                    *role,
                    rank,
                );
                bounds[*destination][slot] = bounds[*destination][slot].meet(arrival);
            }
        }
        if bounds == previous {
            return bounds;
        }
    }
}

/// One arrival's contribution to a slot's carrier bound: a bare forward of a
/// formal still carrying the role inherits that formal's bound, and a
/// `carrier - positive`/`carrier + positive` step keeps the matching side.
/// Any other shape -- arithmetic over several inputs, a projection, a
/// literal, an operand call -- names no bound this judgment can read.
/// A record-subject role under a custom struct view is exact-or-nothing:
/// scalar arithmetic cannot reach it, so a bare forward or borrow of a
/// role-carrying formal inherits, while a rebuilt literal or a record-typed
/// projection keeps `Equal` -- the member's own edge judgment already
/// proved that arrival's rank coordinate, and the slot's fresh atom is what
/// the site reads.
fn carrier_arrival_bound(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    source_bounds: &[CarrierBound],
    source_slots: &[SymbolHandle],
    argument: Option<ExpressionHandle>,
    role: SymbolHandle,
    rank: &RankProjection,
) -> CarrierBound {
    let Some(argument) = argument else {
        return CarrierBound::None;
    };
    // The source formal position mirrors the telescope's own lookup: non-self
    // slots in order, and a constant formal cannot carry a mutable role.
    let formal_bound = |symbol: SymbolHandle| {
        program
            .state_parameters(source)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .position(|parameter| parameter.symbol == symbol && !parameter.is_const)
            .filter(|position| source_slots[*position] == role)
            .and_then(|position| source_bounds.get(position).copied())
            .unwrap_or(CarrierBound::None)
    };
    let bare_name = |expression: ExpressionHandle| match program
        .expression_table
        .expression(projection::unwrapped(program, expression))
    {
        ExpressionNode::Name(name) if name.symbol.is_valid() && name.head_symbol == name.symbol => {
            Some(name.symbol)
        }
        _ => None,
    };
    // The collection a windowed arrival reads: borrows and member chains
    // resolve to their root formal, whose bound for this role decides what
    // the subslice inherits.
    let carrier_root = |mut expression: ExpressionHandle| loop {
        match program
            .expression_table
            .expression(projection::unwrapped(program, expression))
        {
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Name(name)
                if name.symbol.is_valid() && name.head_symbol == name.symbol =>
            {
                break Some(name.symbol);
            }
            _ => break None,
        }
    };
    // A record role tracks WHICH record the slot holds, not a scalar bound.
    // Every arrival shape the member's own edge judgment can prove for a
    // projected coordinate -- a forward of the slot still carrying the role,
    // a borrow of one, a rebuild literal, or a prefix projection of the
    // coordinate's chain -- keeps the slot's claim exact. An operand call or
    // an index read is a record only through a shape no coordinate walks, so
    // it strips the role as always. The same arm covers a field view's
    // subject formal, a member-chain subject's carrier formal, and a
    // bounded distance's member-chain upper formal.
    let record_role = (role == rank.parameter && rank.record_subject.is_valid())
        || (role == rank.paired_parameter && rank.paired_record_subject.is_valid());
    if record_role {
        return match program
            .expression_table
            .expression(projection::unwrapped(program, argument))
        {
            ExpressionNode::Name(name)
                if name.symbol.is_valid() && name.head_symbol == name.symbol =>
            {
                formal_bound(name.symbol)
            }
            ExpressionNode::Borrow(borrow) => carrier_arrival_bound(
                program,
                machine,
                source,
                source_bounds,
                source_slots,
                Some(borrow.target),
                role,
                rank,
            ),
            ExpressionNode::StructLiteral(_) | ExpressionNode::Member(_) => CarrierBound::Equal,
            _ => CarrierBound::None,
        };
    }
    match program
        .expression_table
        .expression(projection::unwrapped(program, argument))
    {
        ExpressionNode::Name(name) if name.symbol.is_valid() && name.head_symbol == name.symbol => {
            formal_bound(name.symbol)
        }
        ExpressionNode::Binary(binary) => {
            let Some(symbol) = bare_name(binary.left) else {
                return CarrierBound::None;
            };
            // The step amount is the strict-step half the telescope names: a
            // positive literal or a formal whose declared range proves it
            // nonzero. A zero-able operand is not divergence evidence.
            let positive = positive_step_amount(program, source, binary.right);
            let bound = formal_bound(symbol);
            match (binary.operator, positive) {
                (typed_trees::expression::BinaryOperator::Subtract, true) => {
                    bound.meet(CarrierBound::AtMost)
                }
                (typed_trees::expression::BinaryOperator::Add, true) => {
                    bound.meet(CarrierBound::AtLeast)
                }
                // `carrier + 0`/`carrier - 0` spell a computed claimant, not a
                // bare forward: only a ranged member's own arrival judgment
                // proves that copy equal to a required carrier, so it keeps
                // the role through the anchored rule -- never by shape alone.
                _ => CarrierBound::None,
            }
        }
        // A window's produced length never exceeds its collection's: a
        // builtin subslice into a slice carrier weakens the collection's
        // bound to `AtMost`, matching the direction the ranked copy moved.
        // The member's own edge judgment still proved the arrival's exact
        // length before this bound can carry the role to a call site.
        ExpressionNode::Indexed(indexed)
            if crate::value_custody::places::has_builtin_subslice_meaning(
                program,
                machine,
                Some(source),
                projection::unwrapped(program, argument),
            ) =>
        {
            carrier_root(indexed.collection).map_or(CarrierBound::None, |symbol| {
                formal_bound(symbol).meet(CarrierBound::AtMost)
            })
        }
        _ => CarrierBound::None,
    }
}

/// Every authored internal arrival: `(source index, destination index,
/// actuals)` for each named transition target inside the machine and each
/// implicit self transition. This mirrors the telescope's occurrence set
/// exactly: a role consumed through an arrival this walk did not see would
/// name a value the member's own judgment never established.
fn internal_arrivals<'program>(
    program: &'program TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<(usize, usize, &'program [ExpressionHandle])> {
    let states = program.machine_states(machine);
    let mut arrivals = Vec::new();
    for (source, state) in states.iter().enumerate() {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        if let Some(destination) =
                            internal_state_index(program, machine, path.symbol)
                        {
                            arrivals.push((
                                source,
                                destination,
                                program.statement_table.expression_handles(*arguments),
                            ));
                        }
                    }
                    TransitionTargetNode::SelfTarget => {
                        arrivals.push((source, source, &[]));
                    }
                    TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    arrivals
}

/// Resolve a named internal transition target to its state index inside
/// `machine`, exactly as the telescope's discovery does: a transition back to
/// the machine's declared entry names the machine symbol, while transitions
/// to subordinate states name the state symbol.
fn internal_state_index(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    target_symbol: SymbolHandle,
) -> Option<usize> {
    if target_symbol == machine.symbol {
        let entry_name = machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or_default();
        return program
            .machine_states(machine)
            .iter()
            .position(|state| state.name.as_str() == entry_name)
            .or_else(|| (!program.machine_states(machine).is_empty()).then_some(0));
    }
    program
        .machine_states(machine)
        .iter()
        .position(|state| state.symbol == target_symbol)
}

fn target_machine(program: &TypedTrees, symbol: SymbolHandle) -> Option<usize> {
    if !symbol.is_valid() {
        return None;
    }
    program.machines().iter().position(|machine| {
        machine.symbol == symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == symbol)
    })
}

fn expression_is_inert(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return true;
    }
    let inert = |expression| expression_is_inert(program, machine, state, expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            inert(dispatch.subject)
                && program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .all(|arm| {
                        let pattern_inert = match arm.pattern {
                            typed_trees::expression::MatchPattern::Value(pattern) => inert(pattern),
                            typed_trees::expression::MatchPattern::Wildcard => true,
                        };
                        pattern_inert && inert(arm.value)
                    })
        }
        ExpressionNode::Atomic(atomic) => inert(atomic.value),
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
        ExpressionNode::Member(member) => inert(member.receiver),
        ExpressionNode::Binary(binary) => {
            meaning::binary_is_builtin(program, machine.symbol, expression, binary)
                && inert(binary.left)
                && inert(binary.right)
        }
        ExpressionNode::Unary(unary) => inert(unary.operand),
        ExpressionNode::Cast(cast) => inert(cast.value),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| inert(field.value)),
        ExpressionNode::ArrayLiteral(items) => program
            .expression_table
            .expression_handles(*items)
            .iter()
            .all(|item| inert(*item)),
        // A builtin indexed read or subslice is a pure projection of its
        // collection. Only an authored or selected index operation could hide
        // an effect here, and its scalar bounds traverse the same inert check;
        // bound meaning deliberately treats calls as symbolic leaves, so the
        // operands still need this structural walk.
        ExpressionNode::Indexed(indexed)
            if crate::value_custody::places::has_builtin_subslice_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) || crate::value_custody::places::place_has_builtin_coordinates(
                program,
                machine,
                Some(state),
                expression,
            ) =>
        {
            let endpoints_inert = match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => [range.start, range.end]
                    .into_iter()
                    .all(|endpoint| !endpoint.is_valid() || inert(endpoint)),
                _ => inert(indexed.index),
            };
            inert(indexed.collection) && endpoints_inert
        }
        // Forming a reference writes nothing of its own; a later store through
        // the alias is a separate statement whose frame is closed over the
        // referent's origins by the write-frame owner. A call or an
        // unadmitted indexed operation can have selected behavior this pure
        // rank slice cannot see.
        ExpressionNode::Borrow(borrow) => inert(borrow.target),
        ExpressionNode::Call(_) | ExpressionNode::Indexed(_) | ExpressionNode::Range(_) => false,
    }
}
