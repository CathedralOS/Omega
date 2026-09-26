//! Cancellation execution: the recorded `request_cancel` transition, the
//! safe-point observation it enables, and the outcome-bound settlement
//! observation authorizes.
//!
//! `request_cancellation` is transactional, not a read-only probe: it writes
//! the request onto the exact live claim's dependency while retaining the
//! claim, and `settle` reports `Cancelled` only after `observe_cancellation`
//! records the activation observing that request at a canonical safe point.
//! Cooperative semantics stay intact — a recorded request never forces the
//! cancelled outcome, a never-suspending activation has no safe point to
//! observe at, and an activation that completed inline can never report one.

use super::{
    canonical_crossing, id, invocation_receipt, moved_arguments, runtime, stack_lease, wcsu_plan,
};
use crate::{
    ActivationInstanceId, TaskLifecycleClaimId, TaskLifecycleLedger, TaskRuntimeInstanceId,
    TaskSettlementOutcome, TaskStartStorage,
};

fn instance(identity: u64) -> TaskRuntimeInstanceId {
    id(identity, TaskRuntimeInstanceId::from_normalized_identity)
}

fn activation(identity: u64) -> ActivationInstanceId {
    id(identity, ActivationInstanceId::from_normalized_identity)
}

#[test]
fn cancelled_settlement_requires_the_recorded_request() {
    let plan = wcsu_plan(61);
    let instance = instance(500);
    let receipt = invocation_receipt(&plan, instance, 501, 502);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &receipt,
            activation(503),
            moved_arguments(&plan, 504),
            TaskStartStorage::Persistent(stack_lease(&plan, 505, 506)),
        )
        .expect("accepted activation");

    // Without a recorded request a cancelled outcome is fabricated: the
    // rejection returns the claim and the record stays live.
    let error = ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect_err("a cancelled outcome cannot be fabricated");
    assert!(
        error
            .diagnostic()
            .0
            .contains("without a recorded cancellation request"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );
    let claim = error.into_claim();
    assert_eq!(ledger.records().count(), 1, "the claim survives");

    ledger
        .request_cancellation(&claim)
        .expect("the recorded request retains the claim");

    // A recorded request alone is still not the observation: settlement
    // rejects until the activation observes the request at a canonical
    // safe point.
    let error = ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect_err("an unobserved request cannot settle cancelled");
    assert!(
        error.diagnostic().0.contains("safe-point observation"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );
    let claim = error.into_claim();
    assert_eq!(ledger.records().count(), 1, "the claim survives");

    ledger
        .observe_cancellation(&claim, canonical_crossing())
        .expect("the activation observes the request at a canonical safe point");
    let settled = ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("the recorded observation authorizes the cancelled outcome");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Cancelled);
    assert!(
        !ledger.cancellation_requested(settled.identity()),
        "settlement removes the record"
    );
}

#[test]
fn completed_settlement_survives_an_unobserved_request() {
    let plan = wcsu_plan(62);
    let instance = instance(510);
    let receipt = invocation_receipt(&plan, instance, 511, 512);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &receipt,
            activation(513),
            moved_arguments(&plan, 514),
            TaskStartStorage::Persistent(stack_lease(&plan, 515, 516)),
        )
        .expect("accepted activation");

    // Cooperative cancellation: the request does not promise the activation
    // observes it, so an ordinary Returned/Failed settlement is still valid.
    ledger
        .request_cancellation(&claim)
        .expect("request records");
    let settled = ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("completion may ignore an unobserved request");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Completed);
}

#[test]
fn inline_completion_can_never_settle_cancelled() {
    let plan = wcsu_plan(63);
    let instance = instance(520);
    let receipt = invocation_receipt(&plan, instance, 521, 522);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &receipt,
            activation(523),
            moved_arguments(&plan, 524),
            TaskStartStorage::InlineCompletion,
        )
        .expect("accepted inline completion");

    // The request is recordable — `request_cancel(&self)` is legal on any
    // live claim — but the activation finished before the claim existed, so
    // a cancelled outcome can never be honest here.
    ledger
        .request_cancellation(&claim)
        .expect("the request still records on a live claim");
    let error = ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect_err("an inline completion cannot settle cancelled");
    assert!(
        error.diagnostic().0.contains("inline completion"),
        "unexpected diagnostic: {}",
        error.diagnostic().0
    );
    let settled = ledger
        .settle(error.into_claim(), TaskSettlementOutcome::Completed)
        .expect("the inline completion settles its ordinary outcome");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Completed);
}

#[test]
fn cancellation_request_rejects_foreign_and_unknown_claims() {
    let plan = wcsu_plan(64);
    let owner_instance = instance(530);
    let foreign_instance = instance(531);
    let receipt = invocation_receipt(&plan, owner_instance, 532, 533);
    let mut owner = TaskLifecycleLedger::new(runtime(), owner_instance);
    let mut foreign = TaskLifecycleLedger::new(runtime(), foreign_instance);
    let claim = owner
        .accept_invocation(
            &receipt,
            activation(534),
            moved_arguments(&plan, 535),
            TaskStartStorage::InlineCompletion,
        )
        .expect("accepted activation");

    let diagnostic = foreign
        .request_cancellation(&claim)
        .expect_err("a foreign instance cannot request cancellation");
    assert!(diagnostic.0.contains("exact live task lifecycle claim"));
    assert!(
        !foreign.cancellation_requested(claim.identity()),
        "the foreign ledger recorded nothing"
    );

    // An unknown claim identity reports no request rather than fabricating
    // live state.
    let unknown = id(536, TaskLifecycleClaimId::from_normalized_identity);
    assert!(!owner.cancellation_requested(unknown));

    owner
        .request_cancellation(&claim)
        .expect("the owning instance records the request");
    assert!(owner.cancellation_requested(claim.identity()));
}

#[test]
fn repeated_requests_and_close_still_require_settlement() {
    let plan = wcsu_plan(65);
    let instance = instance(540);
    let receipt = invocation_receipt(&plan, instance, 541, 542);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &receipt,
            activation(543),
            moved_arguments(&plan, 544),
            TaskStartStorage::Persistent(stack_lease(&plan, 545, 546)),
        )
        .expect("accepted activation");

    ledger
        .request_cancellation(&claim)
        .expect("first request records");
    ledger
        .request_cancellation(&claim)
        .expect("a repeated request records the same fact");

    // A request is not a settlement: close and storage reclaim still reject
    // while the claim lives.
    let close = ledger.close().expect_err("a requested claim is still live");
    let mut ledger = close.into_ledger();
    ledger
        .observe_cancellation(&claim, canonical_crossing())
        .expect("the request is observed at a canonical safe point");
    ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("the claim settles through its observed request");
    ledger.close().expect("settled claims permit close");
}
