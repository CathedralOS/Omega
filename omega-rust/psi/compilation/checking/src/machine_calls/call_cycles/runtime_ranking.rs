//! Runtime call components share an authored ranking. Nonincreasing edges may
//! forward that rank, but must form a DAG: every complete cycle then contains
//! a strict decrease. Source-complete candidate members constrain the search
//! prefix; only a complete prefix supplies a publishable component decision.
//! No source subject or public termination claim is added.

mod comparison;
mod meaning;
mod prefix;
mod projection;

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::signature::SignatureContractKind;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

use crate::proof_contracts::contract_entailment::{
    RankingRangeCallEdge, RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
    RankingRangeMeasure, call_member_premise_symbols, mixed_call_endpoints_are_pinned,
    prove_ranking_range_call,
};
use comparison::Comparison;
use projection::RankProjection;

/// Exact scalar coordinates from the runtime component judgment. These are
/// transient query results, not another executable representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuntimeNaturalState {
    pub state: SymbolHandle,
    pub parameter: SymbolHandle,
    pub coordinate: crate::RankSubjectCoordinate,
    pub parameter_position: u32,
    pub primitive_type: typed_trees::types::PrimitiveType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuntimeNaturalEdge {
    pub source: SymbolHandle,
    pub target: SymbolHandle,
    pub statement_ordinal: u32,
    pub strict: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuntimeNaturalComponent {
    pub states: Vec<ValidatedRuntimeNaturalState>,
    pub edges: Vec<ValidatedRuntimeNaturalEdge>,
}

pub(super) fn extend_runtime_adjacency(
    program: &dyn crate::CheckingInput,
    proof_only: &ProofOnlyClassification,
    adjacency: &mut [Vec<usize>],
) -> Vec<usize> {
    let dispatched = dispatched_implementations(program);
    let mut selector_self_recursion = Vec::new();
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
            if let Some((_, implementations)) = dispatched.iter().find(|(slot, _)| *slot == target)
            {
                // A selector call reaches whichever implementation is
                // selected, including the caller itself. Retain dispatch
                // provenance: direct-call ranking cannot discharge selection.
                adjacency[index].extend(implementations.iter().copied());
                if implementations.contains(&index) && !selector_self_recursion.contains(&index) {
                    selector_self_recursion.push(index);
                }
            } else if let Some(callee) = target_machine(program, target)
                && (callee != index
                    || target == machine.symbol
                    || program
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| target == entry.symbol))
            {
                adjacency[index].push(callee);
            }
        }
        adjacency[index].sort_unstable();
        adjacency[index].dedup();
    }
    selector_self_recursion
}

/// Every implementation machine a selector call through each slot can
/// enter: the rows, for the slot's requirement, of every implementation
/// selected for the slot's interface. A selector names its table at runtime,
/// so termination must treat the call as an edge to each of them.
fn dispatched_implementations(
    program: &dyn crate::CheckingInput,
) -> Vec<(SymbolHandle, Vec<usize>)> {
    if program.dynamic_slots.is_empty() {
        return Vec::new();
    }
    let selections = crate::collect_dynamic_implementation_selections(program).unwrap_or_default();
    program
        .dynamic_slots
        .iter()
        .map(|slot| {
            let mut implementations = selections
                .iter()
                .filter(|selection| {
                    program.normalized_type_identity(selection.selector_type)
                        == slot.interface_identity
                })
                .filter_map(|selection| program.conformance_by_symbol(selection.implementation))
                .flat_map(|conformance| {
                    conformance
                        .rows
                        .iter()
                        .filter(|row| row.requirement == slot.requirement)
                        .filter_map(|row| target_machine(program, row.realization_machine))
                })
                .collect::<Vec<_>>();
            implementations.sort_unstable();
            implementations.dedup();
            (slot.symbol, implementations)
        })
        .collect()
}

/// A source-complete candidate for one exact authored body. These borrowed
/// readings are search scratch; only the component's complete decision can
/// publish them. Missing members supply no inductive ranking premise.
pub struct RuntimeRankBodyWitness<'selection> {
    pub machine: SymbolHandle,
    pub body: arena::HandleSpan<State>,
    pub states: &'selection [crate::RankStateBindings],
    pub edges: &'selection [checked_trees::CheckedRankArrivalEdge],
    pub entry_rank_formed: bool,
}

