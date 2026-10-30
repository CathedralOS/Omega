//! Exact proof-call cohorts shared by cycle admission and induction use.
//! Preparing a complete cohort establishes no theorem. Structural admission
//! proves every cycle descends; computed intervals remain a separate judgment.

use super::{
    ExactProofCallEdge, collect_exact_proof_call_edges, proof_call_site_sort_key,
    strongly_connected_components,
};
use std::collections::BTreeSet;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

pub(super) struct CompleteProofCallComponent {
    pub members: Vec<usize>,
    pub edges: Vec<ExactProofCallEdge>,
}

pub(super) fn complete(
    program: &dyn crate::CheckingInput,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    adjacency: &[Vec<usize>],
    proof_dependencies: &[Vec<usize>],
) -> Vec<CompleteProofCallComponent> {
    let machines = program.machines();
    let exact_edges = collect_exact_proof_call_edges(program, proof_only);
    let exact_self_recursive_members = exact_edges
        .iter()
        .filter(|edge| edge.caller == edge.callee)
        .map(|edge| edge.caller)
        .collect::<BTreeSet<_>>();
    let mut components = strongly_connected_components(adjacency)
        .into_iter()
        .filter(|members| {
            (members.len() > 1
                || members
                    .first()
                    .is_some_and(|member| exact_self_recursive_members.contains(member)))
                && members
                    .iter()
                    .all(|member| proof_only.is_proof_machine(program, &machines[*member]))
        })
        .collect::<Vec<_>>();
    components.sort_by_key(|members| members.first().copied().unwrap_or(usize::MAX));

    components
        .into_iter()
        .filter_map(|members| {
            let member_set = members.iter().copied().collect::<BTreeSet<_>>();
            let mut component_edges = exact_edges
                .iter()
                .filter(|edge| {
                    member_set.contains(&edge.caller) && member_set.contains(&edge.callee)
                })
                .cloned()
                .collect::<Vec<_>>();
            component_edges.sort_by_key(|edge| {
                (
                    machines[edge.caller].symbol.arena_index(),
                    proof_call_site_sort_key(edge.site),
                    machines[edge.callee].symbol.arena_index(),
                )
            });
            if component_edges.is_empty() {
                return None;
            }
            // Compare occurrences per machine pair, not just the component's
            // total. A missing selected callee or parallel receiver call must
            // not borrow another pair's structural certificate. Same-machine
            // dependency targets also include state jumps, whose existing
            // recursion judgment remains separate from this completeness fence.
            let mut certified_calls = component_edges
                .iter()
                .filter(|edge| edge.caller != edge.callee)
                .map(|edge| (edge.caller, edge.callee))
                .collect::<Vec<_>>();
            let mut resolved_calls = members
                .iter()
                .flat_map(|caller| {
                    proof_dependencies[*caller]
                        .iter()
                        .filter(|callee| member_set.contains(callee))
                        .map(|callee| (*caller, *callee))
                })
                .collect::<Vec<_>>();
            certified_calls.sort_unstable();
            resolved_calls.sort_unstable();
            if certified_calls != resolved_calls {
                return None;
            }
            Some(CompleteProofCallComponent {
                members,
                edges: component_edges,
            })
        })
        .collect()
}

/// A return to an authored ranking entry closes a cycle from that invocation.
/// Complete structural admission makes every such cycle strictly descend,
/// including a preserving closing edge reached through a descending helper.
/// This licenses induction only; the caller still owes the recursive premises.
pub(crate) fn structural_return_descends(
    program: &dyn crate::CheckingInput,
    origin: &Machine,
    state: &State,
    statement: usize,
    expression: ExpressionHandle,
) -> bool {
    let Some((caller, _)) = super::super::calls::machine_state_by_symbol(program, state.symbol)
    else {
        return false;
    };
    if !super::super::calls::collect_call_sites_in_statement(program, caller, state, statement)
        .iter().any(|site| matches!(site, crate::CallSite::Expression { expression: observed, .. } if *observed == expression)) { return false; }
    let Some((rank, _, _)) = super::proof_rank_parameter(program, origin) else {
        return false;
    };
    let classification = crate::proof_only_classification(program);
    let graph = super::build_machine_call_graph(program, &classification);
    let machines = program.machines();
    complete(
        program,
        &classification,
        &graph.edges,
        &graph.proof_dependencies,
    )
    .into_iter()
    .filter(|component| {
        component
            .members
            .iter()
            .any(|&index| machines[index].symbol == origin.symbol)
            && component
                .members
                .iter()
                .any(|&index| machines[index].symbol == caller.symbol)
    })
    .any(|component| {
        super::proof_ranking::validate_component(program, &component.members, &component.edges)
            .is_some_and(|validated| {
                validated
                    .members
                    .iter()
                    .any(|member| member.machine == origin.symbol && member.rank_parameter == rank)
            })
    })
}
