//! Provider admission: the gate that binds receipts, spends bounded
//! provisioned backing under fresh lease eras, and conserves custody on
//! every rejection.

use super::{
    activation_fact_for, activation_set, id, moved_arguments, receipt_candidate, runtime,
    stack_lease, wcsu_plan,
};
use crate::{
    ActivationInstanceId, ActivationPlanId, MovedTaskArguments, StackPlan, TaskActivationPlanSet,
    TaskArgumentCustodyId, TaskRuntimeAdmission, TaskRuntimeId, TaskRuntimeInstanceId,
    TaskSettlementOutcome, TaskStartOperation, TaskStartStorage, TaskStorageBinding,
    TaskStorageOwnerId, TaskStorageProvenance,
};

fn instance(identity: u64) -> TaskRuntimeInstanceId {
    id(identity, TaskRuntimeInstanceId::from_normalized_identity)
}

fn activation(identity: u64) -> ActivationInstanceId {
    id(identity, ActivationInstanceId::from_normalized_identity)
}

fn custody(identity: u64) -> TaskArgumentCustodyId {
    id(identity, TaskArgumentCustodyId::from_normalized_identity)
}

fn owner() -> TaskStorageOwnerId {
    id(300, TaskStorageOwnerId::from_normalized_identity)
}

fn gate(slots: &[StackPlan], instance_identity: u64) -> TaskRuntimeAdmission {
    TaskRuntimeAdmission::new(
        runtime(),
        instance(instance_identity),
        owner(),
        slots.to_vec(),
    )
    .expect("provisioned admission gate")
}

fn persistent_binding(record: &crate::TaskDependencyRecord) -> TaskStorageProvenance {
    let TaskStorageBinding::Persistent(provenance) = record.storage else {
        panic!("pending admission records persistent storage");
    };
    provenance
}

#[test]
fn pending_admission_establishes_a_fresh_provider_lease() {
    let plan = wcsu_plan(50);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 301);

    let claim = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(301), 302, 303, TaskStartOperation::Start),
            activation(304),
            moved_arguments(&plan, 305),
        )
        .expect("a satisfying provisioned slot admits the pending start");

    let record = gate.records().next().expect("one live dependency");
    let provenance = persistent_binding(record);
    assert_eq!(provenance.owner, owner());
    assert_eq!(
        provenance.lease.normalized_identity(),
        1,
        "the provider mints the first fresh lease era"
    );
    assert_eq!(record.operation, TaskStartOperation::Start);
    gate.request_cancellation(&claim)
        .expect("a live claim accepts a cancellation request");
    assert!(gate.cancellation_requested(claim.identity()));

    let close = gate.close().expect_err("a live claim blocks close");
    let mut gate = close.into_admission();
    let settled = gate
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("terminal settlement");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Cancelled);
    assert_eq!(
        settled.released_storage(),
        TaskStorageBinding::Persistent(provenance),
        "settlement releases the exact pool lease"
    );
    gate.close().expect("an empty gate closes");
}

#[test]
fn exhausted_provisioning_rejects_and_settlement_releases_backing_under_a_fresh_era() {
    let plan = wcsu_plan(51);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 310);

    let first = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(310), 311, 312, TaskStartOperation::Start),
            activation(313),
            moved_arguments(&plan, 314),
        )
        .expect("first admission");

    // Fixed capacity is real authority: a second start rejects and
    // conserves its whole moved bundle.
    let second_custody = custody(316);
    let rejection = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(310), 317, 318, TaskStartOperation::Start),
            activation(319),
            MovedTaskArguments::new(plan.candidate().argument_layout, second_custody),
        )
        .expect_err("a single-slot pool cannot admit a second pending activation");
    assert!(
        rejection.diagnostic().0.contains("exhausted"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );
    assert_eq!(rejection.into_arguments().custody(), second_custody);

    // Settlement returns the backing, and its next admission mints a fresh
    // lease era rather than replaying the released one.
    gate.settle(first, TaskSettlementOutcome::Completed)
        .expect("settle first");
    let second = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(310), 320, 321, TaskStartOperation::Start),
            activation(322),
            moved_arguments(&plan, 323),
        )
        .expect("released backing admits again");
    let provenance = persistent_binding(gate.records().next().expect("one live dependency"));
    assert_eq!(
        provenance.lease.normalized_identity(),
        2,
        "reused storage mints a fresh lease era"
    );
    gate.settle(second, TaskSettlementOutcome::Completed)
        .expect("settle second");
    gate.close().expect("empty gate closes");
}

