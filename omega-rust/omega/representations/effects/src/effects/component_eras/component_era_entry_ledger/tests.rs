//! Component era entry ledger tests.

use super::{
    ComponentEraCandidate, ComponentEraEntryLedger, ComponentEraEntryReceipt,
    ComponentEraEntryState, ComponentEraLeaveReceipt, ComponentEraLedgerId,
    ComponentEraPublicationReceipt, ComponentEraQuiescenceReceipt, ComponentEraRetirementReceipt,
    ExecutableTcbProfileAcceptance, InstalledArtifactOccurrenceDigest,
    ProgramLocalRootEpochLeaseId,
};
use crate::{
    ExecutableTcbManifest, ExecutableTcbProfile, ExecutionScope, IncompleteScopePolicy,
    ScopeCompleteness, evaluate_executable_tcb_profile,
};

fn acceptance(name: &str, closure: u64) -> ExecutableTcbProfileAcceptance {
    evaluate_executable_tcb_profile(
        &ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::CallerAddressSpace,
                selected_provider_closure_report_identity: closure,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        },
        &ExecutableTcbProfile {
            name: name.into(),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        },
    )
    .expect("profile acceptance")
}

fn ledger_with_identity(identity: u64, maximum: usize) -> ComponentEraEntryLedger {
    ComponentEraEntryLedger::new(
        ComponentEraLedgerId::from_normalized_identity(identity).expect("ledger identity"),
        "CodecBinding/v1".into(),
        "CodecEntry/v1".into(),
        maximum,
        acceptance("platform", 1),
    )
    .expect("ledger")
}

fn ledger(maximum: usize) -> ComponentEraEntryLedger {
    ledger_with_identity(1, maximum)
}

fn lease_id(identity: u64) -> ProgramLocalRootEpochLeaseId {
    ProgramLocalRootEpochLeaseId::from_normalized_identity(identity).expect("epoch lease identity")
}

fn candidate(era: u64) -> ComponentEraCandidate {
    ComponentEraCandidate {
        era_identity: era,
        artifact_occurrence_digest: InstalledArtifactOccurrenceDigest::from_sha256([era as u8; 32]),
        artifact_instance_compatibility_report_identity: era + 1_000,
        binding_contract_identity: "CodecBinding/v1".into(),
        entry_contract_identity: "CodecEntry/v1".into(),
        entry_plan_identity: format!("entry-plan:{era}"),
        entry_plan_admission_receipt_identity: format!("receipt:entry-plan:{era}"),
        executable_tcb_acceptance: acceptance(format!("era-{era}").as_str(), era),
    }
}

fn publish(ledger: &mut ComponentEraEntryLedger, era: u64, receipt: u64) {
    let candidate = candidate(era);
    let publication = ComponentEraPublicationReceipt::from_runtime(
        receipt,
        ledger,
        &candidate,
        true,
        ledger.current_era().is_some(),
    );
    ledger.publish(candidate, publication).expect("publication");
}

#[test]
fn routing_switch_closes_old_entry_but_retains_its_active_invocation() {
    let mut ledger = ledger(2);
    publish(&mut ledger, 10, 100);
    let entry_receipt =
        ComponentEraEntryReceipt::from_runtime(500, &ledger, 10, "entry-plan:10".into(), true);
    let old_entry = ledger.enter(entry_receipt).expect("v1 entry");
    publish(&mut ledger, 20, 101);
    assert_eq!(ledger.current_era(), Some(20));
    assert_eq!(
        ledger.live_eras().collect::<Vec<_>>(),
        vec![
            (10, ComponentEraEntryState::Closing, 1),
            (20, ComponentEraEntryState::Open, 0),
        ]
    );
    let stale =
        ComponentEraEntryReceipt::from_runtime(501, &ledger, 10, "entry-plan:10".into(), true);
    assert!(ledger.enter(stale).is_err());

    let leave = ComponentEraLeaveReceipt::from_runtime(&old_entry, true);
    ledger
        .leave(old_entry, leave)
        .expect("old entry leaves its own era");
    let replayed =
        ComponentEraEntryReceipt::from_runtime(500, &ledger, 20, "entry-plan:20".into(), true);
    assert!(ledger.enter(replayed).is_err());
    let blocked = ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 1, true);
    assert!(ledger.establish_quiescence(blocked).is_err());
    let quiescent = ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 0, true);
    ledger
        .establish_quiescence(quiescent)
        .expect("all holds disposed");
    let retirement = ComponentEraRetirementReceipt::from_runtime(700, &ledger, 10, true);
    ledger
        .retire(retirement)
        .expect("quiescent old era retires");
    assert_eq!(
        ledger.live_eras().collect::<Vec<_>>(),
        vec![(20, ComponentEraEntryState::Open, 0)]
    );
    assert_eq!(
        ledger
            .live_executable_tcb_report()
            .completeness()
            .sources()
            .len(),
        2
    );
}

