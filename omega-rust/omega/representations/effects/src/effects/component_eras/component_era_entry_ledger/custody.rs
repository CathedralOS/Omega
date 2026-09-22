//! One-field substitution custody over the component-era entry ledger's
//! runtime receipt family.
//!
//! The ledger is the containing authority for every receipt this family
//! mints: `binding_contract_identity` and `entry_contract_identity` are its
//! published identities, and the current era, per-era records, and consumed
//! publication/invocation/retirement/lease identity sets are its retained
//! state. `from_runtime` is the honest construction leg — it copies the
//! ledger and candidate coordinates into the receipt — and
//! `publish`/`enter`/`leave`/`establish_quiescence`/`retire` plus the
//! epoch-lease acquire/validate/release operations are the independent
//! replays: each compares every copied field against the retained argument
//! or ledger record, so a field substituted after minting rejects and the
//! ledger state stays untouched.
//!
//! The minted identities (`publication_identity`, `invocation_identity`,
//! `retirement_identity`, the lease's `identity`) are adopted verbatim —
//! nothing outside the runtime could contradict a fresh nonce — but the
//! consumed sets move honestly: zero or replayed identities reject, and a
//! lease rebound to another live hold's identity performs the
//! indistinguishable release of that hold. A receipt minted under a foreign
//! ledger's contract identities, or carrying a stale era or invocation
//! coordinate, rejects against this ledger's records.

use super::{
    ActiveComponentEraEntry, ComponentEraCandidate, ComponentEraEntryLedger,
    ComponentEraEntryReceipt, ComponentEraEntryState, ComponentEraLeaveReceipt,
    ComponentEraLedgerId, ComponentEraPublicationReceipt, ComponentEraQuiescenceReceipt,
    ComponentEraRetirementReceipt, ExecutableTcbProfileAcceptance,
    InstalledArtifactOccurrenceDigest, ProgramLocalRootEpochLease, ProgramLocalRootEpochLeaseId,
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

/// An acceptance whose manifest answers a different coexistence scope: the
/// embedded `CoexistingExecutableTcbSet` admission replay rejects it.
fn isolated_acceptance() -> ExecutableTcbProfileAcceptance {
    evaluate_executable_tcb_profile(
        &ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::IsolatedProvider(77),
                selected_provider_closure_report_identity: 77,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        },
        &ExecutableTcbProfile {
            name: "isolated".into(),
            scope: ExecutionScope::IsolatedProvider(77),
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        },
    )
    .expect("isolated profile acceptance")
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

/// A second ledger under different published contract identities: receipts
/// minted against it are foreign evidence to the primary ledger.
fn foreign_ledger(maximum: usize) -> ComponentEraEntryLedger {
    ComponentEraEntryLedger::new(
        ComponentEraLedgerId::from_normalized_identity(2).expect("ledger identity"),
        "ForeignBinding/v1".into(),
        "ForeignEntry/v1".into(),
        maximum,
        acceptance("platform", 1),
    )
    .expect("foreign ledger")
}

fn lease_id(identity: u64) -> ProgramLocalRootEpochLeaseId {
    ProgramLocalRootEpochLeaseId::from_normalized_identity(identity).expect("epoch lease identity")
}

fn candidate_with_contracts(era: u64, binding: &str, entry: &str) -> ComponentEraCandidate {
    ComponentEraCandidate {
        era_identity: era,
        artifact_occurrence_digest: InstalledArtifactOccurrenceDigest::from_sha256([era as u8; 32]),
        artifact_instance_compatibility_report_identity: era + 1_000,
        binding_contract_identity: binding.into(),
        entry_contract_identity: entry.into(),
        entry_plan_identity: format!("entry-plan:{era}"),
        entry_plan_admission_receipt_identity: format!("receipt:entry-plan:{era}"),
        executable_tcb_acceptance: acceptance(format!("era-{era}").as_str(), era),
    }
}

fn candidate(era: u64) -> ComponentEraCandidate {
    candidate_with_contracts(era, "CodecBinding/v1", "CodecEntry/v1")
}

/// A candidate answering `foreign_ledger`'s published contract identities.
fn foreign_candidate(era: u64) -> ComponentEraCandidate {
    candidate_with_contracts(era, "ForeignBinding/v1", "ForeignEntry/v1")
}

fn publish_candidate(
    ledger: &mut ComponentEraEntryLedger,
    candidate: ComponentEraCandidate,
    receipt: u64,
) {
    let publication = ComponentEraPublicationReceipt::from_runtime(
        receipt,
        ledger,
        &candidate,
        true,
        ledger.current_era().is_some(),
    );
    ledger.publish(candidate, publication).expect("publication");
}

fn publish(ledger: &mut ComponentEraEntryLedger, era: u64, receipt: u64) {
    publish_candidate(ledger, candidate(era), receipt);
}

fn foreign_publish(ledger: &mut ComponentEraEntryLedger, era: u64, receipt: u64) {
    publish_candidate(ledger, foreign_candidate(era), receipt);
}

/// Publish `era`, then publish `successor` so `era` is Closing — the state a
/// quiescence receipt must name.
fn publish_then_supersede(ledger: &mut ComponentEraEntryLedger, era: u64, successor: u64) {
    publish(ledger, era, era + 90);
    publish(ledger, successor, successor + 90);
}

/// Publish `era`, supersede it, and quiesce it — the state a retirement
/// receipt must name.
fn publish_supersede_quiesce(ledger: &mut ComponentEraEntryLedger, era: u64, successor: u64) {
    publish_then_supersede(ledger, era, successor);
    ledger
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            ledger, era, 0, true,
        ))
        .expect("quiescence");
}

