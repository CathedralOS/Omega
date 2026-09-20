//! Journal tests: recorded facts replay to the ledger's own roster, and a
//! dropped, reordered, replayed-identity, or substituted fact rejects at its
//! position instead of reconstructing a different roster.

use super::{ComponentEraJournal, ComponentEraJournalFact};
use crate::{
    ComponentEraCandidate, ComponentEraEntryLedger, ComponentEraEntryReceipt,
    ComponentEraEntryState, ComponentEraLeaveReceipt, ComponentEraLedgerId,
    ComponentEraPublicationReceipt, ComponentEraQuiescenceReceipt, ComponentEraRetirementReceipt,
    ExecutableTcbManifest, ExecutableTcbProfile, ExecutableTcbProfileAcceptance, ExecutionScope,
    IncompleteScopePolicy, ProgramLocalRootEpochLeaseId, ScopeCompleteness,
    evaluate_executable_tcb_profile,
};
use installation_evidence::InstalledArtifactOccurrenceDigest;

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

fn ledger(maximum: usize) -> ComponentEraEntryLedger {
    ComponentEraEntryLedger::new(
        ComponentEraLedgerId::from_normalized_identity(1).expect("ledger identity"),
        "CodecBinding/v1".into(),
        "CodecEntry/v1".into(),
        maximum,
        acceptance("platform", 1),
    )
    .expect("ledger")
}

fn journal() -> ComponentEraJournal {
    ComponentEraJournal::new("CodecBinding/v1".into(), "CodecEntry/v1".into()).expect("journal")
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

fn publish_and_record(
    ledger: &mut ComponentEraEntryLedger,
    journal: &mut ComponentEraJournal,
    era: u64,
    receipt: u64,
) {
    let candidate = candidate(era);
    let publication = ComponentEraPublicationReceipt::from_runtime(
        receipt,
        ledger,
        &candidate,
        true,
        ledger.current_era().is_some(),
    );
    ledger
        .publish(candidate, publication.clone())
        .expect("publication");
    journal.record_publication(&publication);
}

fn replayed_sequence() -> (ComponentEraEntryLedger, ComponentEraJournal) {
    let mut ledger = ledger(2);
    let mut journal = journal();

    publish_and_record(&mut ledger, &mut journal, 10, 100);
    let entry =
        ComponentEraEntryReceipt::from_runtime(500, &ledger, 10, "entry-plan:10".into(), true);
    let active = ledger.enter(entry.clone()).expect("v1 entry");
    journal.record_entry(&entry);

    publish_and_record(&mut ledger, &mut journal, 20, 101);
    let lease = ledger
        .acquire_program_local_root_epoch_lease(lease_id(900), 20, "CodecEntry/v1")
        .expect("current era lease");
    journal.record_epoch_lease_acquisition(&lease);
    journal.record_epoch_lease_release(&lease);
    ledger
        .release_program_local_root_epoch_lease(lease)
        .expect("release");

    let leave = ComponentEraLeaveReceipt::from_runtime(&active, true);
    ledger.leave(active, leave.clone()).expect("leave");
    journal.record_leave(&leave);

    let quiescence = ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 0, true);
    ledger
        .establish_quiescence(quiescence.clone())
        .expect("quiescence");
    journal.record_quiescence(&quiescence);

    let retirement = ComponentEraRetirementReceipt::from_runtime(700, &ledger, 10, true);
    ledger.retire(retirement.clone()).expect("retirement");
    journal.record_retirement(&retirement);

    (ledger, journal)
}

#[test]
fn replay_reconstructs_the_full_transition_roster() {
    let (ledger, journal) = replayed_sequence();
    assert_eq!(journal.len(), 8);

    let roster = journal.replay().expect("a complete journal replays");
    assert_eq!(
        roster.live_eras().collect::<Vec<_>>(),
        ledger.live_eras().collect::<Vec<_>>()
    );
    assert_eq!(roster.current_era(), ledger.current_era());
    for (era, _, _) in ledger.live_eras() {
        assert_eq!(
            roster.epoch_lease_holds(era),
            ledger.program_local_root_authority_holds(era)
        );
    }
    assert_eq!(
        roster
            .era_candidate(20)
            .expect("current era candidate")
            .entry_plan_identity,
        "entry-plan:20"
    );
}

#[test]
fn replay_rejects_a_fact_the_ledger_never_accepted() {
    let (_, journal) = replayed_sequence();
    // Drop the Entered fact: the journaled Left then names an invocation that
    // never entered.
    let mut facts = journal.facts().to_vec();
    let entered = facts
        .iter()
        .position(|fact| matches!(fact, ComponentEraJournalFact::Entered { .. }))
        .expect("entered fact");
    facts.remove(entered);
    let left = facts
        .iter()
        .position(|fact| matches!(fact, ComponentEraJournalFact::Left { .. }))
        .expect("left fact");
    let dropped = ComponentEraJournal::from_facts(
        journal.binding_contract_identity().to_owned(),
        journal.entry_contract_identity().to_owned(),
        facts,
    )
    .expect("journal");
    let error = dropped.replay().expect_err("dropped entry");
    assert_eq!(error.position(), left);
    assert!(error.diagnostic().contains("never entered"));

    // A leave-only journal fails at its first fact for the same reason.
    let leave_only = ComponentEraJournal::from_facts(
        "CodecBinding/v1".into(),
        "CodecEntry/v1".into(),
        vec![ComponentEraJournalFact::Left {
            invocation_identity: 500,
            binding_contract_identity: "CodecBinding/v1".into(),
            era_identity: 10,
            entry_plan_identity: "entry-plan:10".into(),
            leave_completed: true,
        }],
    )
    .expect("journal");
    assert!(leave_only.replay().is_err());
}

