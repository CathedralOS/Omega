//! Close private progress assumptions within independently ranked recursion.
//!
//! Runtime and erased proof components retain their own checked call-edge
//! authority. State-cycle ranking alone does not authorize a recursive call.
//! Cleanup dependencies are fixed by custody before this analysis. Borrow
//! that exact graph throughout both fixed points; only progress summaries
//! change between rounds, not the selected cleanup calls.

use super::CheckedProgressSummary;
use crate::checks::termination::progress::machine_summaries::{
    derive_machine_summary, no_guarantee,
};
use checked_trees::FlowFacts;
use language_semantics::TerminationGuarantee;
use symbols::SymbolHandle;

mod projections;

pub(super) fn derive_summaries(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    proof_components: &[validation::ValidatedProofRecursiveComponent],
    self_call_descents: &[validation::ValidatedSelfCallDescent],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
) -> Vec<CheckedProgressSummary> {
    let mut components = validation::retained_runtime_recursive_components(rank_arrivals);
    components.extend(proof_components.iter().map(|component| {
        component
            .members
            .iter()
            .map(|member| member.machine)
            .collect::<Vec<_>>()
    }));
    // Only actual source-judged call descent closes a singleton assumption.
    // A ranked state cycle alone says nothing about a separate self call.
    let proof_only = validation::proof_only_classification(program);
    for descent in self_call_descents {
        let Some(machine) = program.machine_by_symbol(descent.machine) else {
            continue;
        };
        if !proof_only.is_proof_machine(program, machine)
            && !validation::retained_internal_rank_nonincrease(program, machine, rank_arrivals)
        {
            continue;
        }
        if !components
            .iter()
            .any(|component| component.contains(&descent.machine))
        {
            components.push(vec![descent.machine]);
        }
    }
    // The semantic seed inventory is fixed for this analysis. Scan it once,
    // not once per component and external-summary round. Call-summary seeds
    // still change with the fixed point and are joined inside each component.
    let maximum_fact_projection_length = if components.is_empty() {
        0
    } else {
        projections::maximum_fact_projection_length(semantic)
    };
    let mut summaries = program
        .machines()
        .iter()
        .map(|machine| no_guarantee(machine.symbol))
        .collect::<Vec<_>>();
    loop {
        let previous = summaries.clone();
        for (index, machine) in program.machines().iter().enumerate() {
            if !components
                .iter()
                .any(|component| component.contains(&machine.symbol))
            {
                summaries[index] = derive_machine_summary(
                    program,
                    flow,
                    semantic,
                    cleanup_dependencies,
                    machine,
                    &previous,
                    call_frames,
                    rank_arrivals,
                )
                .unwrap_or_else(|| no_guarantee(machine.symbol));
            }
        }
        for component in &components {
            let derived = derive_component(
                program,
                flow,
                semantic,
                cleanup_dependencies,
                component,
                &previous,
                maximum_fact_projection_length,
                call_frames,
                rank_arrivals,
            );
            for (index, machine) in program.machines().iter().enumerate() {
                if component.contains(&machine.symbol) {
                    summaries[index] = derived[index].clone();
                }
            }
        }
        // Premises and build-bound demands are sets. Their discovery order can
        // rotate around a cycle without changing the judgment.
        if equivalent(&summaries, &previous) {
            return summaries;
        }
    }
}

fn derive_component(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    component: &[SymbolHandle],
    external: &[CheckedProgressSummary],
    maximum_fact_projection_length: usize,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
) -> Vec<CheckedProgressSummary> {
    let mut summaries = external.to_vec();
    let mut unavailable = vec![false; summaries.len()];
    let projection_limit = projections::finite_projection_limit(
        program,
        flow,
        maximum_fact_projection_length,
        component,
        external,
        call_frames,
    );
    for (index, machine) in program.machines().iter().enumerate() {
        if component.contains(&machine.symbol) {
            summaries[index].guarantee =
                super::super::retained_machine_local_summary(machine, rank_arrivals);
            summaries[index].build_bound_demands.clear();
            unavailable[index] = projection_limit.is_none();
        }
    }
    loop {
        let previous = summaries.clone();
        for (index, machine) in program.machines().iter().enumerate() {
            if !component.contains(&machine.symbol) {
                continue;
            }
            let mut summary = if unavailable[index] {
                no_guarantee(machine.symbol)
            } else {
                derive_machine_summary(
                    program,
                    flow,
                    semantic,
                    cleanup_dependencies,
                    machine,
                    &previous,
                    call_frames,
                    rank_arrivals,
                )
                .unwrap_or_else(|| no_guarantee(machine.symbol))
            };
            if let TerminationGuarantee::Terminates { premises } = &summary.guarantee
                && premises.iter().any(|premise| {
                    projection_limit.is_none_or(|limit| premise.subject.projections.len() > limit)
                })
            {
                // This demand needs an unbounded family of projected subjects,
                // not a finite caller premise. Never truncate it into a promise.
                unavailable[index] = true;
                summary = no_guarantee(machine.symbol);
            }
            summaries[index] = summary;
        }
        if equivalent(&summaries, &previous) {
            return summaries;
        }
    }
}

fn equivalent(left: &[CheckedProgressSummary], right: &[CheckedProgressSummary]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.machine == right.machine
                && match (&left.guarantee, &right.guarantee) {
                    (TerminationGuarantee::NoGuarantee, TerminationGuarantee::NoGuarantee) => true,
                    (
                        TerminationGuarantee::Terminates { premises: left },
                        TerminationGuarantee::Terminates { premises: right },
                    ) => same_set(left, right),
                    _ => false,
                }
                && same_set(&left.build_bound_demands, &right.build_bound_demands)
        })
}

fn same_set<T: PartialEq>(left: &[T], right: &[T]) -> bool {
    left.len() == right.len() && left.iter().all(|item| right.contains(item))
}