/// The same preconditions minted under `foreign_ledger`'s contracts.
fn foreign_publish_then_supersede(ledger: &mut ComponentEraEntryLedger, era: u64, successor: u64) {
    foreign_publish(ledger, era, era + 90);
    foreign_publish(ledger, successor, successor + 90);
}

fn foreign_publish_supersede_quiesce(
    ledger: &mut ComponentEraEntryLedger,
    era: u64,
    successor: u64,
) {
    foreign_publish_then_supersede(ledger, era, successor);
    ledger
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            ledger, era, 0, true,
        ))
        .expect("quiescence");
}

/// The receipt-binding join at publication: every copied coordinate must
/// equal the retained candidate and the ledger's published identities.
const PUBLICATION_BINDING: &str = "does not bind and expose the exact candidate";

/// Apply one substitution to an honest first-era publication and replay it
/// against a fresh ledger. The returned ledger proves rejection left the
/// containing state untouched.
fn rejected_publication(
    mutate: impl FnOnce(&mut ComponentEraCandidate, &mut ComponentEraPublicationReceipt),
) -> (ComponentEraEntryLedger, String) {
    let mut ledger = ledger(4);
    let mut candidate = candidate(10);
    let mut receipt =
        ComponentEraPublicationReceipt::from_runtime(100, &ledger, &candidate, true, false);
    mutate(&mut candidate, &mut receipt);
    let error = ledger
        .publish(candidate, receipt)
        .expect_err("a substituted publication must not commit");
    assert_eq!(ledger.current_era(), None, "rejection retains no era");
    assert!(
        ledger.live_eras().next().is_none(),
        "rejection retains no live era record"
    );
    (ledger, error.diagnostic().to_owned())
}

fn publication_reject_with(
    fragment: &str,
    mutate: impl FnOnce(&mut ComponentEraCandidate, &mut ComponentEraPublicationReceipt),
) {
    let (_, diagnostic) = rejected_publication(mutate);
    assert!(
        diagnostic.contains(fragment),
        "expected `{fragment}` among `{diagnostic}`"
    );
}

#[test]
fn component_era_publication_receipt_rejects_every_one_field_substitution() {
    // Every receipt field copied at minting replays against the retained
    // candidate argument and the ledger's own identities.
    publication_reject_with("receipt is zero or replayed", |_, receipt| {
        receipt.publication_identity = 0;
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.binding_contract_identity = "ForeignBinding/v1".into();
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.entry_contract_identity = "ForeignEntry/v1".into();
    });
    // `previous_era_identity` must equal the ledger's current era — `None`
    // before the first publication; a fabricated predecessor rejects, and on
    // a routing switch any drift from the exact superseded era rejects.
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.previous_era_identity = Some(10);
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.candidate_era_identity = 99;
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.candidate_artifact_occurrence_digest =
            InstalledArtifactOccurrenceDigest::from_sha256([0xEE; 32]);
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.candidate_artifact_instance_compatibility_report_identity += 1;
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.candidate_entry_plan_identity = "entry-plan:foreign".into();
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.candidate_entry_plan_admission_receipt_identity =
            "receipt:entry-plan:foreign".into();
    });
    // The retained whole-candidate copy: a wholesale or an inner-field
    // substitution both diverge from the published argument.
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.candidate = candidate(99);
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.candidate.entry_contract_identity = "OtherEntry/v1".into();
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt
            .candidate
            .artifact_instance_compatibility_report_identity = 4_444;
    });
    publication_reject_with(PUBLICATION_BINDING, |_, receipt| {
        receipt.new_era_visible = false;
    });

    // `previous_era_closed` is replay-bound once a predecessor exists: the
    // second publication must report the old era closed to future entry.
    let mut switched = ledger(4);
    publish(&mut switched, 10, 100);
    let next = candidate(20);
    let mut receipt =
        ComponentEraPublicationReceipt::from_runtime(101, &switched, &next, true, true);
    receipt.previous_era_closed = false;
    let error = switched
        .publish(next, receipt)
        .expect_err("an unclosed previous era must reject");
    assert!(
        error
            .diagnostic()
            .contains("does not close the previous era"),
        "{}",
        error.diagnostic()
    );
    assert_eq!(
        switched.live_eras().collect::<Vec<_>>(),
        vec![(10, ComponentEraEntryState::Open, 0)],
        "rejection keeps the previous era open and current"
    );
    // A fabricated predecessor diverges the same way on the switch.
    let next = candidate(30);
    let mut receipt =
        ComponentEraPublicationReceipt::from_runtime(102, &switched, &next, true, true);
    receipt.previous_era_identity = Some(999);
    let error = switched
        .publish(next, receipt)
        .expect_err("a fabricated predecessor must reject");
    assert!(
        error.diagnostic().contains(PUBLICATION_BINDING),
        "{}",
        error.diagnostic()
    );

    // The minted `publication_identity` is adopted verbatim — a fresh nonce
    // publishes — but the consumed set moves honestly: replaying the identity
    // a second time rejects.
    let mut adopted = ledger(4);
    publish(&mut adopted, 10, 555);
    let replayed = candidate(20);
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(555, &adopted, &replayed, true, true);
    let error = adopted
        .publish(replayed, receipt)
        .expect_err("a replayed publication identity must reject");
    assert!(
        error.diagnostic().contains("zero or replayed"),
        "{}",
        error.diagnostic()
    );

    // A receipt minted under a foreign ledger's contract identities cannot
    // move this ledger's routing.
    let mut foreign = foreign_ledger(4);
    foreign_publish(&mut foreign, 10, 700);
    let ours = candidate(10);
    let foreign_receipt =
        ComponentEraPublicationReceipt::from_runtime(701, &foreign, &ours, true, false);
    let mut ledger = ledger(4);
    let error = ledger
        .publish(ours, foreign_receipt)
        .expect_err("a foreign-ledger receipt must reject");
    assert!(
        error.diagnostic().contains(PUBLICATION_BINDING),
        "{}",
        error.diagnostic()
    );
}

