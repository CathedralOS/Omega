use super::{
    candidate, canonical_crossing, id, invocation_receipt, moved_arguments, runtime, stack_lease,
    wcsu_plan,
};
use crate::{
    ActivationInstanceId, TaskLifecycleLedger, TaskRuntimeInstanceId, TaskSettlementOutcome,
    TaskStartOperation, TaskStartStorage, TaskStorageBinding, TaskStorageLeaseId,
    TaskStorageOwnerId, TaskStorageProvenance, validate_activation_plan,
};

#[test]
fn lifecycle_claim_pins_runtime_plan_and_storage_until_settlement() {
    let plan = wcsu_plan(31);
    let instance = id(100, TaskRuntimeInstanceId::from_normalized_identity);
    let activation = id(101, ActivationInstanceId::from_normalized_identity);
    let storage = TaskStorageProvenance {
        owner: id(102, TaskStorageOwnerId::from_normalized_identity),
        lease: id(103, TaskStorageLeaseId::from_normalized_identity),
    };
    let receipt = invocation_receipt(&plan, instance, 104, 105);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &receipt,
            activation,
            moved_arguments(&plan, 106),
            TaskStartStorage::Persistent(stack_lease(&plan, 102, 103)),
        )
        .expect("accepted activation");

    let record = ledger.records().next().expect("one live dependency");
    assert_eq!(
        record.executor_selection,
        receipt.executor_selection().identity()
    );
    assert_eq!(record.invocation, receipt.candidate().invocation);
    assert_eq!(record.invocation_receipt, receipt.candidate().receipt);
    assert_eq!(record.invocation_binding, receipt.identity());
    assert_eq!(record.operation, TaskStartOperation::Start);
    assert!(!ledger.cancellation_requested(claim.identity()));
    ledger
        .request_cancellation(&claim)
        .expect("cancellation preserves the claim");
    assert!(ledger.cancellation_requested(claim.identity()));
    ledger
        .observe_cancellation(&claim, canonical_crossing())
        .expect("the activation observes the request at a canonical safe point");
    assert!(ledger.validate_storage_reclaim(storage).is_err());
    let close = ledger.close().expect_err("live child blocks runtime close");
    let mut ledger = close.into_ledger();
    let settled = ledger
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("terminal settlement");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Cancelled);
    assert_eq!(
        settled.released_storage(),
        TaskStorageBinding::Persistent(storage)
    );
    assert_eq!(
        settled
            .into_released_lease()
            .expect("persistent activation held a stack lease")
            .provenance(),
        storage,
        "settlement releases the exact retained lease authority"
    );
    ledger
        .validate_storage_reclaim(storage)
        .expect("settlement releases storage");
    assert_eq!(ledger.close().expect("runtime closes").runtime(), runtime());
}

#[test]
fn lifecycle_rejects_replayed_activation_and_storage_eras() {
    let plan = wcsu_plan(33);
    let instance = id(110, TaskRuntimeInstanceId::from_normalized_identity);
    let first_activation = id(111, ActivationInstanceId::from_normalized_identity);
    let second_activation = id(112, ActivationInstanceId::from_normalized_identity);
    let receipt = invocation_receipt(&plan, instance, 116, 117);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &receipt,
            first_activation,
            moved_arguments(&plan, 125),
            TaskStartStorage::Persistent(stack_lease(&plan, 113, 114)),
        )
        .expect("first activation");

    let replayed_activation_receipt = invocation_receipt(&plan, instance, 118, 119);
    assert!(
        ledger
            .accept_invocation(
                &replayed_activation_receipt,
                first_activation,
                moved_arguments(&plan, 126),
                TaskStartStorage::Persistent(stack_lease(&plan, 113, 115)),
            )
            .expect_err("activation replay")
            .diagnostic()
            .0
            .contains("already been accepted")
    );
    let replayed_storage_receipt = invocation_receipt(&plan, instance, 120, 121);
    assert!(
        ledger
            .accept_invocation(
                &replayed_storage_receipt,
                second_activation,
                moved_arguments(&plan, 127),
                TaskStartStorage::Persistent(stack_lease(&plan, 113, 114)),
            )
            .expect_err("lease replay")
            .diagnostic()
            .0
            .contains("new lease era")
    );
    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("settle first activation");
    let post_settlement_receipt = invocation_receipt(&plan, instance, 122, 123);
    assert!(
        ledger
            .accept_invocation(
                &post_settlement_receipt,
                second_activation,
                moved_arguments(&plan, 128),
                TaskStartStorage::Persistent(stack_lease(&plan, 113, 114)),
            )
            .is_err()
    );
}

#[test]
fn lifecycle_rejects_replayed_invocations_and_provider_receipts() {
    let plan = validate_activation_plan(candidate()).expect("activation plan");
    let instance = id(130, TaskRuntimeInstanceId::from_normalized_identity);
    let first = invocation_receipt(&plan, instance, 131, 132);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &first,
            id(133, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 139),
            TaskStartStorage::InlineCompletion,
        )
        .expect("first invocation");

    assert!(
        ledger
            .accept_invocation(
                &first,
                id(134, ActivationInstanceId::from_normalized_identity),
                moved_arguments(&plan, 140),
                TaskStartStorage::InlineCompletion,
            )
            .expect_err("invocation replay")
            .diagnostic()
            .0
            .contains("invocation identity has already been accepted")
    );

    let mut repeated_receipt = invocation_receipt(&plan, instance, 135, 136);
    repeated_receipt.candidate.receipt = first.candidate().receipt;
    assert!(
        ledger
            .accept_invocation(
                &repeated_receipt,
                id(137, ActivationInstanceId::from_normalized_identity),
                moved_arguments(&plan, 141),
                TaskStartStorage::InlineCompletion,
            )
            .expect_err("provider receipt replay")
            .diagnostic()
            .0
            .contains("invocation receipt has already been accepted")
    );

    ledger
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect("settle first invocation");
    assert!(
        ledger
            .accept_invocation(
                &first,
                id(138, ActivationInstanceId::from_normalized_identity),
                moved_arguments(&plan, 142),
                TaskStartStorage::InlineCompletion,
            )
            .is_err(),
        "settlement does not make a provider invocation receipt replayable"
    );
}

#[test]
fn failed_cross_runtime_settlement_returns_the_linear_claim() {
    let plan = validate_activation_plan(candidate()).expect("activation plan");
    let owner_instance = id(120, TaskRuntimeInstanceId::from_normalized_identity);
    let mut owner = TaskLifecycleLedger::new(runtime(), owner_instance);
    let mut wrong_instance = TaskLifecycleLedger::new(
        runtime(),
        id(121, TaskRuntimeInstanceId::from_normalized_identity),
    );
    let receipt = invocation_receipt(&plan, owner_instance, 124, 125);
    let claim = owner
        .accept_invocation(
            &receipt,
            id(122, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 126),
            TaskStartStorage::InlineCompletion,
        )
        .expect("accepted activation");
    let error = wrong_instance
        .settle(claim, TaskSettlementOutcome::Completed)
        .expect_err("another runtime instance cannot settle this claim");
    owner
        .settle(error.into_claim(), TaskSettlementOutcome::Completed)
        .expect("failed settlement preserves the claim");
}