#[test]
fn publication_enforces_retention_limit_and_receipt_identity() {
    let mut ledger = ledger(1);
    publish(&mut ledger, 10, 100);
    let next = candidate(20);
    let receipt = ComponentEraPublicationReceipt::from_runtime(101, &ledger, &next, true, true);
    let error = ledger.publish(next, receipt).expect_err("live-era limit");
    assert!(error.diagnostic().contains("retention limit"));
    let (next, _) = (*error).into_parts();
    assert_eq!(next.era_identity, 20);
}

#[test]
fn publication_receipt_retains_the_complete_candidate_not_only_compact_ids() {
    let mut ledger = ledger(2);
    let mut substituted = candidate(10);
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(100, &ledger, &substituted, true, false);
    substituted.executable_tcb_acceptance = acceptance("substituted-era", 999);

    let error = ledger
        .publish(substituted, receipt)
        .expect_err("candidate evidence substituted after receipt construction");
    assert!(error.diagnostic().contains("exact candidate"));
    assert_eq!(ledger.current_era(), None);
}

#[test]
fn leave_and_retirement_receipts_cannot_drift_or_replay() {
    let mut ledger = ledger(2);
    publish(&mut ledger, 10, 100);
    let entry = ledger
        .enter(ComponentEraEntryReceipt::from_runtime(
            500,
            &ledger,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("entry");
    let mut drifted = ComponentEraLeaveReceipt::from_runtime(&entry, true);
    drifted.era_identity = 11;
    let error = ledger.leave(entry, drifted).expect_err("leave drift");
    let (entry, _) = (*error).into_parts();
    let exact_leave = ComponentEraLeaveReceipt::from_runtime(&entry, true);
    ledger.leave(entry, exact_leave).expect("exact leave");
    publish(&mut ledger, 20, 101);
    ledger
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            &ledger, 10, 0, true,
        ))
        .expect("quiescence");
    ledger
        .retire(ComponentEraRetirementReceipt::from_runtime(
            700, &ledger, 10, true,
        ))
        .expect("first retirement");
    let replay = ComponentEraRetirementReceipt::from_runtime(700, &ledger, 20, true);
    assert!(ledger.retire(replay).is_err());
}