#[test]
fn unsatisfying_backing_rejects_without_spending_capacity() {
    let plan = wcsu_plan(52);
    let activations = activation_set(&plan);
    let required = plan.candidate().stack_plan;
    let undersized = StackPlan {
        bytes: required.bytes / 2,
        ..required
    };
    let mut gate = gate(&[undersized], 330);

    let rejected_custody = custody(333);
    let rejection = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(330), 331, 332, TaskStartOperation::Start),
            activation(334),
            MovedTaskArguments::new(plan.candidate().argument_layout, rejected_custody),
        )
        .expect_err("undersized backing cannot satisfy the plan");
    assert!(
        rejection.diagnostic().0.contains("satisfying"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );
    assert_eq!(rejection.into_arguments().custody(), rejected_custody);

    // The failed admission spent nothing: a caller-supplied lease still
    // reaches the ledger through the same gate.
    let claim = gate
        .admit_with_storage(
            &activations,
            receipt_candidate(&plan, instance(330), 335, 336, TaskStartOperation::Start),
            activation(337),
            moved_arguments(&plan, 338),
            TaskStartStorage::Persistent(stack_lease(&plan, 339, 340)),
        )
        .expect("caller-supplied lease admits without touching the pool");
    gate.settle(claim, TaskSettlementOutcome::Completed)
        .expect("settle caller-supplied storage");
}

#[test]
fn admission_selects_the_satisfying_slot() {
    let plan = wcsu_plan(53);
    let activations = activation_set(&plan);
    let required = plan.candidate().stack_plan;
    let undersized = StackPlan {
        bytes: required.bytes / 2,
        ..required
    };
    let mut gate = gate(&[undersized, required], 340);

    gate.admit_pending(
        &activations,
        receipt_candidate(&plan, instance(340), 341, 342, TaskStartOperation::Start),
        activation(343),
        moved_arguments(&plan, 344),
    )
    .expect("the satisfying slot admits");

    // Only the undersized slot remains unleased, so the next pending start
    // rejects on fit rather than on raw capacity.
    let rejection = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(340), 345, 346, TaskStartOperation::Start),
            activation(347),
            moved_arguments(&plan, 348),
        )
        .expect_err("the remaining undersized slot cannot satisfy");
    assert!(
        rejection.diagnostic().0.contains("satisfying"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );
}

#[test]
fn inline_completion_admits_without_provisioning() {
    let plan = wcsu_plan(54);
    let activations = activation_set(&plan);
    let mut gate = gate(&[], 350);

    let claim = gate
        .admit_with_storage(
            &activations,
            receipt_candidate(&plan, instance(350), 351, 352, TaskStartOperation::Start),
            activation(353),
            moved_arguments(&plan, 354),
            TaskStartStorage::InlineCompletion,
        )
        .expect("an inline completion needs no provisioned backing");
    let record = gate.records().next().expect("one live dependency");
    assert_eq!(record.storage, TaskStorageBinding::InlineCompletion);

    let settled = gate
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("settle inline completion");
    assert_eq!(
        settled.released_storage(),
        TaskStorageBinding::InlineCompletion
    );
    assert!(settled.into_released_lease().is_none());
    gate.close().expect("empty gate closes");
}

#[test]
fn receipt_binding_rejects_before_custody_moves() {
    let plan = wcsu_plan(55);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 360);

    // A receipt naming an activation plan this provider does not serve
    // fails closed.
    let mut foreign_plan =
        receipt_candidate(&plan, instance(360), 361, 362, TaskStartOperation::Start);
    foreign_plan.activation_plan = id(363, ActivationPlanId::from_normalized_identity);
    let rejection = gate
        .admit_pending(
            &activations,
            foreign_plan,
            activation(364),
            moved_arguments(&plan, 365),
        )
        .expect_err("an unknown activation plan rejects");
    assert!(
        rejection.diagnostic().0.contains("no activation plan"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );

    // A receipt naming a different selected runtime fails binding even
    // though the plan is served.
    let mut foreign_runtime =
        receipt_candidate(&plan, instance(360), 366, 367, TaskStartOperation::Start);
    foreign_runtime.runtime = id(368, TaskRuntimeId::from_normalized_identity);
    let rejection = gate
        .admit_pending(
            &activations,
            foreign_runtime,
            activation(369),
            moved_arguments(&plan, 370),
        )
        .expect_err("runtime drift rejects");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("different selected runtime"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );

    // Neither rejection spent the provisioned slot.
    gate.admit_pending(
        &activations,
        receipt_candidate(&plan, instance(360), 371, 372, TaskStartOperation::Start),
        activation(373),
        moved_arguments(&plan, 374),
    )
    .expect("the untouched slot still admits");
}

