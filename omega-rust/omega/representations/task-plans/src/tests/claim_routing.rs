//! Routed `Task<T>` establishment: admission mints the
//! `provider`/`activation` field pair a source task value carries, the pair
//! resolves back to its live claim on the minting instance, and every
//! transition that reaches the runtime as a bare `Task<T>` value fails
//! closed on a foreign, fabricated, or already-settled pair.

use super::{
    activation_set, canonical_crossing, id, moved_arguments, receipt_candidate, wcsu_plan,
};
use crate::{
    ActivationInstanceId, StackPlan, TaskClaimRoute, TaskRuntimeAdmission, TaskRuntimeInstanceId,
    TaskSettlementOutcome, TaskStartOperation, TaskStorageBinding, TaskStorageOwnerId,
    TaskStorageProvenance,
};

fn instance(identity: u64) -> TaskRuntimeInstanceId {
    id(identity, TaskRuntimeInstanceId::from_normalized_identity)
}

fn activation(identity: u64) -> ActivationInstanceId {
    id(identity, ActivationInstanceId::from_normalized_identity)
}

fn owner() -> TaskStorageOwnerId {
    id(300, TaskStorageOwnerId::from_normalized_identity)
}

fn gate(slots: &[StackPlan], instance_identity: u64) -> TaskRuntimeAdmission {
    TaskRuntimeAdmission::new(
        super::runtime(),
        instance(instance_identity),
        owner(),
        slots.to_vec(),
    )
    .expect("provisioned admission gate")
}

/// A start admitted through the gate's own provisioned backing.
fn admit_start(
    gate: &mut TaskRuntimeAdmission,
    activations: &crate::TaskActivationPlanSet,
    plan: &crate::ValidatedActivationPlan,
    instance_identity: u64,
    invocation: u64,
    receipt: u64,
    activation_id: u64,
    custody: u64,
) -> crate::TaskLifecycleClaim {
    gate.admit_pending(
        activations,
        receipt_candidate(
            plan,
            instance(instance_identity),
            invocation,
            receipt,
            TaskStartOperation::Start,
        ),
        activation(activation_id),
        moved_arguments(plan, custody),
    )
    .expect("admission with provisioned backing")
}

#[test]
fn accepted_admission_mints_the_source_route() {
    let plan = wcsu_plan(50);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 301);

    let claim = admit_start(&mut gate, &activations, &plan, 301, 302, 303, 304, 305);
    let route = claim.route();

    // The source `Task<T>` fields name the minting runtime instance and the
    // accepted activation — the only pair that can ever resolve to this
    // claim.
    assert_eq!(route.provider(), 301);
    assert_eq!(route.activation(), 304);
    assert_eq!(
        gate.resolve_claim_route(route).expect("the minted route"),
        claim.identity(),
    );
    let record = gate.records().next().expect("one live dependency");
    assert_eq!(record.route(), route, "the record carries the same route");
}

