//! Whole-composition extraction: the sealed model retains activation
//! creation, resource identities, wait/wake edges, placement, provider
//! evidence and explicit dimension absence; replay rejects drifted models.

use super::{
    activation_fact, activation_fact_for, activation_set, candidate, canonical_crossing, id,
    wcsu_plan,
};
use crate::{
    ActivationPlanCandidate, CompositionCrossActivationEdges, CompositionPriorities,
    MachineContractId, TaskActivationPlanFact, TaskActivationPlanSet, TaskSpecializationCommitment,
    TaskStartOperation, ValidatedActivationPlan, compose_composition_model,
    replay_composition_model, validate_activation_plan,
};

fn second_candidate() -> ActivationPlanCandidate {
    let mut candidate = candidate();
    candidate.machine_contract = id(41, MachineContractId::from_normalized_identity);
    candidate
}

fn second_fact(plan: &ValidatedActivationPlan) -> TaskActivationPlanFact {
    let mut fact = activation_fact(plan);
    fact.operation = TaskStartOperation::TryStart;
    fact.specialization_commitment = TaskSpecializationCommitment::from_digest([9; 32]);
    fact
}

#[test]
fn composition_extraction_retains_every_dimension() {
    let plan = wcsu_plan(31);
    let fact = activation_fact_for(&plan, TaskStartOperation::TryStart);
    let set = TaskActivationPlanSet {
        activations: vec![fact.clone()],
    };

    let model = compose_composition_model(&set).expect("composition model composes");
    assert_eq!(model.activations().len(), 1);
    let row = &model.activations()[0];

    // Activation creation.
    assert_eq!(row.creation.operation, TaskStartOperation::TryStart);
    assert_eq!(row.creation.start_requirement, fact.start_requirement);
    assert_eq!(row.creation.target_machine, fact.target_machine);
    assert_eq!(row.creation.target_entry, fact.target_entry);
    assert_eq!(
        row.creation.specialization_commitment,
        fact.specialization_commitment
    );
    assert_eq!(
        row.creation.specialization_report_fingerprint,
        fact.specialization_report_fingerprint
    );

    // Resource identities, including the sealed WCSU projection coordinate.
    let candidate = plan.candidate();
    assert_eq!(row.plan, plan.normalized_identity());
    assert_eq!(row.resources.machine_contract, candidate.machine_contract);
    assert_eq!(row.resources.entry, candidate.entry);
    assert_eq!(
        row.resources.argument_layout,
        candidate.argument_layout.identity
    );
    assert_eq!(
        row.resources.terminal_outcome_layout,
        candidate.terminal_outcome_layout
    );
    assert_eq!(row.resources.calling_plan, candidate.calling_plan);
    assert_eq!(row.resources.stack, candidate.stack_plan);
    assert_eq!(
        row.resources.wcsu_stack_projection,
        plan.wcsu_stack_projection().map(|p| p.identity())
    );

    // Wait/wake edges: exactly the canonical suspension crossings.
    assert_eq!(row.wait_wake_edges.len(), 1);
    assert_eq!(row.wait_wake_edges[0].crossing, canonical_crossing());
    assert!(row.wait_wake_edges[0].preserve_cpu);
    assert!(!row.wait_wake_edges[0].preserve_host_thread);
    assert!(row.cancellation_required);

    // Placement is the join the activation commits to.
    assert_eq!(row.placement, candidate.carry_obligations);

    // Provider evidence travels verbatim.
    assert_eq!(row.provider_evidence, fact.selected_runtime);

    // Dimensions the settled vocabulary cannot yet answer are explicit.
    assert_eq!(model.priorities(), CompositionPriorities::NotRetained);
    assert_eq!(
        model.cross_activation_edges(),
        CompositionCrossActivationEdges::NotRetained
    );
}

#[test]
fn composition_model_identity_is_order_canonical() {
    let first = wcsu_plan(31);
    let second = validate_activation_plan(second_candidate()).expect("second plan validates");
    let forward = TaskActivationPlanSet {
        activations: vec![activation_fact(&first), second_fact(&second)],
    };
    let backward = TaskActivationPlanSet {
        activations: vec![second_fact(&second), activation_fact(&first)],
    };

    let model = compose_composition_model(&forward).expect("forward composes");
    let reversed = compose_composition_model(&backward).expect("backward composes");
    assert_eq!(model, reversed);
    assert_eq!(model.identity(), reversed.identity());
    assert_eq!(model.activations().len(), 2);
}

#[test]
fn composition_replay_rejects_a_drifted_model() {
    let plan = wcsu_plan(31);
    let set = activation_set(&plan);
    let model = compose_composition_model(&set).expect("composition model composes");
    replay_composition_model(&set, &model).expect("fresh model replays");

    // A model extracted from a different settled set cannot replay here.
    let changed = TaskActivationPlanSet {
        activations: vec![activation_fact_for(&plan, TaskStartOperation::TryStart)],
    };
    let drifted = compose_composition_model(&changed).expect("changed set composes");
    let error = replay_composition_model(&set, &drifted).expect_err("drifted model rejects");
    assert!(
        error.0.contains("does not match a fresh extraction"),
        "unexpected diagnostic: {}",
        error.0
    );
}

#[test]
fn composition_rejects_duplicate_plan_identities() {
    let plan = wcsu_plan(31);
    let set = TaskActivationPlanSet {
        activations: vec![activation_fact(&plan), second_fact(&plan)],
    };
    let error = compose_composition_model(&set).expect_err("duplicate plan identities reject");
    assert!(
        error.0.contains("same plan identity"),
        "unexpected diagnostic: {}",
        error.0
    );
}

#[test]
fn empty_plan_set_composes_a_sealed_empty_model() {
    let model = compose_composition_model(&TaskActivationPlanSet::default())
        .expect("empty composition composes");
    assert!(model.activations().is_empty());
    replay_composition_model(&TaskActivationPlanSet::default(), &model)
        .expect("empty model replays");
}