#[test]
fn component_era_candidate_rejects_every_one_field_substitution() {
    // The publish argument is the family's other half: every candidate field
    // is bound either by the completeness precheck, by the ledger's contract
    // identities, or by the receipt's copied coordinates.
    publication_reject_with("candidate is incomplete", |candidate, _| {
        candidate.era_identity = 0;
    });
    publication_reject_with(PUBLICATION_BINDING, |candidate, _| {
        candidate.era_identity = 99;
    });
    publication_reject_with(PUBLICATION_BINDING, |candidate, _| {
        candidate.artifact_occurrence_digest =
            InstalledArtifactOccurrenceDigest::from_sha256([0x77; 32]);
    });
    publication_reject_with("candidate is incomplete", |candidate, _| {
        candidate.artifact_instance_compatibility_report_identity = 0;
    });
    publication_reject_with(PUBLICATION_BINDING, |candidate, _| {
        candidate.artifact_instance_compatibility_report_identity += 1;
    });
    publication_reject_with(PUBLICATION_BINDING, |candidate, _| {
        candidate.binding_contract_identity = "ForeignBinding/v1".into();
    });
    publication_reject_with("candidate is incomplete", |candidate, _| {
        candidate.binding_contract_identity.clear();
    });
    publication_reject_with(PUBLICATION_BINDING, |candidate, _| {
        candidate.entry_contract_identity = "ForeignEntry/v1".into();
    });
    publication_reject_with("candidate is incomplete", |candidate, _| {
        candidate.entry_contract_identity.clear();
    });
    publication_reject_with(PUBLICATION_BINDING, |candidate, _| {
        candidate.entry_plan_identity = "entry-plan:foreign".into();
    });
    publication_reject_with("candidate is incomplete", |candidate, _| {
        candidate.entry_plan_identity.clear();
    });
    publication_reject_with(PUBLICATION_BINDING, |candidate, _| {
        candidate.entry_plan_admission_receipt_identity = "receipt:foreign".into();
    });
    publication_reject_with("candidate is incomplete", |candidate, _| {
        candidate.entry_plan_admission_receipt_identity.clear();
    });
    // A same-scope evidence substitution diverges from the receipt's copy.
    publication_reject_with(PUBLICATION_BINDING, |candidate, _| {
        candidate.executable_tcb_acceptance = acceptance("substituted-era", 999);
    });

    // A scope-drifting acceptance honestly reminted into the receipt passes
    // the binding join and is refused by the embedded coexisting-TCB
    // admission replay itself.
    let mut scoped = ledger(4);
    let mut drifted = candidate(10);
    drifted.executable_tcb_acceptance = isolated_acceptance();
    let receipt = ComponentEraPublicationReceipt::from_runtime(100, &scoped, &drifted, true, false);
    let error = scoped
        .publish(drifted, receipt)
        .expect_err("a scope-drifting acceptance must not be admitted");
    assert!(
        error
            .diagnostic()
            .contains("does not match coexistence scope"),
        "{}",
        error.diagnostic()
    );

    // A second publication of a live era identity rejects even under a fresh
    // receipt; exceeding the live-era retention limit rejects before binding.
    let mut existing = ledger(4);
    publish(&mut existing, 10, 100);
    let duplicate = candidate(10);
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(101, &existing, &duplicate, true, true);
    let error = existing
        .publish(duplicate, receipt)
        .expect_err("a duplicated era identity must reject");
    assert!(
        error.diagnostic().contains("already live"),
        "{}",
        error.diagnostic()
    );

    let mut capped = ledger(1);
    publish(&mut capped, 10, 100);
    let overflow = candidate(20);
    let receipt = ComponentEraPublicationReceipt::from_runtime(101, &capped, &overflow, true, true);
    let error = capped
        .publish(overflow, receipt)
        .expect_err("a publication beyond the retention limit must reject");
    assert!(
        error.diagnostic().contains("retention limit"),
        "{}",
        error.diagnostic()
    );
}