#[test]
fn operation_binds_the_exact_activation_fact() {
    let plan = wcsu_plan(56);
    let activations = TaskActivationPlanSet {
        activations: vec![
            activation_fact_for(&plan, TaskStartOperation::Start),
            activation_fact_for(&plan, TaskStartOperation::TryStart),
        ],
    };
    let mut gate = gate(&[plan.candidate().stack_plan], 380);

    // Both facts share one plan identity; the receipt's operation selects
    // the exact one it binds.
    gate.admit_pending(
        &activations,
        receipt_candidate(&plan, instance(380), 381, 382, TaskStartOperation::TryStart),
        activation(383),
        moved_arguments(&plan, 384),
    )
    .expect("a try_start receipt binds the try_start fact");
    let record = gate.records().next().expect("one live dependency");
    assert_eq!(record.operation, TaskStartOperation::TryStart);
}

#[test]
fn caller_storage_rejection_returns_the_supplied_lease() {
    let plan = wcsu_plan(57);
    let activations = activation_set(&plan);
    let mut gate = gate(&[], 390);

    gate.admit_with_storage(
        &activations,
        receipt_candidate(&plan, instance(390), 391, 392, TaskStartOperation::Start),
        activation(393),
        moved_arguments(&plan, 394),
        TaskStartStorage::Persistent(stack_lease(&plan, 395, 396)),
    )
    .expect("first caller-supplied admission");

    // Replaying the activation identity rejects through the ledger's own
    // conserving carrier: the supplied lease returns to its owner.
    let rejected_custody = custody(397);
    let rejection = gate
        .admit_with_storage(
            &activations,
            receipt_candidate(&plan, instance(390), 398, 399, TaskStartOperation::Start),
            activation(393),
            MovedTaskArguments::new(plan.candidate().argument_layout, rejected_custody),
            TaskStartStorage::Persistent(stack_lease(&plan, 400, 401)),
        )
        .expect_err("a replayed activation identity rejects");
    assert!(
        rejection.diagnostic().0.contains("already been accepted"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );
    let (arguments, storage) = rejection.into_custody();
    assert_eq!(arguments.custody(), rejected_custody);
    let TaskStartStorage::Persistent(lease) = storage else {
        panic!("the caller-supplied lease returns whole");
    };
    assert_eq!(
        lease.provenance(),
        TaskStorageProvenance {
            owner: id(400, TaskStorageOwnerId::from_normalized_identity),
            lease: id(401, crate::TaskStorageLeaseId::from_normalized_identity),
        }
    );
}

#[test]
fn cancelled_settlement_through_the_gate_requires_the_recorded_request() {
    let plan = wcsu_plan(59);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 420);

    let claim = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(420), 421, 422, TaskStartOperation::Start),
            activation(423),
            moved_arguments(&plan, 424),
        )
        .expect("pending admission");

    // A fabricated cancelled outcome rejects and returns the claim; the
    // leased slot must not leak back into the free set.
    let claim = gate
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect_err("cancelled without a request rejects")
        .into_claim();
    gate.admit_pending(
        &activations,
        receipt_candidate(&plan, instance(420), 425, 426, TaskStartOperation::Start),
        activation(427),
        moved_arguments(&plan, 428),
    )
    .expect_err("the rejected settlement did not release the leased slot");

    gate.request_cancellation(&claim)
        .expect("the provider records the request");
    gate.settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("the recorded request authorizes the cancelled settlement");

    // Settlement returned the backing to the pool under a fresh era.
    gate.admit_pending(
        &activations,
        receipt_candidate(&plan, instance(420), 429, 430, TaskStartOperation::Start),
        activation(431),
        moved_arguments(&plan, 432),
    )
    .expect("the released slot admits again");
}

#[test]
fn malformed_provisioning_rejects_at_construction() {
    let plan = wcsu_plan(58);
    let mut malformed = plan.candidate().stack_plan;
    malformed.alignment = 3;
    let diagnostic = TaskRuntimeAdmission::new(runtime(), instance(410), owner(), vec![malformed])
        .expect_err("a malformed provisioned slot rejects at construction");
    assert!(diagnostic.0.contains("power of two"));
}
