//! Atomic assembly after topology, guard, leaf, and provider admission.
use super::super::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedProviderAttachmentRequirementPlan,
    MachineSupplyMode, TypedTrees,
};

use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::composed_control::closed_sum;
use crate::execution::terminal_unit::control;
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
