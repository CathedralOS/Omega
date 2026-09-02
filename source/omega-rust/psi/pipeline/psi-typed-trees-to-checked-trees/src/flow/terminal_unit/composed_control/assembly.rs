//! Atomic assembly after topology, guard, leaf, and provider admission.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &psi_typed_trees::machine::Machine,
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
    let provider_attachment_requirements = checked_composed_provider_attachment_requirements(
        program,
        shapes,
        machine,
        &graph.attachment_type_identity,
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
    )?;
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

pub(super) fn finish(
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    attachment_type_identity: String,
    provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
    states: Vec<CheckedComposedUnitControlStatePlan>,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let body_qualifications = facts
        .qualifications
        .for_machine(machine.symbol)
        .map(|fact| fact.body_committed.clone())
        .unwrap_or_default();
    if !body_qualifications.is_empty() {
        return None;
    }
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let machine_reach = facts.service_reaches.for_machine(machine.symbol)?;
    Some(CheckedComposedUnitControlMachinePlan {
        machine: machine.symbol,
        attachment_type_identity,
        provider_attachment_requirements,
        body_qualifications,
        contract_report_fingerprint: contract.report_fingerprint,
        contract_commitment: contract.commitment,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: psi_language_semantics::ServiceReachSummary {
            direct: machine_reach.inferred_direct,
            transitive: machine_reach.inferred_transitive,
        },
        states,
    })
}
