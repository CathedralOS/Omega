use super::{candidate, id, invocation_receipt, runtime};
use crate::{
    ActivationInstanceId, TaskLifecycleLedger, TaskRuntimeInstanceId, TaskStartOperation,
    TaskStorageBinding, TaskStorageLeaseId, TaskStorageOwnerId, TaskStorageProvenance,
    validate_activation_plan,
};

#[test]
fn lifecycle_claim_pins_runtime_plan_and_storage_until_settlement() {
    let plan = validate_activation_plan(candidate()).expect("activation plan");
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
            TaskStorageBinding::Persistent(storage),
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
    ledger
        .validate_cancellation_request(&claim)
        .expect("cancellation preserves the claim");
    assert!(ledger.validate_storage_reclaim(storage).is_err());
    let close = ledger.close().expect_err("live child blocks runtime close");
    let mut ledger = close.into_ledger();
    let settled = ledger.settle(claim).expect("terminal settlement");
    assert_eq!(
        settled.released_storage(),
        TaskStorageBinding::Persistent(storage)
    );
    ledger
        .validate_storage_reclaim(storage)
        .expect("settlement releases storage");
    assert_eq!(ledger.close().expect("runtime closes").runtime(), runtime());
}

#[test]
fn lifecycle_rejects_replayed_activation_and_storage_eras() {
    let plan = validate_activation_plan(candidate()).expect("activation plan");
    let instance = id(110, TaskRuntimeInstanceId::from_normalized_identity);
    let first_activation = id(111, ActivationInstanceId::from_normalized_identity);
    let second_activation = id(112, ActivationInstanceId::from_normalized_identity);
    let storage = TaskStorageProvenance {
        owner: id(113, TaskStorageOwnerId::from_normalized_identity),
        lease: id(114, TaskStorageLeaseId::from_normalized_identity),
    };
    let alternate_storage = TaskStorageProvenance {
        owner: storage.owner,
        lease: id(115, TaskStorageLeaseId::from_normalized_identity),
    };
    let receipt = invocation_receipt(&plan, instance, 116, 117);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let claim = ledger
        .accept_invocation(
            &receipt,
            first_activation,
            TaskStorageBinding::Persistent(storage),
        )
        .expect("first activation");

    let replayed_activation_receipt = invocation_receipt(&plan, instance, 118, 119);
    assert!(
        ledger
            .accept_invocation(
                &replayed_activation_receipt,
                first_activation,
                TaskStorageBinding::Persistent(alternate_storage),
            )
            .expect_err("activation replay")
            .0
            .contains("already been accepted")
    );
    let replayed_storage_receipt = invocation_receipt(&plan, instance, 120, 121);
    assert!(
        ledger
            .accept_invocation(
                &replayed_storage_receipt,
                second_activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect_err("lease replay")
            .0
            .contains("new lease era")
    );
    ledger.settle(claim).expect("settle first activation");
    let post_settlement_receipt = invocation_receipt(&plan, instance, 122, 123);
    assert!(
        ledger
            .accept_invocation(
                &post_settlement_receipt,
                second_activation,
                TaskStorageBinding::Persistent(storage),
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
            TaskStorageBinding::InlineCompletion,
        )
        .expect("first invocation");

    assert!(
        ledger
            .accept_invocation(
                &first,
                id(134, ActivationInstanceId::from_normalized_identity),
                TaskStorageBinding::InlineCompletion,
            )
            .expect_err("invocation replay")
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
                TaskStorageBinding::InlineCompletion,
            )
            .expect_err("provider receipt replay")
            .0
            .contains("invocation receipt has already been accepted")
    );

    ledger.settle(claim).expect("settle first invocation");
    assert!(
        ledger
            .accept_invocation(
                &first,
                id(138, ActivationInstanceId::from_normalized_identity),
                TaskStorageBinding::InlineCompletion,
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
            TaskStorageBinding::InlineCompletion,
        )
        .expect("accepted activation");
    let error = wrong_instance
        .settle(claim)
        .expect_err("another runtime instance cannot settle this claim");
    owner
        .settle(error.into_claim())
        .expect("failed settlement preserves the claim");
}
