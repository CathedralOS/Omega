//! Close private progress assumptions within independently ranked recursion.
//!
//! Runtime and erased proof components retain their own checked call-edge
//! authority. State-cycle ranking alone does not authorize a recursive call.
//! Cleanup dependencies are fixed by custody before this analysis. Borrow
//! that exact graph throughout both fixed points; only progress summaries
//! change between rounds, not the selected cleanup calls.
//!
//! A machine's summary reads only the summaries of the machines it calls, so
//! the fixed point runs per call-cycle component, callees first
//! (`shared::machine_waves`). Each call component reaches its own fixed point
//! from its callees' settled summaries, and the call components of one wave
//! run in parallel. Every ranked recursive component above is tied into a
//! single call component, so its joint derivation stays whole.

use super::CheckedProgressSummary;
use crate::checks::termination::progress::machine_summaries::{
    ProgressSummaries, derive_machine_summary, no_guarantee, selected_call_reads, summary_reads,
};
use crate::shared::machine_waves::MachineCallComponents;
use checked_trees::FlowFacts;
use language_semantics::TerminationGuarantee;

mod projections;

/// Inputs shared by every call component's derivation.
struct ProgressInputs<'a, 'frames> {
    program: &'a dyn ::validation::CheckingInput,
    flow: &'a FlowFacts,
    semantic: &'a facts::FactPlan,
    cleanup_dependencies: &'a arena::Arena<flow_effects::CleanupDependency>,
    call_frames: Option<&'a validation::CallFrameResolver<'frames>>,
    rank_arrivals: &'a checked_trees::CheckedRankArrivalPlan,
    maximum_fact_projection_length: usize,
    /// Ranked recursive components: roster positions (ascending) and geometry.
    ranked: Vec<(Vec<usize>, Option<projections::ComponentProjectionGeometry>)>,
    /// For each roster position, the ranked components that contain it.
    ranked_of: Vec<Vec<usize>>,
    /// Whether a machine's own summary is among its reads.
    reads_itself: Vec<bool>,
}

impl ProgressInputs<'_, '_> {
    fn derive(
        &self,
        machine: &typed_trees::machine::Machine,
        summaries: &ProgressSummaries<'_>,
    ) -> CheckedProgressSummary {
        derive_machine_summary(
            self.program,
            self.flow,
            self.semantic,
            self.cleanup_dependencies,
            machine,
            summaries,
            self.call_frames,
            self.rank_arrivals,
        )
        .unwrap_or_else(|| no_guarantee(machine.symbol))
    }
}

