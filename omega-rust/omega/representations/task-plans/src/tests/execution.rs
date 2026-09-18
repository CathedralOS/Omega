//! Execution state of live activations: park/resume at canonical
//! suspension crossings, settlement waiting on a terminated activation,
//! and the safe-point observation cooperative cancellation requires.
//!
//! Parking records the exact canonical crossing the invocation suspended
//! at and changes nothing else — the claim, its receipt binding and its
//! retained lease all survive the interval. A parked claim cannot settle:
//! resumption continues the same invocation before any terminal outcome.
//! `observe_cancellation` binds the observation to a plan crossing, so a
//! `Cancelled` outcome can only be reported where the plan declares a safe
//! point — and never by a never-suspending or inline-completed activation.

use super::{
    activation_set, candidate, canonical_crossing, id, invocation_receipt, moved_arguments,
    receipt_candidate, runtime, stack_lease, wcsu_plan, wcsu_projection,
};
use crate::{
    ActivationCarryObligations, ActivationInstanceId, CanonicalSuspensionCrossing,
    SuspensionCrossingId, TaskLifecycleClaim, TaskLifecycleLedger, TaskRuntimeAdmission,
    TaskRuntimeInstanceId, TaskSettlementOutcome, TaskStartOperation, TaskStartStorage,
    TaskStorageOwnerId, ValidatedActivationPlan, validate_wcsu_activation_plan,
};

fn instance(identity: u64) -> TaskRuntimeInstanceId {
    id(identity, TaskRuntimeInstanceId::from_normalized_identity)
}

fn activation(identity: u64) -> ActivationInstanceId {
    id(identity, ActivationInstanceId::from_normalized_identity)
}

fn crossing(identity: u64) -> SuspensionCrossingId {
    SuspensionCrossingId::new(identity).expect("nonzero crossing identity")
}

/// A WCSU-backed plan that declares no safe point: `may_suspend` is false
/// and the canonical roster is empty, so nothing can park or observe.
fn non_suspending_plan(validation_identity: u64) -> ValidatedActivationPlan {
    let projection = wcsu_projection(validation_identity);
    let mut candidate = candidate();
    candidate.may_suspend = false;
    candidate.canonical_suspension_crossings = Vec::new();
    candidate.carry_obligations = ActivationCarryObligations::none();
    candidate.stack_plan = projection.stack_plan();
    validate_wcsu_activation_plan(candidate, projection)
        .expect("WCSU-backed non-suspending activation plan")
}

/// A WCSU-backed plan whose canonical roster declares two safe points.
fn two_crossing_plan(validation_identity: u64) -> ValidatedActivationPlan {
    let projection = wcsu_projection(validation_identity);
    let mut candidate = candidate();
    candidate
        .canonical_suspension_crossings
        .push(CanonicalSuspensionCrossing {
            identity: crossing(8),
            suspension_allowed: true,
            preserve_cpu: false,
            preserve_host_thread: false,
        });
    candidate.stack_plan = projection.stack_plan();
    validate_wcsu_activation_plan(candidate, projection)
        .expect("WCSU-backed two-crossing activation plan")
}

fn start_persistent(
    ledger: &mut TaskLifecycleLedger,
    plan: &ValidatedActivationPlan,
    instance: TaskRuntimeInstanceId,
    identities: (u64, u64, u64, u64, u64),
) -> TaskLifecycleClaim {
    let (activation, invocation, receipt, custody, lease) = identities;
    ledger
        .accept_invocation(
            &invocation_receipt(plan, instance, invocation, receipt),
            activation_id(activation),
            moved_arguments(plan, custody),
            TaskStartStorage::Persistent(stack_lease(plan, 600, lease)),
        )
        .expect("accepted activation")
}

fn activation_id(identity: u64) -> ActivationInstanceId {
    id(identity, ActivationInstanceId::from_normalized_identity)
}

