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
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

use crate::proof_contracts::contract_entailment::{
    RankingRangeCallEdge, RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
    call_member_premise_symbols, discover_state_entry_mappings, mixed_call_endpoints_are_pinned,
    prove_ranking_range_call, prove_ranking_range_call_entry,
};
use comparison::Comparison;
use projection::RankProjection;

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
    // graph can still cycle. A call issued from a subordinate state still
    // answers to this shared hypothesis. The member's discovered
    // telescope names the entry role each site formal carries, so
    // authored subjects and endpoints normalize to the atom the site
    // actually holds. Discovery keeps a contested claim only for a bare
    // forward, so every slot sharing one role is an equal copy of the
    // same arrival value; a computed claimant stays role-less rather than
    // borrowing equality evidence the member's own edge judgment may never
    // have run for an admitted component.
    let member_mappings = component
        .iter()
        .enumerate()
        .map(|(position, index)| {
            discover_state_entry_mappings(
                program,
                &program.machines()[*index],
                ranks[position].parameter,
                &[],
            )
        })
        .collect::<Option<Vec<_>>>();
    let Some(member_mappings) = member_mappings else {
        return Err("the ranking needs entry-to-state arrival evidence");
    };
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
                        StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => continue,
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
                            // stage still runs that judgment whenever the
                            // member's local state graph can cycle -- so it
                            // adds no hypothesis edge here.
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
                            match comparison::argument_comparison(
                                program,
                                &ranks[position],
                                *argument,
                                &site_guards,
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
        // A builtin subslice is a pure rewindowing of its collection. Only an
        // authored or selected index operation could hide an effect here, and
        // its scalar bounds traverse the same inert check.
        ExpressionNode::Indexed(indexed)
            if crate::value_custody::places::has_builtin_subslice_meaning(
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
                _ => false,
            };
            inert(indexed.collection) && endpoints_inert
        }
        // A borrow can expose the ranked value; a call or an unadmitted indexed
        // operation can have selected behavior this pure rank slice cannot see.
        ExpressionNode::Borrow(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Range(_) => false,
    }
}
