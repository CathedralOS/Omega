//! Atomic assembly of state-graph Unit control plans after provider admission.
use super::super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedProviderAttachmentRequirementPlan, TypedTrees,
};

pub(in crate::execution::terminal_unit) fn finish(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    attachment_type_identity: Option<String>,
    provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
    states: Vec<CheckedComposedUnitControlStatePlan>,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let body_qualifications =
        super::super::types::body_structural_qualifications(program, facts, machine.symbol);
    if !body_qualifications.is_empty() {
        return None;
    }
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let machine_reach = facts.service_reaches.for_machine(machine.symbol)?;
    Some(CheckedComposedUnitControlMachinePlan {
        machine: machine.symbol,
        result: checked_trees::CheckedControlResultPlan::Unit,
        result_reference_sources: Vec::new(),
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