#[test]
fn program_local_root_epoch_lease_requires_current_open_exact_contract_and_artifact() {
    let mut ledger = ledger(2);
    let mut incomplete = candidate(10);
    incomplete.artifact_instance_compatibility_report_identity = 0;
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(99, &ledger, &incomplete, true, false);
    assert!(ledger.publish(incomplete, receipt).is_err());

    let original_candidate = candidate(10);
    let mut drifted = ComponentEraPublicationReceipt::from_runtime(
        100,
        &ledger,
        &original_candidate,
        true,
        false,
    );
    drifted.candidate_artifact_instance_compatibility_report_identity += 1;
    let error = ledger
        .publish(original_candidate, drifted)
        .expect_err("artifact-instance drift");
    let (published_candidate, _) = (*error).into_parts();
    let exact = ComponentEraPublicationReceipt::from_runtime(
        100,
        &ledger,
        &published_candidate,
        true,
        false,
    );
    ledger
        .publish(published_candidate, exact)
        .expect("exact publication");

    let wrong_contract = ledger
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "OtherEntry/v1")
        .expect_err("entry contract mismatch");
    assert!(wrong_contract.diagnostic().contains("exact open"));
    assert_eq!(ledger.program_local_root_authority_holds(10), Some(0));
    let stale = ledger
        .acquire_program_local_root_epoch_lease(lease_id(901), 9, "CodecEntry/v1")
        .expect_err("stale era");
    assert!(stale.diagnostic().contains("exact current"));

    let mut lease = ledger
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("exact current era lease");
    assert_eq!(lease.era_identity(), 10);
    assert_eq!(
        lease.artifact_instance_compatibility_report_identity(),
        1_010
    );
    assert_eq!(ledger.program_local_root_authority_holds(10), Some(1));
    ledger
        .validate_program_local_root_epoch_lease(&lease)
        .expect("issued lease is live in the current open era");

    lease.candidate.executable_tcb_acceptance = acceptance("substituted-era", 999);
    assert!(
        ledger
            .validate_program_local_root_epoch_lease(&lease)
            .is_err(),
        "a lease cannot substitute candidate evidence omitted from its compact projections"
    );
    let error = ledger
        .release_program_local_root_epoch_lease(lease)
        .expect_err("substituted candidate evidence");
    assert_eq!(ledger.program_local_root_authority_holds(10), Some(1));
    let mut lease = error.into_lease();
    lease.candidate = candidate(10);
    ledger
        .validate_program_local_root_epoch_lease(&lease)
        .expect("restored exact lease remains live");

    publish(&mut ledger, 20, 101);
    assert!(
        ledger
            .validate_program_local_root_epoch_lease(&lease)
            .is_err(),
        "a closing-era lease still pins retirement but cannot establish new authority"
    );
    let closing = ledger
        .acquire_program_local_root_epoch_lease(lease_id(902), 10, "CodecEntry/v1")
        .expect_err("closing era");
    assert!(closing.diagnostic().contains("exact current"));
    ledger
        .release_program_local_root_epoch_lease(lease)
        .expect("closing era retains the exact releasable hold");
    let current = ledger
        .acquire_program_local_root_epoch_lease(lease_id(902), 20, "CodecEntry/v1")
        .expect("new current era");
    ledger
        .release_program_local_root_epoch_lease(current)
        .expect("release current era hold");
}

#[test]
fn live_program_local_root_epoch_lease_blocks_quiescence_and_retirement() {
    let mut ledger = ledger(2);
    publish(&mut ledger, 10, 100);
    let lease = ledger
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("lease");
    publish(&mut ledger, 20, 101);

    let retirement = ComponentEraRetirementReceipt::from_runtime(700, &ledger, 10, true);
    let blocked = ledger.retire(retirement).expect_err("live epoch lease");
    assert!(blocked.diagnostic().contains("epoch leases remain live"));
    let quiescence = ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 0, true);
    let blocked = ledger
        .establish_quiescence(quiescence)
        .expect_err("authority hold");
    assert!(blocked.diagnostic().contains("authority holds"));

    ledger
        .release_program_local_root_epoch_lease(lease)
        .expect("exact release");
    assert_eq!(ledger.program_local_root_authority_holds(10), Some(0));
    ledger
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            &ledger, 10, 0, true,
        ))
        .expect("quiescence after release");
    ledger
        .retire(ComponentEraRetirementReceipt::from_runtime(
            700, &ledger, 10, true,
        ))
        .expect("retirement after release");
}

#[test]
fn epoch_lease_release_rejects_ledger_substitution_and_identity_replay() {
    let mut first = ledger_with_identity(1, 1);
    let mut second = ledger_with_identity(2, 1);
    publish(&mut first, 10, 100);
    publish(&mut second, 10, 200);

    let lease = first
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("first ledger lease");
    let error = second
        .release_program_local_root_epoch_lease(lease)
        .expect_err("cross-ledger substitution");
    assert!(error.diagnostic().contains("exact published era"));
    assert_eq!(first.program_local_root_authority_holds(10), Some(1));
    assert_eq!(second.program_local_root_authority_holds(10), Some(0));
    let lease = error.into_lease();
    first
        .release_program_local_root_epoch_lease(lease)
        .expect("rightful ledger release");

    let replay = first
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect_err("lease identity replay");
    assert!(replay.diagnostic().contains("already issued or consumed"));
}
