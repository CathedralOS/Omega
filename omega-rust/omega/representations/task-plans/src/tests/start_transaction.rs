use super::{candidate, id, invocation_receipt, moved_arguments, runtime, stack_lease, wcsu_plan};
use crate::{
    ActivationInstanceId, MovedTaskArguments, StackLeaseBacking, StackPlan, StackRepresentationId,
    TaskArgumentCustodyId, TaskLifecycleLedger, TaskRuntimeInstanceId, TaskSettlementOutcome,
    TaskStartStorage, TaskStorageLeaseId, TaskStorageOwnerId, TaskStorageProvenance, ValueLayoutId,
    establish_stack_lease, validate_activation_plan,
};

fn provenance(owner: u64, lease: u64) -> TaskStorageProvenance {
    TaskStorageProvenance {
        owner: id(owner, TaskStorageOwnerId::from_normalized_identity),
        lease: id(lease, TaskStorageLeaseId::from_normalized_identity),
    }
}

#[test]
fn stack_lease_requires_sealed_wcsu_evidence_and_satisfying_backing() {
    let plan = wcsu_plan(41);
    let bridged = validate_activation_plan(candidate()).expect("local-bridge plan");

    assert!(
        establish_stack_lease(
            &bridged,
            StackLeaseBacking {
                provenance: provenance(200, 201),
                backing: bridged.candidate().stack_plan,
            },
        )
        .expect_err("a numeric fit alone supplies no StackLease")
        .0
        .contains("numeric fit")
    );

    let required = plan.candidate().stack_plan;
    assert!(
        establish_stack_lease(
            &plan,
            StackLeaseBacking {
                provenance: provenance(200, 202),
                backing: StackPlan {
                    bytes: required.bytes - 1,
                    ..required
                },
            },
        )
        .expect_err("undersized backing")
        .0
        .contains("does not cover")
    );
    assert!(
        establish_stack_lease(
            &plan,
            StackLeaseBacking {
                provenance: provenance(200, 203),
                backing: StackPlan {
                    alignment: required.alignment / 2,
                    ..required
                },
            },
        )
        .expect_err("understated alignment")
        .0
        .contains("understates")
    );
    assert!(
        establish_stack_lease(
            &plan,
            StackLeaseBacking {
                provenance: provenance(200, 204),
                backing: StackPlan {
                    alignment: required.alignment + 8,
                    ..required
                },
            },
        )
        .expect_err("non-power-of-two alignment")
        .0
        .contains("nonzero power of two")
    );
    assert!(
        establish_stack_lease(
            &plan,
            StackLeaseBacking {
                provenance: provenance(200, 205),
                backing: StackPlan {
                    representation: id(206, StackRepresentationId::from_normalized_identity),
                    ..required
                },
            },
        )
        .expect_err("foreign stack representation")
        .0
        .contains("representation")
    );

    // An over-provisioned pool slot satisfies the same plan and binds its
    // exact identity.
    let pool_backing = StackPlan {
        bytes: required.bytes * 2,
        ..required
    };
    let lease = establish_stack_lease(
        &plan,
        StackLeaseBacking {
            provenance: provenance(200, 207),
            backing: pool_backing,
        },
    )
    .expect("pool slot satisfies the exact plan");
    assert_eq!(lease.provenance(), provenance(200, 207));
    assert_eq!(lease.activation_plan(), plan.normalized_identity());
    assert_eq!(lease.satisfied_plan(), required);
    assert_eq!(lease.backing(), pool_backing);
}

#[test]
fn start_rejection_returns_every_moved_argument_and_the_lease() {
    let plan = wcsu_plan(43);
    let instance = id(210, TaskRuntimeInstanceId::from_normalized_identity);
    let activation = id(211, ActivationInstanceId::from_normalized_identity);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let receipt = invocation_receipt(&plan, instance, 212, 213);

    // Argument-layout drift: the rejection conserves the whole bundle and
    // the supplied lease.
    let wrong_layout = id(214, ValueLayoutId::from_normalized_identity);
    let custody = id(215, TaskArgumentCustodyId::from_normalized_identity);
    let rejection = ledger
        .accept_invocation(
            &receipt,
            activation,
            MovedTaskArguments::new(wrong_layout, custody),
            TaskStartStorage::Persistent(stack_lease(&plan, 216, 217)),
        )
        .expect_err("argument layout drift rejects");
    assert!(rejection.diagnostic().0.contains("argument layout"));
    assert_eq!(rejection.activation(), activation);
    let (arguments, storage) = rejection.into_custody();
    assert_eq!(arguments.custody(), custody);
    assert_eq!(arguments.layout(), wrong_layout);
    let TaskStartStorage::Persistent(lease) = storage else {
        panic!("persistent storage custody returns its lease");
    };
    assert_eq!(lease.provenance(), provenance(216, 217));

    // A receipt minted by another runtime instance rejects the same way.
    let foreign_receipt = invocation_receipt(
        &plan,
        id(218, TaskRuntimeInstanceId::from_normalized_identity),
        219,
        220,
    );
    let custody = id(221, TaskArgumentCustodyId::from_normalized_identity);
    let rejection = ledger
        .accept_invocation(
            &foreign_receipt,
            id(222, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 221),
            TaskStartStorage::Persistent(stack_lease(&plan, 223, 224)),
        )
        .expect_err("cross-instance receipt rejects");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("different runtime instance")
    );
    let (arguments, storage) = rejection.into_custody();
    assert_eq!(arguments.custody(), custody);
    let TaskStartStorage::Persistent(lease) = storage else {
        panic!("cross-instance rejection returns the lease");
    };
    assert_eq!(lease.provenance(), provenance(223, 224));

    // A lease established for a different activation plan cannot be offered
    // here; custody still returns whole.
    let other_plan = wcsu_plan(44);
    let custody = id(225, TaskArgumentCustodyId::from_normalized_identity);
    let rejection = ledger
        .accept_invocation(
            &receipt,
            id(226, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 225),
            TaskStartStorage::Persistent(stack_lease(&other_plan, 227, 228)),
        )
        .expect_err("foreign-plan lease rejects");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("different activation plan")
    );
    let (arguments, _) = rejection.into_custody();
    assert_eq!(arguments.custody(), custody);
}