#[test]
fn routes_reject_foreign_instances() {
    let plan = wcsu_plan(50);
    let activations = activation_set(&plan);
    let mut first = gate(&[plan.candidate().stack_plan], 301);
    let mut second = gate(&[plan.candidate().stack_plan], 401);

    let claim = admit_start(&mut first, &activations, &plan, 301, 302, 303, 304, 305);
    let route = claim.route();

    // Cross-instance failure returns the claim: nothing on the second
    // instance answers for the pair the first minted.
    let diagnostic = second
        .resolve_claim_route(route)
        .expect_err("a foreign instance cannot resolve the route");
    assert!(
        diagnostic.0.contains("different runtime instance"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    for result in [
        second.request_cancellation_by_route(route),
        second.park_by_route(route, canonical_crossing()),
        second.resume_by_route(route).map(|_| ()),
        second.observe_cancellation_by_route(route, canonical_crossing()),
    ] {
        assert!(
            result
                .expect_err("a foreign route fails closed")
                .0
                .contains("different runtime instance"),
        );
    }
    let error = second
        .settle_by_route(route, TaskSettlementOutcome::Completed)
        .expect_err("a foreign route cannot settle here");
    assert_eq!(error.route(), route, "rejection reports the presented pair");
    assert!(
        error.diagnostic().0.contains("different runtime instance"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );

    // The claim survives untouched on its minting instance.
    assert_eq!(
        first
            .resolve_claim_route(route)
            .expect("still live at home"),
        claim.identity(),
    );
    let settled = first
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("the minting instance settles its own claim");
    assert_eq!(settled.route(), route);
}

#[test]
fn routes_reject_fabricated_and_settled_pairs() {
    let plan = wcsu_plan(50);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 301);

    let claim = admit_start(&mut gate, &activations, &plan, 301, 302, 303, 304, 305);
    let route = claim.route();

    // Same provider, never-minted activation.
    let fabricated = TaskClaimRoute::new(301, 999);
    let diagnostic = gate
        .resolve_claim_route(fabricated)
        .expect_err("a fabricated pair resolves to nothing");
    assert!(
        diagnostic.0.contains("no live claim"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );

    gate.settle_by_route(route, TaskSettlementOutcome::Completed)
        .expect("the minted route settles");
    let diagnostic = gate
        .resolve_claim_route(route)
        .expect_err("a settled claim's route is stale");
    assert!(diagnostic.0.contains("no live claim"));
    let error = gate
        .settle_by_route(route, TaskSettlementOutcome::Completed)
        .expect_err("a settled route cannot settle twice");
    assert_eq!(error.route(), route);
    assert!(error.diagnostic().0.contains("no live claim"));
}

#[test]
fn routed_cancellation_settles_only_after_observation() {
    let plan = wcsu_plan(50);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 301);

    let claim = admit_start(&mut gate, &activations, &plan, 301, 302, 303, 304, 305);
    let route = claim.route();

    // A cancelled settlement driven by the value the source holds needs the
    // same recorded request plus safe-point observation as the claim path.
    let error = gate
        .settle_by_route(route, TaskSettlementOutcome::Cancelled)
        .expect_err("no request was ever recorded");
    assert!(
        error
            .diagnostic()
            .0
            .contains("without a recorded cancellation request"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );

    gate.request_cancellation_by_route(route)
        .expect("the route records the request");
    assert!(gate.cancellation_requested(claim.identity()));

    let error = gate
        .settle_by_route(route, TaskSettlementOutcome::Cancelled)
        .expect_err("a bare request is not the observation");
    assert!(
        error.diagnostic().0.contains("safe-point observation"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );

    gate.observe_cancellation_by_route(route, canonical_crossing())
        .expect("the running activation observes at a canonical crossing");
    assert_eq!(
        gate.cancellation_observed(claim.identity()),
        Some(canonical_crossing())
    );

    let settled = gate
        .settle_by_route(route, TaskSettlementOutcome::Cancelled)
        .expect("recorded then observed settles cancelled");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Cancelled);
    assert_eq!(
        settled.released_storage(),
        TaskStorageBinding::Persistent(TaskStorageProvenance {
            owner: owner(),
            lease: id(1, crate::TaskStorageLeaseId::from_normalized_identity),
        }),
        "routed settlement releases the exact pool lease",
    );
}

#[test]
fn routed_park_resume_and_settlement() {
    let plan = wcsu_plan(50);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 301);

    let claim = admit_start(&mut gate, &activations, &plan, 301, 302, 303, 304, 305);
    let route = claim.route();

    gate.park_by_route(route, canonical_crossing())
        .expect("the running activation parks at its canonical crossing");
    assert_eq!(
        gate.parked_crossing(claim.identity()),
        Some(canonical_crossing())
    );

    // A parked activation holds no terminal outcome: routed settlement
    // refuses without consuming anything.
    let error = gate
        .settle_by_route(route, TaskSettlementOutcome::Completed)
        .expect_err("a parked activation cannot settle");
    assert!(
        error.diagnostic().0.contains("parked"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );
    assert_eq!(
        gate.resolve_claim_route(route).expect("still live"),
        claim.identity()
    );

    let crossing = gate
        .resume_by_route(route)
        .expect("the parked invocation resumes under the same claim");
    assert_eq!(crossing, canonical_crossing());
    assert_eq!(gate.parked_crossing(claim.identity()), None);

    gate.settle_by_route(route, TaskSettlementOutcome::Completed)
        .expect("the resumed activation settles");
}

#[test]
fn routed_settlement_releases_pool_backing_for_fresh_eras() {
    let plan = wcsu_plan(50);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 301);

    let first = admit_start(&mut gate, &activations, &plan, 301, 302, 303, 304, 305);
    let first_route = first.route();
    gate.settle_by_route(first_route, TaskSettlementOutcome::Completed)
        .expect("first routed settlement");

    // The released slot re-admits under a fresh lease era; the settled pair
    // never rebinds.
    let second = admit_start(&mut gate, &activations, &plan, 301, 402, 403, 404, 405);
    assert_ne!(
        second.route(),
        first_route,
        "a fresh activation mints a fresh route"
    );
    assert!(
        gate.resolve_claim_route(first_route).is_err(),
        "the settled pair stays stale"
    );
    let record = gate.records().next().expect("one live dependency");
    assert_eq!(record.route(), second.route());
    let TaskStorageBinding::Persistent(provenance) = record.storage else {
        panic!("pending admission records persistent storage");
    };
    assert_eq!(
        provenance.lease.normalized_identity(),
        2,
        "storage reuse mints the next lease era"
    );
}