pub(super) fn project_component(
    program: &dyn crate::CheckingInput,
    adjacency: &[Vec<usize>],
    component: &[usize],
    witnesses: &[RuntimeRankBodyWitness<'_>],
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Result<Option<ValidatedRuntimeNaturalComponent>, &'static str> {
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
    let body_witnesses = component
        .iter()
        .map(|index| {
            let machine = &program.machines()[*index];
            let mut matches = witnesses.iter().filter(|witness| {
                witness.machine == machine.symbol && witness.body == machine.states
            });
            let witness = matches.next();
            if matches.next().is_some() {
                return Err("the component has duplicate source body candidates");
            }
            Ok(witness)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let complete = body_witnesses.iter().all(Option::is_some);
    for (rank, witness) in ranks.iter().zip(&body_witnesses) {
        if rank.range.is_valid() && witness.is_some_and(|witness| !witness.entry_rank_formed) {
            return Err("a member's initial rank range is unproven");
        }
    }
    let member_mappings = component
        .iter()
        .zip(&body_witnesses)
        .map(|(index, witness)| {
            let Some(witness) = witness else {
                return Ok(None);
            };
            let machine = &program.machines()[*index];
            let rows = program
                .machine_states(machine)
                .iter()
                .map(|state| {
                    let mut matching = witness
                        .states
                        .iter()
                        .filter(|row| row.state == state.symbol);
                    let row = matching.next()?;
                    matching.next().is_none().then(|| row.clone())
                })
                .collect::<Option<Vec<_>>>()
                .ok_or("the ranking needs complete entry-to-state arrival evidence")?;
            Ok(Some(rows))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ranked_machines = component
        .iter()
        .map(|position| program.machines()[*position].symbol)
        .collect::<Vec<_>>();
    let relevance = super::rank_relevance(program, &ranked_machines)
        .ok_or("the ranking needs exact state recurrence topology")?;
    // A local hop inside a recursive call component owes nonincrease even
    // when it is acyclic in its body's graph. Read the exact arithmetic result
    // already retained at admission, never lineage bounds or step syntax.
    for (index, witness) in component.iter().zip(&body_witnesses) {
        if let Some(witness) = witness
            && !super::rank_components::internal_rank_nonincrease(
                program,
                &program.machines()[*index],
                witness.edges,
            )
        {
            return Err("an internal arrival lacks its source component nonincrease result");
        }
    }
    let mut natural = if complete {
        let mappings = member_mappings
            .iter()
            .map(|rows| rows.as_ref().unwrap().clone())
            .collect::<Vec<_>>();
        projection::natural_component(program, component, &ranks, &mappings, &relevance)
    } else {
        None
    };
    // A prefix store is judged against the premise carriers the call-site
    // judgment actually reads (subjects, endpoints, requires facts, and
    // constrained entries), located in each state through the same telescope.
    // Mixed-range endpoint conservation separately checks the prefixes of all
    // component members against its demanded endpoint observations. A member
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
    // Complete cycles include the member's local arrivals as well as calls.
    // Contracting a whole body to one node would erase a strict local step
    // and wrongly reject its subsequent rank-preserving recursive invocation.
    let component_states = component
        .iter()
        .flat_map(|position| program.machine_states(&program.machines()[*position]))
        .map(|state| state.symbol)
        .collect::<Vec<_>>();
    let mut weak_edges = vec![Vec::new(); component_states.len()];
    for witness in body_witnesses.iter().flatten() {
        for edge in witness.edges {
            if edge.strictly_decreases || !relevance.requires_rank(edge.destination) {
                continue;
            }
            let source = component_states
                .iter()
                .position(|state| *state == edge.source)
                .ok_or("an internal rank edge has no component source")?;
            let destination = component_states
                .iter()
                .position(|state| *state == edge.destination)
                .ok_or("an internal rank edge has no component destination")?;
            weak_edges[source].push(destination);
        }
    }
    for (position, index) in component.iter().copied().enumerate() {
        let machine = &program.machines()[index];
        let states = program.machine_states(machine);
        if states.is_empty() {
            return Err("a member has no checked entry state");
        }
        let Some(mappings) = &member_mappings[position] else {
            continue;
        };
        let mut observed = Vec::new();
        for (state_position, state) in states.iter().enumerate() {
            let mut guards = Vec::new();
            // Internal jumps establish the destination state's own requires,
            // not the machine-entry contract. Carry those facts into the same
            // guarded call judgment used below, including its metadata binding.
            // Prefix preservation protects their exact current-state carriers:
            // a write cannot leave an arrival fact available as a live premise.
            if state_position != 0 {
                for contract in program.state_contracts(state) {
                    if contract.kind != SignatureContractKind::Requires {
                        continue;
                    }
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        if let Some(expression) = fact.as_expression() {
                            guards.push((*expression, true));
                        }
                    }
                }
            }
            // Whether every earlier statement in this state preserved the
            // entry-relative frame the call-site comparison reads.
            let mut entry_relative = true;
            for (statement_ordinal, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                if !relevance.requires_rank(state.symbol) {
                    continue;
                }
                let Some(transition) = statement.as_transition() else {
                    if prefix::preserves_rank(
                        program,
                        machine,
                        state,
                        member_premises[position].as_deref(),
                        &mappings[state_position],
                        statement,
                        frames,
                    ) {
                        continue;
                    }
                    match statement {
                        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => continue,
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
                        // A call-free direct store ends the entry-relative
                        // frame for every later judgment in this state. Only
                        // an edge the member's own current-value judgment
                        // already proved can still be admitted after it.
                        StatementNode::Assignment(assignment)
                            if expression_is_inert(program, machine, state, assignment.target)
                                && expression_is_inert(
                                    program,
                                    machine,
                                    state,
                                    assignment.value,
                                ) =>
                        {
                            entry_relative = false;
                            natural = None;
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
                        if prefix::operand_has_admitted_meaning(program, machine, state, guard) =>
                    {
                        guard
                    }
                    TransitionGuardNode::When(_) => {
                        return Err("a guard has effects or unproven operator meaning");
                    }
                };
                let mut site_guards = guards.clone();
                if !prefix::preserves_rank(
                    program,
                    machine,
                    state,
                    member_premises[position].as_deref(),
                    &mappings[state_position],
                    statement,
                    frames,
                ) {
                    return Err("call operands invalidate the exact entry-relative ranking frame");
                }
                if guard.is_valid() {
                    site_guards.push((guard, true));
                    guards.push((guard, false));
                }
                // `-> callee(..)` and `-> (callee(..))` denote the same tail
                // arrival. An ATTACHED machine must spell a foreign tail call
                // with the value-call parentheses, so a ranked component now
                // meets the parenthesized form where it used to meet the named
                // one. Read both as the same arrival instead of letting the
                // parenthesized spelling reach the unclassified-effect
                // rejection below; a call nested inside an operator or
                // aggregate is not a top-level Call node and still rejects.
                let target_node = program.statement_table.transition_target(transition.target);
                let tail_arrival = match target_node {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => Some((
                        path.symbol,
                        program.statement_table.expression_handles(*arguments),
                    )),
                    TransitionTargetNode::Value(value) => {
                        match program.expression_table.expression(*value) {
                            ExpressionNode::Call(call) if !call.receiver.is_valid() => Some((
                                call.target_symbol,
                                program.expression_table.expression_handles(call.arguments),
                            )),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                match tail_arrival {
                    Some((target_symbol, arguments)) => {
                        let Some(callee) = target_machine(program, target_symbol) else {
                            return Err("a call target has no exact machine identity");
                        };
                        if callee == index
                            && internal_state_index(program, machine, target_symbol).is_some()
                        {
                            if let Some(retained) = &mut natural
                                && !projection::retain_internal_transfer(
                                    program,
                                    retained,
                                    &relevance,
                                    state.symbol,
                                    target_symbol,
                                    transition.target,
                                    statement_ordinal,
                                    body_witnesses[position].unwrap().edges,
                                )
                            {
                                natural = None;
                            }
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
                        if target_symbol != callee_machine.symbol
                            && target_symbol != callee_entry.symbol
                        {
                            return Err("a subordinate-state call needs its own arrival evidence");
                        }
                        if !arguments.iter().all(|argument| {
                            prefix::operand_has_admitted_meaning(program, machine, state, *argument)
                        }) {
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
                        if arguments.len() != parameters.iter().count() {
                            return Err("call arguments do not match the exact entry parameters");
                        }
                        let Some(argument) =
                            arguments.get(ranks[callee_position].argument_position)
                        else {
                            return Err(
                                "the ranked parameter has no corresponding actual argument",
                            );
                        };
                        let strict;
                        if !entry_relative {
                            // The store before this arrival leaves no entry-
                            // relative operand to compare. A self-invocation
                            // keeps the strict decrease its body's source
                            // judgment proved over current values at this
                            // exact occurrence; nothing else is admitted.
                            let proved = callee == index
                                && body_witnesses[position].is_some_and(|witness| {
                                    witness.edges.iter().any(|edge| {
                                        edge.source == state.symbol
                                            && edge.destination == callee_entry.symbol
                                            && edge.target == transition.target
                                            && edge.statement_ordinal as usize == statement_ordinal
                                            && edge.strictly_decreases
                                    })
                                });
                            if !proved {
                                return Err(
                                    "a write, call, or alias invalidates the entry-relative ranking",
                                );
                            }
                            strict = true;
                        } else if matches!(
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
                                Some(RankingRangeCallProgress::Strict) => strict = true,
                                Some(RankingRangeCallProgress::NonIncreasing) => {
                                    strict = false;
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
                                machine.symbol,
                                &ranks[position],
                                *argument,
                                &site_guards,
                                &RankingRangeCallSite {
                                    state,
                                    entry_parameters: &mappings[state_position],
                                },
                            ) {
                                Some(Comparison::Strict) => strict = true,
                                Some(Comparison::Equal) => {
                                    strict = false;
                                }
                                None => {
                                    return Err(
                                        "ranking preservation or strict DECREASE is unproven at a call site",
                                    );
                                }
                            }
                        }
                        if !strict {
                            let source = component_states
                                .iter()
                                .position(|symbol| *symbol == state.symbol)
                                .ok_or("a call rank edge has no component source")?;
                            let destination = component_states
                                .iter()
                                .position(|symbol| *symbol == callee_entry.symbol)
                                .ok_or("a call rank edge has no component destination")?;
                            weak_edges[source].push(destination);
                        }
                        if let Some(retained) = &mut natural {
                            retained.edges.push(ValidatedRuntimeNaturalEdge {
                                source: state.symbol,
                                target: callee_entry.symbol,
                                statement_ordinal: u32::try_from(statement_ordinal)
                                    .map_err(|_| "a ranking statement ordinal exceeds u32")?,
                                strict,
                            });
                        }
                        if mixed_ranges {
                            // Conservation reads the caller's entry values
                            // through this exact site's telescope, including
                            // endpoints whose carrier is a subordinate formal.
                            range_edges.push(RankingRangeCallEdge {
                                source: position,
                                destination: callee_position,
                                site_state: state,
                                statement_ordinal,
                                arguments,
                                guards: site_guards,
                            });
                        }
                    }
                    None => match target_node {
                        TransitionTargetNode::Value(value)
                            if expression_is_inert(program, machine, state, *value) => {}
                        TransitionTargetNode::Terminal => {}
                        _ => {
                            return Err(
                                "a non-tail call or unknown effect prevents ranking admission",
                            );
                        }
                    },
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
    if complete && mixed_ranges {
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
        if !mixed_call_endpoints_are_pinned(program, &members, &range_edges, frames) {
            return Err(
                "dependent rank endpoints are not conserved through every mixed-component call",
            );
        }
    }
    // Removing all strict edges must leave an acyclic graph across exact
    // state arrivals and invocation entries. No complete weak cycle survives.
    if weak_edges_are_acyclic(&weak_edges) {
        // Scalar emission currently identifies authored delivery by the
        // source/target state pair. Keep mixed parallel comparisons out of
        // this projection until emitted delivery retains the call ordinal;
        // ordinary runtime component admission is unchanged.
        if natural.as_ref().is_some_and(|component| {
            component.edges.iter().enumerate().any(|(position, edge)| {
                component.edges[..position].iter().any(|earlier| {
                    earlier.source == edge.source
                        && earlier.target == edge.target
                        && earlier.strict != edge.strict
                })
            })
        }) {
            natural = None;
        }
        Ok(natural)
    } else {
        Err("a preserving cycle has no strict measure DECREASE")
    }
}

fn weak_edges_are_acyclic(adjacency: &[Vec<usize>]) -> bool {
    super::strongly_connected_components(adjacency)
        .iter()
        .all(|component| component.len() == 1 && !adjacency[component[0]].contains(&component[0]))
}

/// Consume the one source-admitted coordinate witness for this exact body.
/// Membership is an inductive fact only after every arrival has passed under
/// the retained premise regime; discovery alternatives cannot supply it.
pub(crate) fn ranged_member_site_invariant(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &State,
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> Option<(
    RankingRangeMeasure,
    ExpressionHandle,
    crate::RankStateBindings,
)> {
    let rank = RankProjection::resolve(program, machine)?;
    let measure = rank.measure?;
    if !rank.range.is_valid() || program.machine_states(machine).first()?.symbol == state.symbol {
        return None;
    }
    let body = admitted.for_body(machine.symbol, machine.states)?;
    let mut matches = admitted
        .states
        .span(body.states)?
        .iter()
        .filter(|row| row.state == state.symbol);
    let row = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some((
        measure,
        rank.range,
        crate::RankStateBindings::from_checked(admitted, row)?,
    ))
}

/// The exact authored occurrence and operand telescope of an internal arrival.
/// Both rank discovery and edge checking use this inventory; parser copies or
/// equal actual expressions do not collapse distinct incoming obligations.
pub(crate) struct InternalArrival<'program> {
    pub source: usize,
    pub destination: usize,
    pub statement_index: usize,
    pub target: typed_trees::statement::TransitionTargetHandle,
    pub arguments: &'program [ExpressionHandle],
    pub recurrent: bool,
}

pub(crate) fn internal_arrival_sites<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<InternalArrival<'program>> {
    let states = program.machine_states(machine);
    let mut arrivals = Vec::new();
    for (source, state) in states.iter().enumerate() {
        for (statement_index, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            for target in transition.targets() {
                if let Some((path, arguments)) =
                    program.statement_table.named_transition_target(target)
                {
                    if let Some(destination) = internal_state_index(program, machine, path.symbol) {
                        arrivals.push(InternalArrival {
                            source,
                            destination,
                            statement_index,
                            target,
                            arguments: program.statement_table.expression_handles(arguments),
                            recurrent: false,
                        });
                    }
                }
            }
        }
    }
    let mut adjacency = vec![Vec::new(); states.len()];
    for arrival in &arrivals {
        adjacency[arrival.source].push(arrival.destination);
    }
    let components = super::strongly_connected_components(&adjacency);
    for arrival in &mut arrivals {
        arrival.recurrent = components.iter().any(|component| {
            component.contains(&arrival.source)
                && component.contains(&arrival.destination)
                && (component.len() > 1 || arrival.source == arrival.destination)
        });
    }
    arrivals
}

/// Resolve an authored internal state jump. A machine entry target remains an
/// invocation, even when tail lowering reuses this activation's storage.
/// Only subordinate states form the source state graph; otherwise a forwarding
/// state before a recursive call acquires a false strict-decrease obligation.
pub fn internal_state_index(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    target_symbol: SymbolHandle,
) -> Option<usize> {
    typed_trees::state::state_position(program.machine_states(machine), target_symbol)
        .filter(|position| *position != 0)
}

/// The execution destination of an already normalized constant-stack transfer.
/// This locates storage, not source call semantics or ranking obligations.
pub fn activation_target_state_index(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    target_symbol: SymbolHandle,
) -> Option<usize> {
    if target_symbol == machine.symbol {
        return (!program.machine_states(machine).is_empty()).then_some(0);
    }
    typed_trees::state::state_position(program.machine_states(machine), target_symbol)
}

fn target_machine(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> Option<usize> {
    if !symbol.is_valid() {
        return None;
    }
    program.machine_position_by_symbol(symbol).or_else(|| {
        let holder = program.machine_holding_state(symbol)?;
        program.machine_position_by_symbol(holder.symbol)
    })
}

fn expression_is_inert(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    expression_is_inert_or_calls(program, machine, state, expression, &|_| false)
}

/// The same inertness walk with value-position calls admitted per
/// `admit_call`: an accepted call's receiver and arguments recur under the
/// same rule, so nested call arguments and composed initializers are
/// covered, while every subterm that is not a call must still be inert the
/// way it would be with no call present.
fn expression_is_inert_or_calls(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    admit_call: &dyn Fn(&typed_trees::expression::TableCallExpression) -> bool,
) -> bool {
    if !expression.is_valid() {
        return true;
    }
    let mut inert =
        |expression| expression_is_inert_or_calls(program, machine, state, expression, admit_call);
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => program
            .expression_table
            .match_deep_all(dispatch, &mut inert),
        ExpressionNode::Atomic(atomic) => inert(atomic.value),
        typed_trees::literal_expression_pattern!() => true,
        ExpressionNode::Member(member) => inert(member.receiver),
        ExpressionNode::Binary(binary) => {
            meaning::binary_is_inert(program, machine, state, expression, binary)
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
        ExpressionNode::Call(call) => {
            // A call admitted by the caller's bar keeps every subterm inert
            // the way transition actuals are; the caller decides which
            // callees may appear and covers their writes in the aggregate
            // write frame separately.
            admit_call(call)
                && (!call.receiver.is_valid() || inert(call.receiver))
                && program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| inert(*argument))
        }
        ExpressionNode::Indexed(_) | ExpressionNode::Range(_) => false,
    }
}