/// Apply one substitution to an entry receipt minted for the current open
/// era and replay it; the ledger must keep the era and its entry count.
fn rejected_entry(
    mutate: impl FnOnce(&mut ComponentEraEntryReceipt),
) -> (ComponentEraEntryLedger, String) {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    let mut receipt =
        ComponentEraEntryReceipt::from_runtime(500, &ledger, 10, "entry-plan:10".into(), true);
    mutate(&mut receipt);
    let error = ledger
        .enter(receipt)
        .expect_err("a substituted entry receipt must not enter");
    assert_eq!(
        ledger.live_eras().collect::<Vec<_>>(),
        vec![(10, ComponentEraEntryState::Open, 0)],
        "rejection leaves the era open with no entry recorded"
    );
    (ledger, error.diagnostic().to_owned())
}

#[test]
fn component_era_entry_receipt_rejects_every_one_field_substitution() {
    let cases: Vec<(&'static str, Box<dyn Fn(&mut ComponentEraEntryReceipt)>)> = vec![
        (
            "invocation_identity::zero",
            Box::new(|receipt| {
                receipt.invocation_identity = 0;
            }),
        ),
        (
            "binding_contract_identity",
            Box::new(|receipt| {
                receipt.binding_contract_identity = "ForeignBinding/v1".into();
            }),
        ),
        (
            "entry_contract_identity",
            Box::new(|receipt| {
                receipt.entry_contract_identity = "ForeignEntry/v1".into();
            }),
        ),
        (
            "resolved_era_identity::stale",
            Box::new(|receipt| {
                receipt.resolved_era_identity = 20;
            }),
        ),
        (
            "resolved_era_identity::foreign",
            Box::new(|receipt| {
                receipt.resolved_era_identity = 99;
            }),
        ),
        (
            "entry_plan_identity",
            Box::new(|receipt| {
                receipt.entry_plan_identity = "entry-plan:foreign".into();
            }),
        ),
        (
            "entry_linearized::false",
            Box::new(|receipt| {
                receipt.entry_linearized = false;
            }),
        ),
    ];
    for (name, mutate) in cases {
        let (_, diagnostic) = rejected_entry(mutate);
        assert!(
            diagnostic.contains("does not linearize exactly once"),
            "{name}: {diagnostic}"
        );
    }

    // With no published era there is no current record to bind at all.
    let mut empty = ledger(4);
    let unpublished =
        ComponentEraEntryReceipt::from_runtime(500, &empty, 10, "entry-plan:10".into(), true);
    let error = empty
        .enter(unpublished)
        .expect_err("an entry without a published era must reject");
    assert!(
        error.diagnostic().contains("no published era"),
        "{}",
        error.diagnostic()
    );

    // The minted `invocation_identity` is adopted verbatim — a fresh nonce
    // enters — but once consumed it cannot linearize a second entry.
    let mut entered = ledger(4);
    publish(&mut entered, 10, 100);
    let first =
        ComponentEraEntryReceipt::from_runtime(500, &entered, 10, "entry-plan:10".into(), true);
    let _entry = entered.enter(first).expect("fresh invocation enters");
    let replay =
        ComponentEraEntryReceipt::from_runtime(500, &entered, 10, "entry-plan:10".into(), true);
    let error = entered
        .enter(replay)
        .expect_err("a replayed invocation identity must reject");
    assert!(
        error
            .diagnostic()
            .contains("does not linearize exactly once"),
        "{}",
        error.diagnostic()
    );
    assert_eq!(
        entered.live_eras().collect::<Vec<_>>(),
        vec![(10, ComponentEraEntryState::Open, 1)]
    );

    // A routing switch keeps the old era live for its retained entries but
    // closes it to new ones: a receipt resolving the superseded era rejects.
    publish(&mut entered, 20, 101);
    let stale =
        ComponentEraEntryReceipt::from_runtime(501, &entered, 10, "entry-plan:10".into(), true);
    let error = entered
        .enter(stale)
        .expect_err("a receipt resolving a closing era must reject");
    assert!(
        error
            .diagnostic()
            .contains("does not linearize exactly once"),
        "{}",
        error.diagnostic()
    );

    // A receipt minted under a foreign ledger's contract identities cannot
    // enter this ledger's era.
    let mut foreign = foreign_ledger(4);
    foreign_publish(&mut foreign, 10, 700);
    let foreign_receipt =
        ComponentEraEntryReceipt::from_runtime(500, &foreign, 10, "entry-plan:10".into(), true);
    let mut ours = ledger(4);
    publish(&mut ours, 10, 100);
    let error = ours
        .enter(foreign_receipt)
        .expect_err("a foreign-ledger entry receipt must reject");
    assert!(
        error
            .diagnostic()
            .contains("does not linearize exactly once"),
        "{}",
        error.diagnostic()
    );
}