#[test]
fn replay_rejects_replayed_identities_and_reordered_facts() {
    let (_, journal) = replayed_sequence();
    let facts = journal.facts().to_vec();

    // A publication fact appended a second time carries a previous-era
    // identity that is no longer current.
    let duplicated = vec![facts[0].clone(), facts[0].clone()];
    let replayed = ComponentEraJournal::from_facts(
        "CodecBinding/v1".into(),
        "CodecEntry/v1".into(),
        duplicated,
    )
    .expect("journal");
    let error = replayed.replay().expect_err("replayed publication");
    assert_eq!(error.position(), 1);
    assert!(error.diagnostic().contains("not current"));

    // A fabricated entry replaying a consumed invocation identity rejects at
    // the fresh-invocation rule even though its era and plan are exact.
    let mut reentered = facts.clone();
    reentered.push(ComponentEraJournalFact::Entered {
        invocation_identity: 500,
        binding_contract_identity: "CodecBinding/v1".into(),
        entry_contract_identity: "CodecEntry/v1".into(),
        resolved_era_identity: 20,
        entry_plan_identity: "entry-plan:20".into(),
        entry_linearized: true,
    });
    let reentered = ComponentEraJournal::from_facts(
        "CodecBinding/v1".into(),
        "CodecEntry/v1".into(),
        reentered,
    )
    .expect("journal");
    let error = reentered.replay().expect_err("replayed invocation");
    assert_eq!(error.position(), facts.len());
    assert!(error.diagnostic().contains("fresh invocation"));

    // An entry journaled before its era publishes has no live era to enter.
    let entered = facts
        .iter()
        .position(|fact| matches!(fact, ComponentEraJournalFact::Entered { .. }))
        .expect("entered fact");
    let reordered = ComponentEraJournal::from_facts(
        "CodecBinding/v1".into(),
        "CodecEntry/v1".into(),
        vec![facts[entered].clone(), facts[0].clone()],
    )
    .expect("journal");
    let error = reordered.replay().expect_err("reordered entry");
    assert_eq!(error.position(), 0);
    assert!(error.diagnostic().contains("not live"));
}

#[test]
fn replay_rejects_substituted_and_foreign_contract_facts() {
    let (_, journal) = replayed_sequence();
    let mut facts = journal.facts().to_vec();

    // Substitute the entered plan for one the era never published.
    let entered = facts
        .iter_mut()
        .find(|fact| matches!(fact, ComponentEraJournalFact::Entered { .. }))
        .expect("entered fact");
    if let ComponentEraJournalFact::Entered {
        entry_plan_identity,
        ..
    } = entered
    {
        *entry_plan_identity = "entry-plan:substituted".into();
    }
    let substituted = ComponentEraJournal::from_facts(
        "CodecBinding/v1".into(),
        "CodecEntry/v1".into(),
        facts.clone(),
    )
    .expect("journal");
    let error = substituted.replay().expect_err("substituted plan");
    assert!(error.diagnostic().contains("did not publish"));

    // A fact recorded under a foreign binding contract rejects on replay.
    if let ComponentEraJournalFact::Published {
        binding_contract_identity,
        ..
    } = &mut facts[0]
    {
        *binding_contract_identity = "Foreign/v1".into();
    }
    let foreign =
        ComponentEraJournal::from_facts("CodecBinding/v1".into(), "CodecEntry/v1".into(), facts)
            .expect("journal");
    let error = foreign.replay().expect_err("foreign contract");
    assert_eq!(error.position(), 0);
    assert!(error.diagnostic().contains("foreign contract"));
}

#[test]
fn replay_rejects_retirement_of_a_current_or_non_quiescent_era() {
    let (ledger, _) = replayed_sequence();
    assert_eq!(ledger.current_era(), Some(20));
    assert_eq!(
        ledger.live_eras().collect::<Vec<_>>(),
        vec![(20, ComponentEraEntryState::Open, 0)]
    );

    // Retiring the open current era must reject: neither quiescent nor
    // noncurrent.
    let retiring_current = ComponentEraJournal::from_facts(
        "CodecBinding/v1".into(),
        "CodecEntry/v1".into(),
        vec![
            journal_publish_fact(&ledger),
            ComponentEraJournalFact::Retired {
                retirement_identity: 700,
                era_identity: 10,
                binding_contract_identity: "CodecBinding/v1".into(),
                lifetime_cohort_released: true,
            },
        ],
    )
    .expect("journal");
    let error = retiring_current.replay().expect_err("open current era");
    assert_eq!(error.position(), 1);
    assert!(error.diagnostic().contains("noncurrent quiescent"));
}

#[test]
fn journal_binds_the_contract_identities_it_was_constructed_with() {
    assert!(ComponentEraJournal::new("".into(), "CodecEntry/v1".into()).is_err());
    assert!(ComponentEraJournal::new("CodecBinding/v1".into(), "  ".into()).is_err());
    let journal = journal();
    assert_eq!(journal.binding_contract_identity(), "CodecBinding/v1");
    assert_eq!(journal.entry_contract_identity(), "CodecEntry/v1");
    assert!(journal.is_empty());
    let roster = journal.replay().expect("an empty journal replays");
    assert_eq!(roster.current_era(), None);
    assert_eq!(roster.live_eras().count(), 0);
}

fn journal_publish_fact(ledger: &ComponentEraEntryLedger) -> ComponentEraJournalFact {
    ComponentEraJournalFact::Published {
        publication_identity: 100,
        binding_contract_identity: ledger.binding_contract_identity().to_owned(),
        entry_contract_identity: ledger.entry_contract_identity().to_owned(),
        previous_era_identity: None,
        candidate: candidate(10),
        new_era_visible: true,
        previous_era_closed: false,
    }
}