pub(super) fn derive_summaries(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    proof_components: &[validation::ValidatedProofCallComponent],
    self_call_descents: &[validation::ValidatedSelfCallDescent],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
) -> Vec<CheckedProgressSummary> {
    let mut components = validation::retained_runtime_recursive_components(rank_arrivals);
    components.extend(rank_arrivals.source_natural_components.iter().filter_map(
        |(_, component)| {
            if component.view != language_semantics::RankingViewId::NAT_DESCENDING
                || rank_arrivals
                    .source_natural_edges
                    .span_or_empty(component.edges)
                    .is_empty()
            {
                return None;
            }
            let members = rank_arrivals
                .source_natural_members
                .span_or_empty(component.members);
            if members.is_empty()
                || !members.iter().all(|member| {
                    program
                        .machine_by_symbol(member.machine)
                        .is_some_and(|machine| machine.states == member.body)
                        && rank_arrivals
                            .judgment_for_body(member.machine, member.body)
                            .is_some_and(|judgment| judgment.local_terminates)
                })
            {
                return None;
            }
            Some(
                members
                    .iter()
                    .map(|member| member.machine)
                    .collect::<Vec<_>>(),
            )
        },
    ));
    components.extend(
        proof_components
            .iter()
            .map(|component| component.members.clone()),
    );
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
    let machines = program.machines();
    let mut ranked_of = vec![Vec::new(); machines.len()];
    let ranked: Vec<_> = components
        .iter()
        .enumerate()
        .map(|(ranked_index, component)| {
            let indices: Vec<usize> = machines
                .iter()
                .enumerate()
                .filter_map(|(index, machine)| component.contains(&machine.symbol).then_some(index))
                .collect();
            for &index in &indices {
                ranked_of[index].push(ranked_index);
            }
            let geometry = projections::prepare_component_geometry(
                program,
                flow,
                semantic,
                component,
                call_frames,
            );
            (indices, geometry)
        })
        .collect();

    // The read graph mirrors every summary lookup a derivation performs.
    let mut reads = vec![Vec::new(); machines.len()];
    for (index, machine) in machines.iter().enumerate() {
        summary_reads(program, flow, machine, &mut reads[index]);
    }
    for (_, dependency) in cleanup_dependencies.iter() {
        if let Some(caller) = program.machine_position_by_symbol(dependency.caller_machine) {
            selected_call_reads(program, dependency.target_machine, &mut reads[caller]);
        }
    }
    for (indices, geometry) in &ranked {
        // A ranked component is derived jointly: tie its members into one
        // cycle, and let it read every external target its projection bound
        // consults.
        for (position, &index) in indices.iter().enumerate() {
            reads[index].push(indices[(position + 1) % indices.len()]);
        }
        if let (Some(&first), Some(geometry)) = (indices.first(), geometry) {
            for &target in geometry.external_call_targets() {
                selected_call_reads(program, target, &mut reads[first]);
            }
        }
    }
    let reads_itself = reads
        .iter()
        .enumerate()
        .map(|(index, reads)| reads.contains(&index))
        .collect();
    let schedule = MachineCallComponents::new(&reads);
    let inputs = ProgressInputs {
        program,
        flow,
        semantic,
        cleanup_dependencies,
        call_frames,
        rank_arrivals,
        maximum_fact_projection_length,
        ranked,
        ranked_of,
        reads_itself,
    };
    let mut summaries = machines
        .iter()
        .map(|machine| no_guarantee(machine.symbol))
        .collect::<Vec<_>>();
    schedule.run_callee_first(&mut summaries, |members, settled| {
        derive_call_component(&inputs, members, settled)
    });
    summaries
}

/// One call component's fixed point from its callees' settled summaries.
///
/// Machines outside every ranked component start from no guarantee and rise
/// with their callees; each ranked component starts from its local promise
/// and is derived jointly. Rounds repeat until no member changes.
fn derive_call_component(
    inputs: &ProgressInputs<'_, '_>,
    members: &[usize],
    settled: &[CheckedProgressSummary],
) -> Vec<CheckedProgressSummary> {
    let machines = inputs.program.machines();
    let mut ranked: Vec<usize> = members
        .iter()
        .flat_map(|&index| inputs.ranked_of[index].iter().copied())
        .collect();
    ranked.sort_unstable();
    ranked.dedup();
    let free = |index: usize| inputs.ranked_of[index].is_empty();

    let initial: Vec<_> = members
        .iter()
        .map(|&index| no_guarantee(machines[index].symbol))
        .collect();
    let mut summaries = initial.clone();
    // The first round reads each earlier member's fresh summary in place.
    let mut constant = vec![false; members.len()];
    for (position, &index) in members.iter().enumerate() {
        if !free(index) {
            continue;
        }
        let machine = &machines[index];
        let summary = inputs.derive(
            machine,
            &ProgressSummaries::new(settled, members, &summaries),
        );
        if matches!(
            &summary.guarantee,
            TerminationGuarantee::Terminates { premises } if premises.is_empty()
        ) || (!machine.target_family_interface
            && (machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !super::super::retained_machine_local_summary(machine, inputs.rank_arrivals)
                    .promises_termination()))
        {
            constant[position] = true;
        }
        summaries[position] = summary;
    }
    if ranked.is_empty() && members.len() == 1 && !inputs.reads_itself[members[0]] {
        // A lone machine off every cycle has read only settled callees.
        return summaries;
    }
    for &component in &ranked {
        derive_component(
            inputs,
            component,
            members,
            settled,
            &initial,
            &mut summaries,
        );
    }
    if equivalent(&summaries, &initial) {
        return summaries;
    }
    let mut last_external = initial;
    loop {
        let previous = summaries.clone();
        for (position, &index) in members.iter().enumerate() {
            if !free(index) || constant[position] {
                continue;
            }
            let summary = inputs.derive(
                &machines[index],
                &ProgressSummaries::new(settled, members, &previous),
            );
            if matches!(
                &summary.guarantee,
                TerminationGuarantee::Terminates { premises } if premises.is_empty()
            ) {
                constant[position] = true;
            }
            summaries[position] = summary;
        }
        let external_changed = members.iter().enumerate().any(|(position, &index)| {
            free(index) && !summary_equivalent(&previous[position], &last_external[position])
        });
        for &component in &ranked {
            if !external_changed && ranked.len() == 1 {
                continue;
            }
            derive_component(
                inputs,
                component,
                members,
                settled,
                &previous,
                &mut summaries,
            );
        }
        // Premises are sets. Their discovery order can rotate around a cycle
        // without changing the judgment.
        if equivalent(&summaries, &previous) {
            return summaries;
        }
        last_external = previous;
    }
}

