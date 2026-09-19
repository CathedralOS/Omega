//! Atomic assembly after topology, guard, leaf, and provider admission.
use super::super::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedProviderAttachmentRequirementPlan, MachineSupplyMode, TypedTrees,
};

use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::checked_composed_provider_attachment_requirements;
use crate::execution::terminal_unit::composed_control::closed_sum;
use crate::execution::terminal_unit::composed_control::nested_control;
use crate::execution::terminal_unit::composed_control::prefixed_control;
use crate::execution::terminal_unit::control;
use crate::execution::terminal_unit::state_flow;
use checked_trees::CheckedUnitPlanOmissionStage;
use std::collections::BTreeMap;

/// Test convenience: the traced assembly without a trace map.
#[cfg(test)]
pub(in crate::execution::terminal_unit) fn build_all(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<CheckedComposedUnitControlMachinePlan> {
    build_all_traced(
        program,
        facts,
        scalar_callees,
        shapes,
        boundaries,
        call_frames,
        &mut BTreeMap::new(),
    )
}

/// `build_all` that also records, for every checked-body machine no composed
/// builder admitted, where the general state-graph route stopped (keyed by
/// the machine symbol's arena index and generation). The shape-specialized
/// builders decline by shape; the state graph's phase is the informative one.
pub(in crate::execution::terminal_unit) fn build_all_traced(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    declined: &mut BTreeMap<(u32, u32), CheckedUnitPlanOmissionStage>,
) -> Vec<CheckedComposedUnitControlMachinePlan> {
    let mut plans = Vec::new();
    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
    {
        let trace = control::LocalConstructionTrace::default();
        let plan = closed_sum::build(
            program,
            facts,
            scalar_callees,
            shapes,
            boundaries,
            machine,
            call_frames,
        )
        .or_else(|| build(program, facts, shapes, boundaries, machine))
        .or_else(|| prefixed_control::build(program, facts, shapes, boundaries, machine))
        .or_else(|| nested_control::build(program, facts, shapes, boundaries, machine))
        .or_else(|| {
            super::super::state_graph::build_traced(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                call_frames,
                &trace,
            )
        });
        match plan {
            Some(plan) => plans.push(plan),
            None => {
                declined.insert(
                    (machine.symbol.arena_index(), machine.symbol.generation()),
                    trace.stage(),
                );
            }
        }
    }
    plans
}

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let graph = super::topology::admit(program, facts, shapes, machine)?;
    let leaves = [
        super::leaves::build(
            program,
            facts,
            machine,
            graph.leaves[0],
            boundaries,
            &graph.leaf_structural_parameters[0],
            &graph.leaf_entry_claims[0],
        )?,
        super::leaves::build(
            program,
            facts,
            machine,
            graph.leaves[1],
            boundaries,
            &graph.leaf_structural_parameters[1],
            &graph.leaf_entry_claims[1],
        )?,
    ];
    let true_flow = state_flow(facts, machine.symbol, graph.leaves[0].symbol)?;
    let false_flow = state_flow(facts, machine.symbol, graph.leaves[1].symbol)?;
    let provider_attachment_requirements =
        if let Some(attachment) = graph.attachment_type_identity.as_deref() {
            checked_composed_provider_attachment_requirements(
                program,
                shapes,
                machine,
                attachment,
                &[
                    (
                        graph.leaves[0],
                        facts.flow.control.calls.span_or_empty(true_flow.calls),
                        &leaves[0].operations,
                    ),
                    (
                        graph.leaves[1],
                        facts.flow.control.calls.span_or_empty(false_flow.calls),
                        &leaves[1].operations,
                    ),
                ],
            )?
        } else {
            Vec::new()
        };
    finish(
        facts,
        machine,
        graph.attachment_type_identity,
        provider_attachment_requirements,
        vec![
            CheckedComposedUnitControlStatePlan {
                state: graph.entry.symbol,
                structural_parameters: graph.entry_structural_parameters,
                scalar_parameters: graph.entry_scalar_parameters,
                erased_scalar_parameters: graph.entry_erased_scalar_parameters,
                requires: Vec::new(),
                entry_claims: graph.entry_claims,
                bindings: graph.entry_bindings,
                binding_initializers: graph.entry_binding_initializers,
                operations: Vec::new(),
                terminator: CheckedComposedUnitControlTerminatorPlan::Conditional {
                    guard: graph.guard,
                    when_true: graph.successors[0].clone(),
                    when_false: graph.successors[1].clone(),
                },
            },
            leaves[0].clone(),
            leaves[1].clone(),
        ],
    )
}

pub(in crate::execution::terminal_unit) fn finish(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    attachment_type_identity: Option<String>,
    provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
    states: Vec<CheckedComposedUnitControlStatePlan>,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let body_qualifications = facts
        .qualifications
        .for_machine(machine.symbol)
        .map(|fact| {
            fact.body_committed
                .iter()
                .copied()
                .filter(|domain| {
                    !matches!(
                        *domain,
                        language_semantics::SemanticDomainTable::WRAPPING
                            | language_semantics::SemanticDomainTable::SATURATING
                            | language_semantics::SemanticDomainTable::TRAPPING
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !body_qualifications.is_empty() {
        return None;
    }
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let machine_reach = facts.service_reaches.for_machine(machine.symbol)?;
    Some(CheckedComposedUnitControlMachinePlan {
        machine: machine.symbol,
        result: checked_trees::CheckedControlResultPlan::Unit,
        natural_ranks: Vec::new(),
        attachment_type_identity,
        provider_attachment_requirements,
        body_qualifications,
        contract_report_fingerprint: contract.report_fingerprint,
        contract_commitment: contract.commitment,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: language_semantics::ServiceReachSummary {
            direct: machine_reach.inferred_direct,
            transitive: machine_reach.inferred_transitive,
        },
        states,
    })
}