#[test]
fn parked_activation_resumes_the_same_invocation_unchanged() {
    let plan = wcsu_plan(70);
    let instance = instance(600);
    let receipt = invocation_receipt(&plan, instance, 601, 602);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &receipt,
            activation(603),
            moved_arguments(&plan, 604),
            TaskStartStorage::Persistent(stack_lease(&plan, 605, 606)),
        )
        .expect("accepted activation");

    assert_eq!(ledger.parked_crossing(claim.identity()), None);
    ledger
        .park(&claim, canonical_crossing())
        .expect("a canonical crossing accepts the park");
    assert_eq!(
        ledger.parked_crossing(claim.identity()),
        Some(canonical_crossing())
    );

    // Parking establishes no result or cleanup edge: the dependency record
    // is exactly the one the start issued.
    let record = ledger.records().next().expect("one live dependency");
    assert_eq!(record.invocation, receipt.candidate().invocation);
    assert_eq!(record.activation, activation(603));

    assert_eq!(
        ledger
            .resume(&claim)
            .expect("resume continues the invocation"),
        canonical_crossing()
    );
    assert_eq!(ledger.parked_crossing(claim.identity()), None);
    let record = ledger.records().next().expect("one live dependency");
    assert_eq!(
        record.invocation,
        receipt.candidate().invocation,
        "resumption continues the same invocation"
    );
    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("the resumed activation settles");
    ledger.close().expect("no live claims remain");
}