/// Derive one ranked component jointly. `external` and `destination` hold the
/// enclosing call component's members (`members`, ascending roster order).
fn derive_component(
    inputs: &ProgressInputs<'_, '_>,
    component: usize,
    members: &[usize],
    settled: &[CheckedProgressSummary],
    external: &[CheckedProgressSummary],
    destination: &mut [CheckedProgressSummary],
) {
    let machines = inputs.program.machines();
    let (component_indices, geometry) = &inputs.ranked[component];
    let positions: Vec<usize> = component_indices
        .iter()
        .map(|index| {
            members
                .binary_search(index)
                .expect("a ranked component lies inside one call component")
        })
        .collect();
    let mut summaries = external.to_vec();
    let mut unavailable = vec![false; summaries.len()];
    let projection_limit = projections::finite_projection_limit(
        inputs.program,
        inputs.maximum_fact_projection_length,
        geometry.as_ref(),
        &ProgressSummaries::new(settled, members, external),
    );
    for (&position, &index) in positions.iter().zip(component_indices) {
        summaries[position].guarantee =
            super::super::retained_machine_local_summary(&machines[index], inputs.rank_arrivals);
        unavailable[position] = projection_limit.is_none();
    }
    let mut previous = summaries.clone();
    loop {
        for &position in &positions {
            previous[position] = summaries[position].clone();
        }
        for (&position, &index) in positions.iter().zip(component_indices) {
            let machine = &machines[index];
            let mut summary = if unavailable[position] {
                no_guarantee(machine.symbol)
            } else {
                inputs.derive(
                    machine,
                    &ProgressSummaries::new(settled, members, &previous),
                )
            };
            if let TerminationGuarantee::Terminates { premises } = &summary.guarantee
                && premises.iter().any(|premise| {
                    projection_limit.is_none_or(|limit| premise.subject.projections.len() > limit)
                })
            {
                // This demand needs an unbounded family of projected subjects,
                // not a finite caller premise. Never truncate it into a promise.
                unavailable[position] = true;
                summary = no_guarantee(machine.symbol);
            }
            summaries[position] = summary;
        }
        if positions
            .iter()
            .all(|&position| summary_equivalent(&summaries[position], &previous[position]))
        {
            for &position in &positions {
                destination[position] = summaries[position].clone();
            }
            return;
        }
    }
}

fn summary_equivalent(left: &CheckedProgressSummary, right: &CheckedProgressSummary) -> bool {
    left.machine == right.machine
        && match (&left.guarantee, &right.guarantee) {
            (TerminationGuarantee::NoGuarantee, TerminationGuarantee::NoGuarantee) => true,
            (
                TerminationGuarantee::Terminates { premises: left },
                TerminationGuarantee::Terminates { premises: right },
            ) => left.len() == right.len() && left.iter().all(|item| right.contains(item)),
            _ => false,
        }
}

fn equivalent(left: &[CheckedProgressSummary], right: &[CheckedProgressSummary]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| summary_equivalent(left, right))
}
