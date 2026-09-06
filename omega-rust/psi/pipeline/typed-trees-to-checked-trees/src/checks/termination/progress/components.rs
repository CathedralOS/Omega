//! Close private progress assumptions only within a validated recursive component.

use super::{CheckedProgressSummary, derive_machine_summary, no_guarantee};
use checked_trees::FlowFacts;
use language_semantics::TerminationGuarantee;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

mod projections;

pub(super) fn derive_summaries(
    program: &TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
) -> Vec<CheckedProgressSummary> {
    let components = validation::validated_runtime_recursive_components(program);
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
                summaries[index] =
                    derive_machine_summary(program, flow, semantic, machine, &previous)
                        .unwrap_or_else(|| no_guarantee(machine.symbol));
            }
        }
        for component in &components {
            let derived = derive_component(program, flow, semantic, component, &previous);
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
    program: &TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    component: &[SymbolHandle],
    external: &[CheckedProgressSummary],
) -> Vec<CheckedProgressSummary> {
    let mut summaries = external.to_vec();
    let mut unavailable = vec![false; summaries.len()];
    let projection_limit =
        projections::finite_projection_limit(program, flow, semantic, component, external);
    for (index, machine) in program.machines().iter().enumerate() {
        if component.contains(&machine.symbol) {
            summaries[index].guarantee =
                super::super::infer_machine_checked_summary(program, machine);
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
                derive_machine_summary(program, flow, semantic, machine, &previous)
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