#[test]
fn park_rejects_crossings_outside_the_canonical_roster() {
    let plan = wcsu_plan(71);
    let instance = instance(610);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = start_persistent(&mut ledger, &plan, instance, (611, 612, 613, 614, 615));

    let diagnostic = ledger
        .park(&claim, crossing(999))
        .expect_err("a non-canonical crossing cannot park the activation");
    assert!(
        diagnostic.0.contains("canonical roster"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    assert_eq!(ledger.parked_crossing(claim.identity()), None);
    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("the failed park left the claim running");
}

#[test]
fn never_suspending_activation_cannot_park_observe_or_settle_cancelled() {
    let plan = non_suspending_plan(72);
    let instance = instance(620);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = start_persistent(&mut ledger, &plan, instance, (621, 622, 623, 624, 625));

    let diagnostic = ledger
        .park(&claim, canonical_crossing())
        .expect_err("a never-suspending activation has no crossing to park at");
    assert!(
        diagnostic.0.contains("canonical roster"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );

    // The request itself is legal — `request_cancel(&self)` retains the
    // claim — but no safe point exists to observe it at, so observation
    // rejects and a cancelled outcome can never be honest.
    ledger
        .request_cancellation(&claim)
        .expect("the request still records on a live claim");
    let diagnostic = ledger
        .observe_cancellation(&claim, canonical_crossing())
        .expect_err("no canonical crossing exists to observe at");
    assert!(
        diagnostic.0.contains("canonical suspension crossings"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    let error = ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect_err("a never-suspending activation cannot settle cancelled");
    assert!(
        error.diagnostic().0.contains("safe-point observation"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );
    ledger
        .settle(error.into_claim(), TaskSettlementOutcome::Completed)
        .expect("a never-suspending task remains finishable");
}

#[test]
fn inline_completion_has_no_activation_to_park_or_observe() {
    let plan = wcsu_plan(73);
    let instance = instance(630);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &invocation_receipt(&plan, instance, 631, 632),
            activation(633),
            moved_arguments(&plan, 634),
            TaskStartStorage::InlineCompletion,
        )
        .expect("accepted inline completion");

    let diagnostic = ledger
        .park(&claim, canonical_crossing())
        .expect_err("an inline-completed activation cannot park");
    assert!(
        diagnostic.0.contains("inline-completed"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    ledger
        .request_cancellation(&claim)
        .expect("the request records on the live claim");
    let diagnostic = ledger
        .observe_cancellation(&claim, canonical_crossing())
        .expect_err("an inline-completed activation cannot observe");
    assert!(
        diagnostic.0.contains("inline-completed"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("the inline completion settles its ordinary outcome");
}

#[test]
fn double_park_and_unparked_resume_reject() {
    let plan = wcsu_plan(74);
    let instance = instance(640);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = start_persistent(&mut ledger, &plan, instance, (641, 642, 643, 644, 645));

    let diagnostic = ledger
        .resume(&claim)
        .expect_err("a running activation has nothing to resume");
    assert!(
        diagnostic.0.contains("parked"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    ledger
        .park(&claim, canonical_crossing())
        .expect("first park succeeds");
    let diagnostic = ledger
        .park(&claim, canonical_crossing())
        .expect_err("an already parked activation cannot park again");
    assert!(
        diagnostic.0.contains("already parked"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    ledger
        .resume(&claim)
        .expect("resume continues the invocation");
    let diagnostic = ledger
        .resume(&claim)
        .expect_err("a resumed activation cannot resume again");
    assert!(
        diagnostic.0.contains("parked"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("the claim settles after resume");
}

#[test]
fn settlement_must_wait_for_resume() {
    let plan = wcsu_plan(75);
    let instance = instance(650);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = start_persistent(&mut ledger, &plan, instance, (651, 652, 653, 654, 655));

    ledger
        .park(&claim, canonical_crossing())
        .expect("the activation parks at a canonical crossing");
    // A parked activation holds no terminal outcome: settling would dispose
    // its suspended continuation. The rejection returns the claim whole.
    let error = ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect_err("a parked activation cannot settle");
    assert!(
        error.diagnostic().0.contains("parked"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );
    let claim = error.into_claim();
    assert_eq!(
        ledger.parked_crossing(claim.identity()),
        Some(canonical_crossing()),
        "the rejected settlement left the activation parked"
    );
    ledger.resume(&claim).expect("resume precedes settlement");
    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("the resumed activation settles");
}

#[test]
fn parked_activation_observes_only_at_its_park_crossing() {
    let plan = two_crossing_plan(76);
    let instance = instance(660);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = start_persistent(&mut ledger, &plan, instance, (661, 662, 663, 664, 665));

    ledger
        .request_cancellation(&claim)
        .expect("the request records");
    ledger
        .park(&claim, crossing(7))
        .expect("the activation parks at the first crossing");

    // Suspended at crossing 7, the activation cannot observe at crossing 8.
    let diagnostic = ledger
        .observe_cancellation(&claim, crossing(8))
        .expect_err("a parked activation observes only at its park crossing");
    assert!(
        diagnostic.0.contains("park crossing"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    ledger
        .observe_cancellation(&claim, crossing(7))
        .expect("the parked crossing accepts the observation");
    assert_eq!(
        ledger.cancellation_observed(claim.identity()),
        Some(crossing(7))
    );

    ledger
        .resume(&claim)
        .expect("the cancelled activation resumes to unwind");
    ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("the observed request settles cancelled");
}

#[test]
fn observation_requires_the_recorded_request() {
    let plan = wcsu_plan(77);
    let instance = instance(670);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = start_persistent(&mut ledger, &plan, instance, (671, 672, 673, 674, 675));

    let diagnostic = ledger
        .observe_cancellation(&claim, canonical_crossing())
        .expect_err("an activation cannot observe a request never made");
    assert!(
        diagnostic.0.contains("recorded cancellation"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
    ledger
        .request_cancellation(&claim)
        .expect("the request records");
    ledger
        .observe_cancellation(&claim, canonical_crossing())
        .expect("a running activation may observe at a canonical safe point");
    ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("the observed request settles cancelled");
}

#[test]
fn foreign_ledger_cannot_park_resume_or_observe() {
    let plan = wcsu_plan(78);
    let owner_instance = instance(680);
    let foreign_instance = instance(681);
    let mut owner = TaskLifecycleLedger::new(runtime(), owner_instance);
    let mut foreign = TaskLifecycleLedger::new(runtime(), foreign_instance);
    let claim = start_persistent(&mut owner, &plan, owner_instance, (682, 683, 684, 685, 686));

    assert!(
        foreign
            .park(&claim, canonical_crossing())
            .expect_err("a foreign instance cannot park the claim")
            .0
            .contains("exact live task lifecycle claim")
    );
    assert!(
        foreign
            .resume(&claim)
            .expect_err("a foreign instance cannot resume the claim")
            .0
            .contains("exact live task lifecycle claim")
    );
    owner
        .request_cancellation(&claim)
        .expect("the owning instance records the request");
    assert!(
        foreign
            .observe_cancellation(&claim, canonical_crossing())
            .expect_err("a foreign instance cannot record an observation")
            .0
            .contains("exact live task lifecycle claim")
    );
    assert_eq!(foreign.parked_crossing(claim.identity()), None);
    assert_eq!(foreign.cancellation_observed(claim.identity()), None);
    owner
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("the owning instance settles");
}

#[test]
fn concurrent_activations_park_resume_and_settle_independently() {
    let plan = wcsu_plan(79);
    let instance = instance(690);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let first = start_persistent(&mut ledger, &plan, instance, (691, 692, 693, 694, 695));
    let second = start_persistent(&mut ledger, &plan, instance, (696, 697, 698, 699, 700));
    assert_eq!(ledger.records().count(), 2);

    // One activation suspends while the other keeps running; each claim's
    // execution state is its own.
    ledger
        .park(&first, canonical_crossing())
        .expect("first activation parks");
    assert_eq!(
        ledger.parked_crossing(first.identity()),
        Some(canonical_crossing())
    );
    assert_eq!(ledger.parked_crossing(second.identity()), None);

    // The running activation finishes while the first stays parked.
    ledger
        .settle(second, TaskSettlementOutcome::Completed)
        .expect("the running activation settles while the first is parked");

    // The parked activation observes its recorded request, resumes the same
    // invocation, and settles cancelled.
    ledger
        .request_cancellation(&first)
        .expect("the request records on the parked claim");
    ledger
        .observe_cancellation(&first, canonical_crossing())
        .expect("observation at the park crossing");
    ledger.resume(&first).expect("resume to unwind");
    let settled = ledger
        .settle(first, TaskSettlementOutcome::Cancelled)
        .expect("the observed activation settles cancelled");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Cancelled);
    ledger.close().expect("all claims settled");
}

#[test]
fn admission_gate_parks_resumes_and_observes_through_the_ledger() {
    let plan = wcsu_plan(80);
    let activations = activation_set(&plan);
    let mut gate = TaskRuntimeAdmission::new(
        runtime(),
        instance(700),
        id(701, TaskStorageOwnerId::from_normalized_identity),
        vec![plan.candidate().stack_plan],
    )
    .expect("provisioned admission gate");

    let claim = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(700), 702, 703, TaskStartOperation::Start),
            activation(704),
            moved_arguments(&plan, 705),
        )
        .expect("pending admission");

    gate.park(&claim, canonical_crossing())
        .expect("the gate forwards the park");
    assert_eq!(
        gate.parked_crossing(claim.identity()),
        Some(canonical_crossing())
    );
    gate.request_cancellation(&claim)
        .expect("the request records");
    gate.observe_cancellation(&claim, canonical_crossing())
        .expect("the gate forwards the observation");
    assert_eq!(
        gate.cancellation_observed(claim.identity()),
        Some(canonical_crossing())
    );
    assert_eq!(
        gate.resume(&claim).expect("the gate forwards the resume"),
        canonical_crossing()
    );
    let settled = gate
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("the observed activation settles cancelled");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Cancelled);
    gate.close().expect("empty gate closes");
}

#[test]
fn settlement_releases_pool_backing_only_after_resume() {
    let plan = wcsu_plan(81);
    let activations = activation_set(&plan);
    let mut gate = TaskRuntimeAdmission::new(
        runtime(),
        instance(710),
        id(711, TaskStorageOwnerId::from_normalized_identity),
        vec![plan.candidate().stack_plan],
    )
    .expect("single-slot pool");

    let claim = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(710), 712, 713, TaskStartOperation::Start),
            activation(714),
            moved_arguments(&plan, 715),
        )
        .expect("first admission spends the slot");
    gate.park(&claim, canonical_crossing())
        .expect("the activation parks");

    // A rejected settle while parked returns the claim and frees nothing:
    // the pool stays exhausted for the next start.
    let claim = gate
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect_err("a parked activation cannot settle")
        .into_claim();
    gate.admit_pending(
        &activations,
        receipt_candidate(&plan, instance(710), 716, 717, TaskStartOperation::Start),
        activation(718),
        moved_arguments(&plan, 719),
    )
    .expect_err("the rejected settlement did not release the leased slot");

    gate.resume(&claim).expect("resume precedes settlement");
    gate.settle(claim, TaskSettlementOutcome::Completed)
        .expect("the resumed activation settles");
    let next = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(710), 720, 721, TaskStartOperation::Start),
            activation(722),
            moved_arguments(&plan, 723),
        )
        .expect("settlement returned the slot under a fresh era");
    gate.settle(next, TaskSettlementOutcome::Completed)
        .expect("the second admission settles");
    gate.close().expect("empty gate closes");
}

#[test]
fn parked_claim_blocks_close_until_resumed_and_settled() {
    let plan = wcsu_plan(82);
    let instance = instance(730);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = start_persistent(&mut ledger, &plan, instance, (731, 732, 733, 734, 735));

    ledger
        .park(&claim, canonical_crossing())
        .expect("the activation parks");
    let close = ledger
        .close()
        .expect_err("a parked claim is still a live claim");
    let mut ledger = close.into_ledger();
    ledger.resume(&claim).expect("resume");
    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("settle");
    ledger.close().expect("settled claims permit close");
}