#[test]
fn lease_replay_rejection_still_conserves_custody() {
    let plan = wcsu_plan(45);
    let instance = id(240, TaskRuntimeInstanceId::from_normalized_identity);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let first_receipt = invocation_receipt(&plan, instance, 241, 242);
    ledger
        .accept_invocation(
            &first_receipt,
            id(243, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 244),
            TaskStartStorage::Persistent(stack_lease(&plan, 245, 246)),
        )
        .expect("first activation");

    // A second start presenting the same lease era rejects and returns its
    // own argument bundle and the replayed lease.
    let second_receipt = invocation_receipt(&plan, instance, 247, 248);
    let custody = id(249, TaskArgumentCustodyId::from_normalized_identity);
    let rejection = ledger
        .accept_invocation(
            &second_receipt,
            id(250, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 249),
            TaskStartStorage::Persistent(stack_lease(&plan, 245, 246)),
        )
        .expect_err("stale lease era rejects");
    assert!(rejection.diagnostic().0.contains("new lease era"));
    let (arguments, storage) = rejection.into_custody();
    assert_eq!(arguments.custody(), custody);
    let TaskStartStorage::Persistent(lease) = storage else {
        panic!("lease replay returns the replayed lease");
    };
    assert_eq!(lease.provenance(), provenance(245, 246));
}

#[test]
fn concurrent_claims_settle_independently_and_cross_instance_settlement_returns_the_claim() {
    let plan = wcsu_plan(47);
    let instance = id(260, TaskRuntimeInstanceId::from_normalized_identity);
    let foreign_instance = id(261, TaskRuntimeInstanceId::from_normalized_identity);
    let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
    let mut foreign_ledger = TaskLifecycleLedger::new(runtime(), foreign_instance);

    let first_receipt = invocation_receipt(&plan, instance, 262, 263);
    let second_receipt = invocation_receipt(&plan, instance, 264, 265);
    let first = ledger
        .accept_invocation(
            &first_receipt,
            id(266, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 267),
            TaskStartStorage::Persistent(stack_lease(&plan, 268, 269)),
        )
        .expect("first concurrent start");
    let second = ledger
        .accept_invocation(
            &second_receipt,
            id(270, ActivationInstanceId::from_normalized_identity),
            moved_arguments(&plan, 271),
            TaskStartStorage::InlineCompletion,
        )
        .expect("second concurrent start");
    assert_eq!(ledger.records().count(), 2);

    // Cancellation requests and a parked interval do not consume either
    // claim; custody outlives execution state.
    ledger
        .request_cancellation(&first)
        .expect("first claim survives a cancel request");
    ledger
        .request_cancellation(&second)
        .expect("second claim survives a cancel request");
    assert!(ledger.cancellation_requested(first.identity()));
    assert!(ledger.cancellation_requested(second.identity()));

    // A foreign ledger cannot settle the claim; the failure returns it.
    let first = foreign_ledger
        .settle(first, TaskSettlementOutcome::Completed)
        .expect_err("cross-instance settlement")
        .into_claim();

    // The second activation settles while the first remains parked; reclaim
    // of the first lease still rejects while its claim lives. The recorded
    // request went unobserved — an inline completion can only report an
    // ordinary outcome.
    let settled_second = ledger
        .settle(second, TaskSettlementOutcome::Completed)
        .expect("second settles first");
    assert!(settled_second.into_released_lease().is_none());
    assert!(
        ledger
            .validate_storage_reclaim(provenance(268, 269))
            .is_err()
    );

    let settled_first = ledger
        .settle(first, TaskSettlementOutcome::Cancelled)
        .expect("first observed the recorded request and settles cancelled");
    assert_eq!(settled_first.outcome(), TaskSettlementOutcome::Cancelled);
    let lease = settled_first
        .into_released_lease()
        .expect("first activation held a stack lease");
    assert_eq!(lease.provenance(), provenance(268, 269));
    ledger
        .validate_storage_reclaim(provenance(268, 269))
        .expect("released lease permits reclamation");
    ledger.close().expect("runtime closes with no live claims");
}