/// Mint one honest entry and one substituted leave receipt against it, then
/// replay. The era keeps its active entry on rejection.
fn rejected_leave(
    mutate_entry: impl FnOnce(&mut ActiveComponentEraEntry),
    mutate_receipt: impl FnOnce(&mut ComponentEraLeaveReceipt),
) -> (ComponentEraEntryLedger, String) {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    let mut entry = ledger
        .enter(ComponentEraEntryReceipt::from_runtime(
            500,
            &ledger,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("entry");
    let mut receipt = ComponentEraLeaveReceipt::from_runtime(&entry, true);
    mutate_receipt(&mut receipt);
    mutate_entry(&mut entry);
    let error = ledger
        .leave(entry, receipt)
        .expect_err("a substituted leave must not complete");
    assert_eq!(
        ledger.live_eras().collect::<Vec<_>>(),
        vec![(10, ComponentEraEntryState::Open, 1)],
        "rejection leaves the entry active"
    );
    (ledger, error.diagnostic().to_owned())
}

#[test]
fn component_era_leave_receipt_rejects_every_one_field_substitution() {
    // Every receipt field is replayed against the exact active entry handle;
    // mutating either side of the join rejects.
    let receipt_cases: Vec<(&'static str, Box<dyn Fn(&mut ComponentEraLeaveReceipt)>)> = vec![
        (
            "invocation_identity",
            Box::new(|receipt| {
                receipt.invocation_identity = 501;
            }),
        ),
        (
            "binding_contract_identity",
            Box::new(|receipt| {
                receipt.binding_contract_identity = "ForeignBinding/v1".into();
            }),
        ),
        (
            "era_identity",
            Box::new(|receipt| {
                receipt.era_identity = 20;
            }),
        ),
        (
            "entry_plan_identity",
            Box::new(|receipt| {
                receipt.entry_plan_identity = "entry-plan:foreign".into();
            }),
        ),
        (
            "leave_completed::false",
            Box::new(|receipt| {
                receipt.leave_completed = false;
            }),
        ),
    ];
    for (name, mutate) in receipt_cases {
        let (_, diagnostic) = rejected_leave(|_| {}, mutate);
        assert!(
            diagnostic.contains("does not complete the exact active entry"),
            "{name}: {diagnostic}"
        );
    }

    // The consumed entry custody is bound by the same join: a substituted
    // handle cannot pair with the honest receipt.
    let entry_cases: Vec<(&'static str, Box<dyn Fn(&mut ActiveComponentEraEntry)>)> = vec![
        (
            "entry.invocation_identity",
            Box::new(|entry| {
                entry.invocation_identity = 501;
            }),
        ),
        (
            "entry.binding_contract_identity",
            Box::new(|entry| {
                entry.binding_contract_identity = "ForeignBinding/v1".into();
            }),
        ),
        (
            "entry.entry_plan_identity",
            Box::new(|entry| {
                entry.entry_plan_identity = "entry-plan:foreign".into();
            }),
        ),
    ];
    for (name, mutate) in entry_cases {
        let (_, diagnostic) = rejected_leave(mutate, |_| {});
        assert!(
            diagnostic.contains("does not complete the exact active entry"),
            "{name}: {diagnostic}"
        );
    }
    let (_, diagnostic) = rejected_leave(|entry| entry.era_identity = 99, |_| {});
    assert!(
        diagnostic.contains("no longer live"),
        "entry.era_identity: {diagnostic}"
    );

    // A receipt answering an invocation this ledger already settled cannot
    // leave again: mutating a second entry to the consumed identity mints a
    // consistent-looking receipt whose membership check still fails.
    let mut settled = ledger(4);
    publish(&mut settled, 10, 100);
    let first = settled
        .enter(ComponentEraEntryReceipt::from_runtime(
            500,
            &settled,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("first entry");
    let first_leave = ComponentEraLeaveReceipt::from_runtime(&first, true);
    settled.leave(first, first_leave).expect("first leave");
    let mut second = settled
        .enter(ComponentEraEntryReceipt::from_runtime(
            501,
            &settled,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("second entry");
    second.invocation_identity = 500;
    let replayed = ComponentEraLeaveReceipt::from_runtime(&second, true);
    let error = settled
        .leave(second, replayed)
        .expect_err("a replayed invocation must not leave");
    assert!(
        error
            .diagnostic()
            .contains("does not complete the exact active entry"),
        "{}",
        error.diagnostic()
    );
    assert_eq!(
        settled.live_eras().collect::<Vec<_>>(),
        vec![(10, ComponentEraEntryState::Open, 1)]
    );

    // A foreign ledger's active entry and its honest leave receipt hold no
    // membership in this ledger.
    let mut foreign = foreign_ledger(4);
    foreign_publish(&mut foreign, 10, 700);
    let foreign_entry = foreign
        .enter(ComponentEraEntryReceipt::from_runtime(
            500,
            &foreign,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("foreign entry");
    let foreign_leave = ComponentEraLeaveReceipt::from_runtime(&foreign_entry, true);
    let mut ours = ledger(4);
    publish(&mut ours, 10, 100);
    let error = ours
        .leave(foreign_entry, foreign_leave)
        .expect_err("a foreign active entry must not leave here");
    assert!(
        error
            .diagnostic()
            .contains("does not complete the exact active entry"),
        "{}",
        error.diagnostic()
    );
}

#[test]
fn component_era_quiescence_receipt_rejects_every_one_field_substitution() {
    let reject = |mutate: Box<dyn FnOnce(&mut ComponentEraQuiescenceReceipt)>| {
        let mut ledger = ledger(4);
        publish_then_supersede(&mut ledger, 10, 20);
        let mut receipt = ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 0, true);
        mutate(&mut receipt);
        let error = ledger
            .establish_quiescence(receipt)
            .expect_err("a substituted quiescence receipt must reject");
        (ledger, error.diagnostic().to_owned())
    };

    // `era_identity` names the record the rest of the receipt replays
    // against: a non-live era fails the lookup, and the live-but-current
    // successor fails the Closing-state join.
    let (_, diagnostic) = reject(Box::new(|receipt| receipt.era_identity = 99));
    assert!(
        diagnostic.contains("not live"),
        "era_identity::foreign: {diagnostic}"
    );
    let cases: Vec<(
        &'static str,
        Box<dyn FnOnce(&mut ComponentEraQuiescenceReceipt)>,
    )> = vec![
        (
            "era_identity::current",
            Box::new(|receipt| {
                receipt.era_identity = 20;
            }),
        ),
        (
            "binding_contract_identity",
            Box::new(|receipt| {
                receipt.binding_contract_identity = "ForeignBinding/v1".into();
            }),
        ),
        (
            "residual_lifetime_cohort_holds",
            Box::new(|receipt| {
                receipt.residual_lifetime_cohort_holds = 1;
            }),
        ),
        (
            "all_dispositions_complete::false",
            Box::new(|receipt| {
                receipt.all_dispositions_complete = false;
            }),
        ),
    ];
    for (name, mutate) in cases {
        let (ledger, diagnostic) = reject(mutate);
        assert!(
            diagnostic.contains(
                "requires closing, zero entries, zero program-local root authority holds, and complete cohort disposition"
            ),
            "{name}: {diagnostic}"
        );
        assert_eq!(
            ledger.live_eras().collect::<Vec<_>>(),
            vec![
                (10, ComponentEraEntryState::Closing, 0),
                (20, ComponentEraEntryState::Open, 0),
            ],
            "{name}: rejection leaves the era closing, not quiescent"
        );
    }

    // The retained-state legs: an open era, an era with an active entry, and
    // an era with a live epoch-lease hold each reject the honest receipt.
    let mut open = ledger(4);
    publish(&mut open, 10, 100);
    let error = open
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            &open, 10, 0, true,
        ))
        .expect_err("an open era cannot quiesce");
    assert!(error.diagnostic().contains("requires closing"));

    let mut occupied = ledger(4);
    publish(&mut occupied, 10, 100);
    let _entry = occupied
        .enter(ComponentEraEntryReceipt::from_runtime(
            500,
            &occupied,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("entry");
    publish(&mut occupied, 20, 101);
    let error = occupied
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            &occupied, 10, 0, true,
        ))
        .expect_err("an era with an active entry cannot quiesce");
    assert!(error.diagnostic().contains("requires closing"));

    let mut held = ledger(4);
    publish(&mut held, 10, 100);
    let _lease = held
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("lease");
    publish(&mut held, 20, 101);
    let error = held
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            &held, 10, 0, true,
        ))
        .expect_err("an era with a live authority hold cannot quiesce");
    assert!(error.diagnostic().contains("requires closing"));

    // A receipt minted under a foreign ledger's contract identity cannot
    // quiesce this ledger's era.
    let mut foreign = foreign_ledger(4);
    foreign_publish_then_supersede(&mut foreign, 10, 20);
    let foreign_receipt = ComponentEraQuiescenceReceipt::from_runtime(&foreign, 10, 0, true);
    let mut ours = ledger(4);
    publish_then_supersede(&mut ours, 10, 20);
    let error = ours
        .establish_quiescence(foreign_receipt)
        .expect_err("a foreign-ledger quiescence receipt must reject");
    assert!(error.diagnostic().contains("requires closing"));
}

#[test]
fn component_era_retirement_receipt_rejects_every_one_field_substitution() {
    let reject = |mutate: Box<dyn FnOnce(&mut ComponentEraRetirementReceipt)>| {
        let mut ledger = ledger(4);
        publish_supersede_quiesce(&mut ledger, 10, 20);
        let mut receipt = ComponentEraRetirementReceipt::from_runtime(700, &ledger, 10, true);
        mutate(&mut receipt);
        let error = ledger
            .retire(receipt)
            .expect_err("a substituted retirement receipt must reject");
        (ledger, error.diagnostic().to_owned())
    };

    let (_, diagnostic) = reject(Box::new(|receipt| receipt.era_identity = 99));
    assert!(
        diagnostic.contains("not live"),
        "era_identity::foreign: {diagnostic}"
    );
    let cases: Vec<(
        &'static str,
        Box<dyn FnOnce(&mut ComponentEraRetirementReceipt)>,
    )> = vec![
        // The current era and the closing-but-not-quiescent era both fail
        // the noncurrent-quiescent join.
        (
            "era_identity::current",
            Box::new(|receipt| {
                receipt.era_identity = 20;
            }),
        ),
        (
            "retirement_identity::zero",
            Box::new(|receipt| {
                receipt.retirement_identity = 0;
            }),
        ),
        (
            "binding_contract_identity",
            Box::new(|receipt| {
                receipt.binding_contract_identity = "ForeignBinding/v1".into();
            }),
        ),
        (
            "lifetime_cohort_released::false",
            Box::new(|receipt| {
                receipt.lifetime_cohort_released = false;
            }),
        ),
    ];
    for (name, mutate) in cases {
        let (ledger, diagnostic) = reject(mutate);
        assert!(
            diagnostic
                .contains("requires a noncurrent quiescent released cohort and fresh receipt"),
            "{name}: {diagnostic}"
        );
        assert_eq!(
            ledger.live_eras().collect::<Vec<_>>(),
            vec![
                (10, ComponentEraEntryState::Quiescent, 0),
                (20, ComponentEraEntryState::Open, 0),
            ],
            "{name}: rejection keeps the quiescent era live"
        );
    }

    // The minted `retirement_identity` is adopted verbatim — a fresh nonce
    // retires — but the consumed set moves honestly: a second retirement
    // under the same identity rejects.
    let mut adopted = ledger(4);
    publish_supersede_quiesce(&mut adopted, 10, 20);
    adopted
        .retire(ComponentEraRetirementReceipt::from_runtime(
            700, &adopted, 10, true,
        ))
        .expect("fresh retirement identity");
    publish(&mut adopted, 30, 102);
    adopted
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            &adopted, 20, 0, true,
        ))
        .expect("second era quiesces");
    let error = adopted
        .retire(ComponentEraRetirementReceipt::from_runtime(
            700, &adopted, 20, true,
        ))
        .expect_err("a replayed retirement identity must reject");
    assert!(
        error.diagnostic().contains("fresh receipt"),
        "{}",
        error.diagnostic()
    );

    // A Closing era that was never quiesced cannot retire; a live
    // epoch-lease hold blocks retirement outright.
    let mut open = ledger(4);
    publish_then_supersede(&mut open, 10, 20);
    let error = open
        .retire(ComponentEraRetirementReceipt::from_runtime(
            700, &open, 10, true,
        ))
        .expect_err("a non-quiescent era cannot retire");
    assert!(error.diagnostic().contains("noncurrent quiescent"));

    let mut held = ledger(4);
    publish(&mut held, 10, 100);
    let _lease = held
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("lease");
    publish(&mut held, 20, 101);
    let error = held
        .retire(ComponentEraRetirementReceipt::from_runtime(
            700, &held, 10, true,
        ))
        .expect_err("a live authority hold blocks retirement");
    assert!(
        error.diagnostic().contains("epoch leases remain live"),
        "{}",
        error.diagnostic()
    );

    // A receipt minted under a foreign ledger's contract identity cannot
    // retire this ledger's era.
    let mut foreign = foreign_ledger(4);
    foreign_publish_supersede_quiesce(&mut foreign, 10, 20);
    let foreign_receipt = ComponentEraRetirementReceipt::from_runtime(700, &foreign, 10, true);
    let mut ours = ledger(4);
    publish_supersede_quiesce(&mut ours, 10, 20);
    let error = ours
        .retire(foreign_receipt)
        .expect_err("a foreign-ledger retirement receipt must reject");
    assert!(error.diagnostic().contains("noncurrent quiescent"));
}

/// Acquire one honest lease, apply one substitution, and replay release.
/// Rejection keeps the era's authority hold.
fn rejected_lease_release(
    mutate: impl FnOnce(&mut ProgramLocalRootEpochLease),
) -> (ComponentEraEntryLedger, String) {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    let mut lease = ledger
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("lease");
    mutate(&mut lease);
    let error = ledger
        .release_program_local_root_epoch_lease(lease)
        .expect_err("a substituted lease must not release");
    let diagnostic = error.diagnostic().to_owned();
    assert_eq!(
        ledger.program_local_root_authority_holds(10),
        Some(1),
        "rejection keeps the era's authority hold"
    );
    (ledger, diagnostic)
}

#[test]
fn program_local_root_epoch_lease_rejects_every_one_field_substitution() {
    // Every binding the lease carries replays against the retained era
    // record and the ledger's own identities at release.
    let cases: Vec<(
        &'static str,
        Box<dyn FnOnce(&mut ProgramLocalRootEpochLease)>,
    )> = vec![
        (
            "identity::unissued",
            Box::new(|lease| {
                lease.identity = lease_id(999);
            }),
        ),
        (
            "ledger",
            Box::new(|lease| {
                lease.ledger =
                    ComponentEraLedgerId::from_normalized_identity(2).expect("ledger identity");
            }),
        ),
        (
            "binding_contract_identity",
            Box::new(|lease| {
                lease.binding_contract_identity = "ForeignBinding/v1".into();
            }),
        ),
        (
            "entry_contract_identity",
            Box::new(|lease| {
                lease.entry_contract_identity = "ForeignEntry/v1".into();
            }),
        ),
        (
            "artifact_occurrence_digest",
            Box::new(|lease| {
                lease.artifact_occurrence_digest =
                    InstalledArtifactOccurrenceDigest::from_sha256([0xEE; 32]);
            }),
        ),
        (
            "artifact_instance_compatibility_report_identity",
            Box::new(|lease| {
                lease.artifact_instance_compatibility_report_identity = 4_444;
            }),
        ),
        (
            "entry_plan_identity",
            Box::new(|lease| {
                lease.entry_plan_identity = "entry-plan:foreign".into();
            }),
        ),
        (
            "entry_plan_admission_receipt_identity",
            Box::new(|lease| {
                lease.entry_plan_admission_receipt_identity = "receipt:entry-plan:foreign".into();
            }),
        ),
        (
            "candidate::wholesale",
            Box::new(|lease| {
                lease.candidate = candidate(20);
            }),
        ),
        (
            "candidate::inner",
            Box::new(|lease| {
                lease.candidate.executable_tcb_acceptance = acceptance("substituted-era", 999);
            }),
        ),
    ];
    for (name, mutate) in cases {
        let (_, diagnostic) = rejected_lease_release(mutate);
        assert!(
            diagnostic.contains("does not belong to this exact published era"),
            "{name}: {diagnostic}"
        );
    }

    // An era identity outside the live roster fails the record lookup
    // itself; a lease rebound to another live era binds the wrong retained
    // candidate.
    let (_, diagnostic) = rejected_lease_release(|lease| lease.era_identity = 99);
    assert!(
        diagnostic.contains("outside this ledger"),
        "era_identity::foreign: {diagnostic}"
    );
    let mut two_eras = ledger(4);
    publish(&mut two_eras, 10, 100);
    publish(&mut two_eras, 20, 101);
    let mut lease = two_eras
        .acquire_program_local_root_epoch_lease(lease_id(900), 20, "CodecEntry/v1")
        .expect("current-era lease");
    lease.era_identity = 10;
    let error = two_eras
        .release_program_local_root_epoch_lease(lease)
        .expect_err("a lease rebound to another era must not release");
    assert!(
        error
            .diagnostic()
            .contains("does not belong to this exact published era"),
        "{}",
        error.diagnostic()
    );

    // The minted lease `identity` is a nonce the ledger adopts verbatim: a
    // fresh identity acquires, while substituting another live hold's
    // identity is indistinguishable from releasing that hold — the release
    // succeeds and the era's hold count moves honestly.
    let mut shared = ledger(4);
    publish(&mut shared, 10, 100);
    let mut lease = shared
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("first lease");
    let _other = shared
        .acquire_program_local_root_epoch_lease(lease_id(901), 10, "CodecEntry/v1")
        .expect("second lease");
    lease.identity = lease_id(901);
    shared
        .release_program_local_root_epoch_lease(lease)
        .expect("a live hold identity releases that exact hold");
    assert_eq!(shared.program_local_root_authority_holds(10), Some(1));

    // The stricter `validate` replay additionally requires the era to be the
    // current open one: a substituted field rejects, and so does the honest
    // lease once a routing switch closes its era — while `release` still
    // accepts it, so custody is never trapped.
    let mut validating = ledger(4);
    publish(&mut validating, 10, 100);
    let mut lease = validating
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("lease");
    lease.artifact_instance_compatibility_report_identity = 4_444;
    let diagnostic = validating
        .validate_program_local_root_epoch_lease(&lease)
        .expect_err("a substituted lease must not validate");
    assert!(
        diagnostic.contains("not live in the exact current open era"),
        "{diagnostic}"
    );
    lease.artifact_instance_compatibility_report_identity = 1_010;
    publish(&mut validating, 20, 101);
    let diagnostic = validating
        .validate_program_local_root_epoch_lease(&lease)
        .expect_err("a closing era cannot establish fresh authority");
    assert!(
        diagnostic.contains("not live in the exact current open era"),
        "{diagnostic}"
    );
    validating
        .release_program_local_root_epoch_lease(lease)
        .expect("a closing era still releases its exact hold");

    // Acquisition replays its own axes: consumed identities, non-current or
    // unpublished eras, and contract identities that diverge from either the
    // ledger or the retained candidate.
    let mut acquiring = ledger(4);
    publish(&mut acquiring, 10, 100);
    let _lease = acquiring
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("lease");
    let error = acquiring
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect_err("a replayed lease identity must reject");
    assert!(
        error.diagnostic().contains("already issued or consumed"),
        "{}",
        error.diagnostic()
    );
    for (name, era, contract, fragment) in [
        ("era::unpublished", 99, "CodecEntry/v1", "exact current"),
        ("contract::empty", 10, "", "exact open"),
        ("contract::foreign", 10, "ForeignEntry/v1", "exact open"),
    ] {
        let error = acquiring
            .acquire_program_local_root_epoch_lease(lease_id(901), era, contract)
            .expect_err("a substituted acquisition must reject");
        assert!(
            error.diagnostic().contains(fragment),
            "{name}: {}",
            error.diagnostic()
        );
    }
    publish(&mut acquiring, 20, 101);
    let error = acquiring
        .acquire_program_local_root_epoch_lease(lease_id(902), 10, "CodecEntry/v1")
        .expect_err("a superseded era cannot issue fresh authority");
    assert!(
        error.diagnostic().contains("exact current"),
        "{}",
        error.diagnostic()
    );
}
