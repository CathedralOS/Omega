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
//!
//! Each replay answers a named `*Rejection` decision rather than a
//! diagnostic string, so every representable field of a family declares one
//! `*FieldForTest` variant — multiple variants per field where the axes
//! differ — and runs through `run_one_field_substitution_matrix`. The legs
//! that need ledger staging the honest record cannot carry — a predecessor
//! to close, an already-consumed identity, an entry already settled, an era
//! held open by an occupied entry or live lease — stay authored beside the
//! matrices that pin them.

use super::{
    ActiveComponentEraEntry, ComponentEraCandidate, ComponentEraEntryLedger,
    ComponentEraEntryReceipt, ComponentEraEntryState, ComponentEraLeaveReceipt,
    ComponentEraLedgerId, ComponentEraPublicationReceipt, ComponentEraQuiescenceReceipt,
    ComponentEraRetirementReceipt, EraEntryRejection, EraLeaveRejection, EraPublicationRejection,
    EraQuiescenceRejection, EraRetirementRejection, ExecutableTcbProfileAcceptance,
    InstalledArtifactOccurrenceDigest, ProgramLocalRootEpochLease,
    ProgramLocalRootEpochLeaseAcquisitionRejection, ProgramLocalRootEpochLeaseId,
    ProgramLocalRootEpochLeaseReleaseRejection, ProgramLocalRootEpochLeaseValidationRejection,
};
use crate::{
    ExecutableTcbManifest, ExecutableTcbProfile, ExecutionScope, IncompleteScopePolicy,
    ScopeCompleteness, evaluate_executable_tcb_profile,
};
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
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

/// The publication join `publish` replays: the candidate argument plus the
/// receipt claiming to bind and expose it. The receipt carries its own copy
/// of the candidate, so substituting either side of the pair diverges the
/// binding join.
#[derive(Debug, Clone, PartialEq)]
struct PublicationJoin {
    candidate: ComponentEraCandidate,
    receipt: ComponentEraPublicationReceipt,
}

/// An honest first-era publication on an empty ledger.
fn honest_publication() -> PublicationJoin {
    let staged = ledger(4);
    let candidate = candidate(10);
    PublicationJoin {
        receipt: ComponentEraPublicationReceipt::from_runtime(
            100, &staged, &candidate, true, false,
        ),
        candidate,
    }
}

/// An authentic publication minted under `foreign_ledger`'s contract
/// identities: donor evidence whose retained custody must differ.
fn foreign_publication() -> PublicationJoin {
    let mut foreign = foreign_ledger(4);
    foreign_publish(&mut foreign, 10, 700);
    let candidate = foreign_candidate(20);
    PublicationJoin {
        receipt: ComponentEraPublicationReceipt::from_runtime(
            701, &foreign, &candidate, true, true,
        ),
        candidate,
    }
}

/// The publication family's independent checker: replay `publish` on a fresh
/// empty ledger. Rejection must leave the containing state untouched.
fn replay_publication(join: &PublicationJoin) -> Result<PublicationJoin, EraPublicationRejection> {
    let mut ledger = ledger(4);
    match ledger.publish(join.candidate.clone(), join.receipt.clone()) {
        Ok(()) => Ok(join.clone()),
        Err(error) => {
            assert_eq!(ledger.current_era(), None, "rejection retains no era");
            assert!(
                ledger.live_eras().next().is_none(),
                "rejection retains no live era record"
            );
            Err(error.rejection())
        }
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable field of [`ComponentEraPublicationReceipt`]; checked
    /// by `publish`'s replay of every copied coordinate against the retained
    /// candidate argument and the ledger's own published identities.
    /// `previous_era_closed` binds only once a predecessor exists, so its leg
    /// needs the routing-switch staging and stays authored in the test — as
    /// do the consumed-identity replay and the already-live and
    /// retention-limit rejections.
    pub enum PublicationReceiptFieldForTest {
        // `publication_identity = 0`: the consumed-identity precheck.
        PublicationIdentity,
        BindingContractIdentity,
        EntryContractIdentity,
        // `None` -> `Some(10)`: claims a predecessor that never existed.
        PreviousEraIdentity,
        CandidateEraIdentity,
        CandidateArtifactOccurrenceDigest,
        CandidateCompatibilityReportIdentity,
        CandidateEntryPlanIdentity,
        CandidateEntryPlanReceiptIdentity,
        // The retained whole-candidate copy swapped wholesale.
        CandidateWholesale,
        // One inner field of the retained candidate copy.
        RetainedCandidateEntryContract,
        RetainedCandidateCompatibilityReport,
        NewEraVisible,
        // An authentic receipt minted under a foreign ledger's contracts.
        ForeignReceipt,
    }
}

/// Substitute exactly one receipt field; the published candidate argument
/// stays honest, so only the replay can expose the drift.
fn substitute_publication_receipt_for_test(
    join: &mut PublicationJoin,
    field: PublicationReceiptFieldForTest,
    donor: &PublicationJoin,
) {
    use PublicationReceiptFieldForTest as Field;
    match field {
        Field::PublicationIdentity => join.receipt.publication_identity = 0,
        Field::BindingContractIdentity => {
            join.receipt.binding_contract_identity = "ForeignBinding/v1".into()
        }
        Field::EntryContractIdentity => {
            join.receipt.entry_contract_identity = "ForeignEntry/v1".into()
        }
        Field::PreviousEraIdentity => join.receipt.previous_era_identity = Some(10),
        Field::CandidateEraIdentity => join.receipt.candidate_era_identity = 99,
        Field::CandidateArtifactOccurrenceDigest => {
            join.receipt.candidate_artifact_occurrence_digest =
                InstalledArtifactOccurrenceDigest::from_sha256([0xEE; 32])
        }
        Field::CandidateCompatibilityReportIdentity => {
            join.receipt
                .candidate_artifact_instance_compatibility_report_identity += 1
        }
        Field::CandidateEntryPlanIdentity => {
            join.receipt.candidate_entry_plan_identity = "entry-plan:foreign".into()
        }
        Field::CandidateEntryPlanReceiptIdentity => {
            join.receipt.candidate_entry_plan_admission_receipt_identity =
                "receipt:entry-plan:foreign".into()
        }
        Field::CandidateWholesale => join.receipt.candidate = candidate(99),
        Field::RetainedCandidateEntryContract => {
            join.receipt.candidate.entry_contract_identity = "OtherEntry/v1".into()
        }
        Field::RetainedCandidateCompatibilityReport => {
            join.receipt
                .candidate
                .artifact_instance_compatibility_report_identity = 4_444
        }
        Field::NewEraVisible => join.receipt.new_era_visible = false,
        Field::ForeignReceipt => join.receipt = donor.receipt.clone(),
    }
}

/// Every leg rejects inside the binding join except the zeroed publication
/// identity, which the consumed-identity precheck refuses first.
fn publication_receipt_outcome(
    field: PublicationReceiptFieldForTest,
) -> MutationOutcome<EraPublicationRejection> {
    MutationOutcome::ExactError(match field {
        PublicationReceiptFieldForTest::PublicationIdentity => {
            EraPublicationRejection::ZeroOrReplayedPublicationIdentity
        }
        _ => EraPublicationRejection::CandidateBindingJoin,
    })
}

optimization_core::custody_field_inventory! {
    /// One substitutable field of the [`ComponentEraCandidate`] publish
    /// argument; the receipt retains the honest copy, so every substitution
    /// diverges the binding join — or trips the completeness precheck, or the
    /// embedded coexisting-TCB admission replay. The already-live era and
    /// retention-limit rejections need staged ledger state and stay authored
    /// in the test.
    pub enum PublicationCandidateFieldForTest {
        // `era_identity = 0`: the completeness precheck.
        EraIdentityZeroed,
        EraIdentitySubstituted,
        ArtifactOccurrenceDigest,
        CompatibilityReportIdentityZeroed,
        CompatibilityReportIdentitySubstituted,
        BindingContractIdentitySubstituted,
        BindingContractIdentityCleared,
        EntryContractIdentitySubstituted,
        EntryContractIdentityCleared,
        EntryPlanIdentitySubstituted,
        EntryPlanIdentityCleared,
        EntryPlanReceiptIdentitySubstituted,
        EntryPlanReceiptIdentityCleared,
        // Same-scope acceptance evidence while the receipt still binds the
        // honest candidate.
        ExecutableTcbAcceptanceSubstituted,
        // A scope-drifting acceptance honestly rebound into the receipt:
        // the binding join clears and the coexisting-TCB admission refuses.
        ExecutableTcbAcceptanceHonestlyRebound,
    }
}

/// Substitute exactly one candidate field; the receipt retains the copy
/// minted over the honest candidate — except `HonestlyRebound`, which
/// remints the containing receipt so the substitution reaches the embedded
/// TCB admission replay.
fn substitute_publication_candidate_for_test(
    join: &mut PublicationJoin,
    field: PublicationCandidateFieldForTest,
    _donor: &PublicationJoin,
) {
    use PublicationCandidateFieldForTest as Field;
    match field {
        Field::EraIdentityZeroed => join.candidate.era_identity = 0,
        Field::EraIdentitySubstituted => join.candidate.era_identity = 99,
        Field::ArtifactOccurrenceDigest => {
            join.candidate.artifact_occurrence_digest =
                InstalledArtifactOccurrenceDigest::from_sha256([0x77; 32])
        }
        Field::CompatibilityReportIdentityZeroed => {
            join.candidate
                .artifact_instance_compatibility_report_identity = 0
        }
        Field::CompatibilityReportIdentitySubstituted => {
            join.candidate
                .artifact_instance_compatibility_report_identity += 1
        }
        Field::BindingContractIdentitySubstituted => {
            join.candidate.binding_contract_identity = "ForeignBinding/v1".into()
        }
        Field::BindingContractIdentityCleared => join.candidate.binding_contract_identity.clear(),
        Field::EntryContractIdentitySubstituted => {
            join.candidate.entry_contract_identity = "ForeignEntry/v1".into()
        }
        Field::EntryContractIdentityCleared => join.candidate.entry_contract_identity.clear(),
        Field::EntryPlanIdentitySubstituted => {
            join.candidate.entry_plan_identity = "entry-plan:foreign".into()
        }
        Field::EntryPlanIdentityCleared => join.candidate.entry_plan_identity.clear(),
        Field::EntryPlanReceiptIdentitySubstituted => {
            join.candidate.entry_plan_admission_receipt_identity = "receipt:foreign".into()
        }
        Field::EntryPlanReceiptIdentityCleared => {
            join.candidate.entry_plan_admission_receipt_identity.clear()
        }
        Field::ExecutableTcbAcceptanceSubstituted => {
            join.candidate.executable_tcb_acceptance = acceptance("substituted-era", 999)
        }
        Field::ExecutableTcbAcceptanceHonestlyRebound => {
            join.candidate.executable_tcb_acceptance = isolated_acceptance();
            let staged = ledger(4);
            join.receipt = ComponentEraPublicationReceipt::from_runtime(
                join.receipt.publication_identity,
                &staged,
                &join.candidate,
                join.receipt.new_era_visible,
                join.receipt.previous_era_closed,
            );
        }
    }
}

/// The completeness-precheck legs name `IncompleteCandidate`, the honestly
/// rebound scope drift names the embedded TCB admission's exact refusal, and
/// every other leg diverges the binding join.
fn publication_candidate_outcome(
    field: PublicationCandidateFieldForTest,
) -> MutationOutcome<EraPublicationRejection> {
    use PublicationCandidateFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::EraIdentityZeroed
        | Field::CompatibilityReportIdentityZeroed
        | Field::BindingContractIdentityCleared
        | Field::EntryContractIdentityCleared
        | Field::EntryPlanIdentityCleared
        | Field::EntryPlanReceiptIdentityCleared => EraPublicationRejection::IncompleteCandidate,
        Field::ExecutableTcbAcceptanceHonestlyRebound => {
            EraPublicationRejection::ExecutableTcbAdmission(
                "accepted executable manifest scope IsolatedProvider(77) does not match \
                 coexistence scope CallerAddressSpace"
                    .into(),
            )
        }
        _ => EraPublicationRejection::CandidateBindingJoin,
    })
}

#[test]
fn component_era_publication_receipt_rejects_every_one_field_substitution() {
    // Every receipt field copied at minting replays against the retained
    // candidate argument and the ledger's own identities.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "component era publication receipt",
        fields: PublicationReceiptFieldForTest::INVENTORY,
        honest: &honest_publication,
        donor: foreign_publication(),
        custody: &Clone::clone,
        substitute: &substitute_publication_receipt_for_test,
        check: &replay_publication,
        outcome: &publication_receipt_outcome,
        joined_replay: None,
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
    assert_eq!(
        error.rejection(),
        EraPublicationRejection::PreviousEraNotClosed
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
    assert_eq!(
        error.rejection(),
        EraPublicationRejection::CandidateBindingJoin
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
    assert_eq!(
        error.rejection(),
        EraPublicationRejection::ZeroOrReplayedPublicationIdentity
    );
}

#[test]
fn component_era_candidate_rejects_every_one_field_substitution() {
    // The publish argument is the family's other half: every candidate field
    // is bound either by the completeness precheck, by the ledger's contract
    // identities, or by the receipt's copied coordinates.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "component era publication candidate",
        fields: PublicationCandidateFieldForTest::INVENTORY,
        honest: &honest_publication,
        donor: foreign_publication(),
        custody: &Clone::clone,
        substitute: &substitute_publication_candidate_for_test,
        check: &replay_publication,
        outcome: &publication_candidate_outcome,
        joined_replay: None,
    });

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
    assert_eq!(error.rejection(), EraPublicationRejection::EraAlreadyLive);

    let mut capped = ledger(1);
    publish(&mut capped, 10, 100);
    let overflow = candidate(20);
    let receipt = ComponentEraPublicationReceipt::from_runtime(101, &capped, &overflow, true, true);
    let error = capped
        .publish(overflow, receipt)
        .expect_err("a publication beyond the retention limit must reject");
    assert_eq!(
        error.rejection(),
        EraPublicationRejection::LiveEraRetentionLimitExceeded
    );
}

/// An honest entry receipt minted for the current open era.
fn honest_entry_receipt() -> ComponentEraEntryReceipt {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    ComponentEraEntryReceipt::from_runtime(500, &ledger, 10, "entry-plan:10".into(), true)
}

/// An authentic entry receipt minted under `foreign_ledger`'s contracts.
fn foreign_entry_receipt() -> ComponentEraEntryReceipt {
    let mut foreign = foreign_ledger(4);
    foreign_publish(&mut foreign, 10, 700);
    ComponentEraEntryReceipt::from_runtime(500, &foreign, 10, "entry-plan:10".into(), true)
}

/// The entry family's independent checker: replay `enter` on a ledger whose
/// era 10 is the open current one. Rejection keeps the era and records no
/// entry.
fn replay_entry(
    receipt: &ComponentEraEntryReceipt,
) -> Result<ComponentEraEntryReceipt, EraEntryRejection> {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    match ledger.enter(receipt.clone()) {
        Ok(_entry) => Ok(receipt.clone()),
        Err(error) => {
            assert_eq!(
                ledger.live_eras().collect::<Vec<_>>(),
                vec![(10, ComponentEraEntryState::Open, 0)],
                "rejection leaves the era open with no entry recorded"
            );
            Err(error.rejection())
        }
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable field of [`ComponentEraEntryReceipt`]; checked by
    /// `enter`'s replay of every copied coordinate against the current open
    /// era's retained record. The no-published-era lookup, the consumed
    /// invocation replay, and the closing-era leg need different retained
    /// state and stay authored in the test.
    pub enum EntryReceiptFieldForTest {
        // `invocation_identity = 0`: the fresh-nonce clause.
        InvocationIdentity,
        BindingContractIdentity,
        EntryContractIdentity,
        // Names the never-published sibling era.
        ResolvedEraIdentityStale,
        // Names an era outside this ledger's roster.
        ResolvedEraIdentityForeign,
        EntryPlanIdentity,
        // `entry_linearized = false`: the linearization clause.
        EntryLinearizationDropped,
        // An authentic receipt minted under a foreign ledger's contracts.
        ForeignReceipt,
    }
}

/// Substitute exactly one entry receipt field.
fn substitute_entry_receipt_for_test(
    receipt: &mut ComponentEraEntryReceipt,
    field: EntryReceiptFieldForTest,
    donor: &ComponentEraEntryReceipt,
) {
    use EntryReceiptFieldForTest as Field;
    match field {
        Field::InvocationIdentity => receipt.invocation_identity = 0,
        Field::BindingContractIdentity => {
            receipt.binding_contract_identity = "ForeignBinding/v1".into()
        }
        Field::EntryContractIdentity => receipt.entry_contract_identity = "ForeignEntry/v1".into(),
        Field::ResolvedEraIdentityStale => receipt.resolved_era_identity = 20,
        Field::ResolvedEraIdentityForeign => receipt.resolved_era_identity = 99,
        Field::EntryPlanIdentity => receipt.entry_plan_identity = "entry-plan:foreign".into(),
        Field::EntryLinearizationDropped => receipt.entry_linearized = false,
        Field::ForeignReceipt => *receipt = donor.clone(),
    }
}

fn entry_receipt_outcome(_field: EntryReceiptFieldForTest) -> MutationOutcome<EraEntryRejection> {
    MutationOutcome::ExactError(EraEntryRejection::ExactLinearizationJoin)
}

#[test]
fn component_era_entry_receipt_rejects_every_one_field_substitution() {
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "component era entry receipt",
        fields: EntryReceiptFieldForTest::INVENTORY,
        honest: &honest_entry_receipt,
        donor: foreign_entry_receipt(),
        custody: &Clone::clone,
        substitute: &substitute_entry_receipt_for_test,
        check: &replay_entry,
        outcome: &entry_receipt_outcome,
        joined_replay: None,
    });

    // With no published era there is no current record to bind at all.
    let mut empty = ledger(4);
    let unpublished =
        ComponentEraEntryReceipt::from_runtime(500, &empty, 10, "entry-plan:10".into(), true);
    let error = empty
        .enter(unpublished)
        .expect_err("an entry without a published era must reject");
    assert_eq!(error.rejection(), EraEntryRejection::NoPublishedEra);

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
    assert_eq!(error.rejection(), EraEntryRejection::ExactLinearizationJoin);
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
    assert_eq!(error.rejection(), EraEntryRejection::ExactLinearizationJoin);
}

/// The leave join `leave` replays: the consumed entry handle plus the
/// receipt claiming to complete it.
#[derive(Debug)]
struct LeaveJoin {
    entry: ActiveComponentEraEntry,
    receipt: ComponentEraLeaveReceipt,
}

/// The retained-custody view of a staged leave: the handle's fields plus the
/// whole receipt.
type LeaveCustody = (u64, String, u64, String, ComponentEraLeaveReceipt);

fn leave_custody(join: &LeaveJoin) -> LeaveCustody {
    (
        join.entry.invocation_identity,
        join.entry.binding_contract_identity.clone(),
        join.entry.era_identity,
        join.entry.entry_plan_identity.clone(),
        join.receipt.clone(),
    )
}

/// An honest entry-and-leave pair staged on era 10.
fn honest_leave() -> LeaveJoin {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    let entry = ledger
        .enter(ComponentEraEntryReceipt::from_runtime(
            500,
            &ledger,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("honest entry");
    let receipt = ComponentEraLeaveReceipt::from_runtime(&entry, true);
    LeaveJoin { entry, receipt }
}

/// An authentic entry-and-leave pair minted under `foreign_ledger`'s
/// contracts.
fn foreign_leave() -> LeaveJoin {
    let mut foreign = foreign_ledger(4);
    foreign_publish(&mut foreign, 10, 700);
    let entry = foreign
        .enter(ComponentEraEntryReceipt::from_runtime(
            500,
            &foreign,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("foreign entry");
    let receipt = ComponentEraLeaveReceipt::from_runtime(&entry, true);
    LeaveJoin { entry, receipt }
}

/// `leave` consumes the handle, which is deliberately non-clonable: the
/// replay rebuilds the substituted record field-for-field.
fn replayed_entry(entry: &ActiveComponentEraEntry) -> ActiveComponentEraEntry {
    ActiveComponentEraEntry {
        invocation_identity: entry.invocation_identity,
        binding_contract_identity: entry.binding_contract_identity.clone(),
        era_identity: entry.era_identity,
        entry_plan_identity: entry.entry_plan_identity.clone(),
    }
}

/// The leave family's independent checker: replay `leave` on a ledger whose
/// era 10 holds one active entry. Rejection leaves the entry active.
fn replay_leave(join: &LeaveJoin) -> Result<LeaveCustody, EraLeaveRejection> {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    let _staged = ledger
        .enter(ComponentEraEntryReceipt::from_runtime(
            500,
            &ledger,
            10,
            "entry-plan:10".into(),
            true,
        ))
        .expect("staging entry");
    match ledger.leave(replayed_entry(&join.entry), join.receipt.clone()) {
        Ok(()) => Ok(leave_custody(join)),
        Err(error) => {
            assert_eq!(
                ledger.live_eras().collect::<Vec<_>>(),
                vec![(10, ComponentEraEntryState::Open, 1)],
                "rejection leaves the entry active"
            );
            Err(error.rejection())
        }
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable field of the leave join — the
    /// [`ComponentEraLeaveReceipt`] and the consumed
    /// [`ActiveComponentEraEntry`] handle it must complete — checked by
    /// `leave`'s replay of the receipt against the exact active entry. The
    /// settled-invocation replay and the foreign-membership leg need staged
    /// ledger state and stay authored in the test.
    pub enum LeaveJoinFieldForTest {
        ReceiptInvocationIdentity,
        ReceiptBindingContractIdentity,
        ReceiptEraIdentity,
        ReceiptEntryPlanIdentity,
        // `leave_completed = false`: the completion clause.
        ReceiptLeaveIncomplete,
        EntryInvocationIdentity,
        EntryBindingContractIdentity,
        EntryEntryPlanIdentity,
        // An era outside the live roster: the record lookup itself fails.
        EntryEraIdentityForeign,
    }
}

/// Substitute exactly one field of the leave join — receipt side or entry
/// handle side.
fn substitute_leave_for_test(
    join: &mut LeaveJoin,
    field: LeaveJoinFieldForTest,
    _donor: &LeaveJoin,
) {
    use LeaveJoinFieldForTest as Field;
    match field {
        Field::ReceiptInvocationIdentity => join.receipt.invocation_identity = 501,
        Field::ReceiptBindingContractIdentity => {
            join.receipt.binding_contract_identity = "ForeignBinding/v1".into()
        }
        Field::ReceiptEraIdentity => join.receipt.era_identity = 20,
        Field::ReceiptEntryPlanIdentity => {
            join.receipt.entry_plan_identity = "entry-plan:foreign".into()
        }
        Field::ReceiptLeaveIncomplete => join.receipt.leave_completed = false,
        Field::EntryInvocationIdentity => join.entry.invocation_identity = 501,
        Field::EntryBindingContractIdentity => {
            join.entry.binding_contract_identity = "ForeignBinding/v1".into()
        }
        Field::EntryEntryPlanIdentity => {
            join.entry.entry_plan_identity = "entry-plan:foreign".into()
        }
        Field::EntryEraIdentityForeign => join.entry.era_identity = 99,
    }
}

/// Only the out-of-roster era escapes the exact-entry join: it fails the
/// record lookup instead.
fn leave_outcome(field: LeaveJoinFieldForTest) -> MutationOutcome<EraLeaveRejection> {
    MutationOutcome::ExactError(match field {
        LeaveJoinFieldForTest::EntryEraIdentityForeign => EraLeaveRejection::EraNotLive,
        _ => EraLeaveRejection::ExactActiveEntryJoin,
    })
}

#[test]
fn component_era_leave_receipt_rejects_every_one_field_substitution() {
    // Every receipt field is replayed against the exact active entry handle;
    // mutating either side of the join rejects.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "component era leave join",
        fields: LeaveJoinFieldForTest::INVENTORY,
        honest: &honest_leave,
        donor: foreign_leave(),
        custody: &leave_custody,
        substitute: &substitute_leave_for_test,
        check: &replay_leave,
        outcome: &leave_outcome,
        joined_replay: None,
    });

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
    assert_eq!(error.rejection(), EraLeaveRejection::ExactActiveEntryJoin);
    assert_eq!(
        settled.live_eras().collect::<Vec<_>>(),
        vec![(10, ComponentEraEntryState::Open, 1)]
    );

    // A foreign ledger's active entry and its honest leave receipt hold no
    // membership in this ledger — the join binds entry to receipt and
    // membership, so an authentic foreign pair only fails when this ledger
    // never entered the invocation.
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
    let foreign_receipt = ComponentEraLeaveReceipt::from_runtime(&foreign_entry, true);
    let mut ours = ledger(4);
    publish(&mut ours, 10, 100);
    let error = ours
        .leave(foreign_entry, foreign_receipt)
        .expect_err("a foreign active entry must not leave here");
    assert_eq!(error.rejection(), EraLeaveRejection::ExactActiveEntryJoin);
}

/// An honest quiescence receipt naming the Closing era.
fn honest_quiescence_receipt() -> ComponentEraQuiescenceReceipt {
    let mut ledger = ledger(4);
    publish_then_supersede(&mut ledger, 10, 20);
    ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 0, true)
}

/// An authentic quiescence receipt minted under `foreign_ledger`'s
/// contracts.
fn foreign_quiescence_receipt() -> ComponentEraQuiescenceReceipt {
    let mut foreign = foreign_ledger(4);
    foreign_publish_then_supersede(&mut foreign, 10, 20);
    ComponentEraQuiescenceReceipt::from_runtime(&foreign, 10, 0, true)
}

/// The quiescence family's independent checker: replay
/// `establish_quiescence` on a ledger where era 10 is Closing under era 20.
/// Rejection leaves the era closing, not quiescent.
fn replay_quiescence(
    receipt: &ComponentEraQuiescenceReceipt,
) -> Result<ComponentEraQuiescenceReceipt, EraQuiescenceRejection> {
    let mut ledger = ledger(4);
    publish_then_supersede(&mut ledger, 10, 20);
    match ledger.establish_quiescence(receipt.clone()) {
        Ok(()) => Ok(receipt.clone()),
        Err(error) => {
            assert_eq!(
                ledger.live_eras().collect::<Vec<_>>(),
                vec![
                    (10, ComponentEraEntryState::Closing, 0),
                    (20, ComponentEraEntryState::Open, 0),
                ],
                "rejection leaves the era closing, not quiescent"
            );
            Err(error.rejection())
        }
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable field of [`ComponentEraQuiescenceReceipt`]; checked
    /// by `establish_quiescence`'s replay against the retained era record.
    /// The open, occupied, and lease-held eras reject on retained state the
    /// honest record cannot carry, and stay authored in the test.
    pub enum QuiescenceReceiptFieldForTest {
        // An era outside the live roster: the record lookup fails.
        EraIdentityForeign,
        // The live-but-current successor fails the Closing-state join.
        EraIdentityCurrent,
        BindingContractIdentity,
        // `residual_lifetime_cohort_holds = 1`: the drained-cohort clause.
        ResidualCohortHolds,
        // `all_dispositions_complete = false`: the disposition clause.
        DispositionsIncomplete,
        // An authentic receipt minted under a foreign ledger's contracts.
        ForeignReceipt,
    }
}

/// Substitute exactly one quiescence receipt field.
fn substitute_quiescence_receipt_for_test(
    receipt: &mut ComponentEraQuiescenceReceipt,
    field: QuiescenceReceiptFieldForTest,
    donor: &ComponentEraQuiescenceReceipt,
) {
    use QuiescenceReceiptFieldForTest as Field;
    match field {
        Field::EraIdentityForeign => receipt.era_identity = 99,
        Field::EraIdentityCurrent => receipt.era_identity = 20,
        Field::BindingContractIdentity => {
            receipt.binding_contract_identity = "ForeignBinding/v1".into()
        }
        Field::ResidualCohortHolds => receipt.residual_lifetime_cohort_holds = 1,
        Field::DispositionsIncomplete => receipt.all_dispositions_complete = false,
        Field::ForeignReceipt => *receipt = donor.clone(),
    }
}

/// Only the out-of-roster era escapes the quiescence join: it fails the
/// record lookup instead.
fn quiescence_receipt_outcome(
    field: QuiescenceReceiptFieldForTest,
) -> MutationOutcome<EraQuiescenceRejection> {
    MutationOutcome::ExactError(match field {
        QuiescenceReceiptFieldForTest::EraIdentityForeign => EraQuiescenceRejection::EraNotLive,
        _ => EraQuiescenceRejection::ClosingCohortJoin,
    })
}

#[test]
fn component_era_quiescence_receipt_rejects_every_one_field_substitution() {
    // `era_identity` names the record the rest of the receipt replays
    // against; every copied coordinate then joins the Closing-era record.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "component era quiescence receipt",
        fields: QuiescenceReceiptFieldForTest::INVENTORY,
        honest: &honest_quiescence_receipt,
        donor: foreign_quiescence_receipt(),
        custody: &Clone::clone,
        substitute: &substitute_quiescence_receipt_for_test,
        check: &replay_quiescence,
        outcome: &quiescence_receipt_outcome,
        joined_replay: None,
    });

    // The retained-state legs: an open era, an era with an active entry, and
    // an era with a live epoch-lease hold each reject the honest receipt.
    let mut open = ledger(4);
    publish(&mut open, 10, 100);
    let error = open
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            &open, 10, 0, true,
        ))
        .expect_err("an open era cannot quiesce");
    assert_eq!(error.rejection(), EraQuiescenceRejection::ClosingCohortJoin);

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
    assert_eq!(error.rejection(), EraQuiescenceRejection::ClosingCohortJoin);

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
    assert_eq!(error.rejection(), EraQuiescenceRejection::ClosingCohortJoin);
}

/// An honest retirement receipt naming the quiescent era.
fn honest_retirement_receipt() -> ComponentEraRetirementReceipt {
    let mut ledger = ledger(4);
    publish_supersede_quiesce(&mut ledger, 10, 20);
    ComponentEraRetirementReceipt::from_runtime(700, &ledger, 10, true)
}

/// An authentic retirement receipt minted under `foreign_ledger`'s
/// contracts.
fn foreign_retirement_receipt() -> ComponentEraRetirementReceipt {
    let mut foreign = foreign_ledger(4);
    foreign_publish_supersede_quiesce(&mut foreign, 10, 20);
    ComponentEraRetirementReceipt::from_runtime(700, &foreign, 10, true)
}

/// The retirement family's independent checker: replay `retire` on a ledger
/// where era 10 is quiescent under era 20. Rejection keeps the quiescent era
/// live.
fn replay_retirement(
    receipt: &ComponentEraRetirementReceipt,
) -> Result<ComponentEraRetirementReceipt, EraRetirementRejection> {
    let mut ledger = ledger(4);
    publish_supersede_quiesce(&mut ledger, 10, 20);
    match ledger.retire(receipt.clone()) {
        Ok(()) => Ok(receipt.clone()),
        Err(error) => {
            assert_eq!(
                ledger.live_eras().collect::<Vec<_>>(),
                vec![
                    (10, ComponentEraEntryState::Quiescent, 0),
                    (20, ComponentEraEntryState::Open, 0),
                ],
                "rejection keeps the quiescent era live"
            );
            Err(error.rejection())
        }
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable field of [`ComponentEraRetirementReceipt`]; checked
    /// by `retire`'s replay against the retained quiescent era record and
    /// the consumed-identity set. The replayed-identity, still-closing, and
    /// lease-held eras need staged ledger state and stay authored in the
    /// test.
    pub enum RetirementReceiptFieldForTest {
        // An era outside the live roster: the record lookup fails.
        EraIdentityForeign,
        // The live-but-current successor fails the noncurrent-quiescent
        // join.
        EraIdentityCurrent,
        // `retirement_identity = 0`: the fresh-nonce clause.
        RetirementIdentity,
        BindingContractIdentity,
        // `lifetime_cohort_released = false`: the cohort-release clause.
        CohortReleaseDropped,
        // An authentic receipt minted under a foreign ledger's contracts.
        ForeignReceipt,
    }
}

/// Substitute exactly one retirement receipt field.
fn substitute_retirement_receipt_for_test(
    receipt: &mut ComponentEraRetirementReceipt,
    field: RetirementReceiptFieldForTest,
    donor: &ComponentEraRetirementReceipt,
) {
    use RetirementReceiptFieldForTest as Field;
    match field {
        Field::EraIdentityForeign => receipt.era_identity = 99,
        Field::EraIdentityCurrent => receipt.era_identity = 20,
        Field::RetirementIdentity => receipt.retirement_identity = 0,
        Field::BindingContractIdentity => {
            receipt.binding_contract_identity = "ForeignBinding/v1".into()
        }
        Field::CohortReleaseDropped => receipt.lifetime_cohort_released = false,
        Field::ForeignReceipt => *receipt = donor.clone(),
    }
}

/// Only the out-of-roster era escapes the retirement join: it fails the
/// record lookup instead.
fn retirement_receipt_outcome(
    field: RetirementReceiptFieldForTest,
) -> MutationOutcome<EraRetirementRejection> {
    MutationOutcome::ExactError(match field {
        RetirementReceiptFieldForTest::EraIdentityForeign => EraRetirementRejection::EraNotLive,
        _ => EraRetirementRejection::QuiescentReleaseJoin,
    })
}

#[test]
fn component_era_retirement_receipt_rejects_every_one_field_substitution() {
    // The current era and the closing-but-not-quiescent era both fail the
    // noncurrent-quiescent join; the nonce and contract coordinates join the
    // consumed set and the ledger's identities.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "component era retirement receipt",
        fields: RetirementReceiptFieldForTest::INVENTORY,
        honest: &honest_retirement_receipt,
        donor: foreign_retirement_receipt(),
        custody: &Clone::clone,
        substitute: &substitute_retirement_receipt_for_test,
        check: &replay_retirement,
        outcome: &retirement_receipt_outcome,
        joined_replay: None,
    });

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
    assert_eq!(
        error.rejection(),
        EraRetirementRejection::QuiescentReleaseJoin
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
    assert_eq!(
        error.rejection(),
        EraRetirementRejection::QuiescentReleaseJoin
    );

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
    assert_eq!(error.rejection(), EraRetirementRejection::EpochLeasesLive);
}

/// The retained-custody view of an epoch lease: every binding the hold
/// carries. The lease itself is deliberately non-clonable, so the matrix
/// compares this projection.
#[derive(Debug, Clone, PartialEq)]
struct EpochLeaseCustody {
    identity: ProgramLocalRootEpochLeaseId,
    ledger: ComponentEraLedgerId,
    binding_contract_identity: String,
    entry_contract_identity: String,
    era_identity: u64,
    artifact_occurrence_digest: InstalledArtifactOccurrenceDigest,
    artifact_instance_compatibility_report_identity: u64,
    entry_plan_identity: String,
    entry_plan_admission_receipt_identity: String,
    candidate: ComponentEraCandidate,
}

impl EpochLeaseCustody {
    fn of(lease: &ProgramLocalRootEpochLease) -> Self {
        Self {
            identity: lease.identity,
            ledger: lease.ledger,
            binding_contract_identity: lease.binding_contract_identity.clone(),
            entry_contract_identity: lease.entry_contract_identity.clone(),
            era_identity: lease.era_identity,
            artifact_occurrence_digest: lease.artifact_occurrence_digest,
            artifact_instance_compatibility_report_identity: lease
                .artifact_instance_compatibility_report_identity,
            entry_plan_identity: lease.entry_plan_identity.clone(),
            entry_plan_admission_receipt_identity: lease
                .entry_plan_admission_receipt_identity
                .clone(),
            candidate: lease.candidate.clone(),
        }
    }
}

/// `release` consumes the hold, which is deliberately non-clonable: the
/// replay rebuilds the substituted record field-for-field.
fn replayed_lease(lease: &ProgramLocalRootEpochLease) -> ProgramLocalRootEpochLease {
    ProgramLocalRootEpochLease {
        identity: lease.identity,
        ledger: lease.ledger,
        binding_contract_identity: lease.binding_contract_identity.clone(),
        entry_contract_identity: lease.entry_contract_identity.clone(),
        era_identity: lease.era_identity,
        artifact_occurrence_digest: lease.artifact_occurrence_digest,
        artifact_instance_compatibility_report_identity: lease
            .artifact_instance_compatibility_report_identity,
        entry_plan_identity: lease.entry_plan_identity.clone(),
        entry_plan_admission_receipt_identity: lease.entry_plan_admission_receipt_identity.clone(),
        candidate: lease.candidate.clone(),
    }
}

/// An honest lease issued for the open era 10.
fn honest_epoch_lease() -> ProgramLocalRootEpochLease {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    ledger
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("honest lease")
}

/// An authentic lease issued under `foreign_ledger`'s contracts.
fn foreign_epoch_lease() -> ProgramLocalRootEpochLease {
    let mut foreign = foreign_ledger(4);
    foreign_publish(&mut foreign, 10, 700);
    foreign
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "ForeignEntry/v1")
        .expect("foreign lease")
}

/// The lease family's independent checker: replay `release` on a ledger
/// whose era 10 issued hold 900. Rejection keeps the era's authority hold.
fn replay_epoch_lease_release(
    lease: &ProgramLocalRootEpochLease,
) -> Result<EpochLeaseCustody, ProgramLocalRootEpochLeaseReleaseRejection> {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    let _issued = ledger
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("staging hold");
    match ledger.release_program_local_root_epoch_lease(replayed_lease(lease)) {
        Ok(()) => Ok(EpochLeaseCustody::of(lease)),
        Err(error) => {
            assert_eq!(
                ledger.program_local_root_authority_holds(10),
                Some(1),
                "rejection keeps the era's authority hold"
            );
            Err(error.rejection())
        }
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable binding of [`ProgramLocalRootEpochLease`]; checked
    /// by `release`'s replay of every carried coordinate against the
    /// retained era record and the issued-hold sets. The rebound-to-a-live-
    /// sibling era leg, the indistinguishable-release leg that honestly
    /// settles another hold's identity, and the stricter `validate` replay
    /// stay authored in the test.
    pub enum EpochLeaseFieldForTest {
        // An identity this ledger never issued.
        IdentityUnissued,
        // A different containing ledger.
        LedgerIdentity,
        BindingContractIdentity,
        EntryContractIdentity,
        ArtifactOccurrenceDigest,
        CompatibilityReportIdentity,
        EntryPlanIdentity,
        EntryPlanReceiptIdentity,
        // The retained candidate copy swapped wholesale.
        CandidateWholesale,
        // One inner axis of the retained candidate copy.
        CandidateAcceptance,
        // An era outside the live roster: the record lookup fails.
        EraIdentityForeign,
        // An authentic lease minted under a foreign ledger.
        ForeignLease,
    }
}

/// Substitute exactly one lease binding.
fn substitute_epoch_lease_for_test(
    lease: &mut ProgramLocalRootEpochLease,
    field: EpochLeaseFieldForTest,
    donor: &ProgramLocalRootEpochLease,
) {
    use EpochLeaseFieldForTest as Field;
    match field {
        Field::IdentityUnissued => lease.identity = lease_id(999),
        Field::LedgerIdentity => {
            lease.ledger =
                ComponentEraLedgerId::from_normalized_identity(2).expect("ledger identity")
        }
        Field::BindingContractIdentity => {
            lease.binding_contract_identity = "ForeignBinding/v1".into()
        }
        Field::EntryContractIdentity => lease.entry_contract_identity = "ForeignEntry/v1".into(),
        Field::ArtifactOccurrenceDigest => {
            lease.artifact_occurrence_digest =
                InstalledArtifactOccurrenceDigest::from_sha256([0xEE; 32])
        }
        Field::CompatibilityReportIdentity => {
            lease.artifact_instance_compatibility_report_identity = 4_444
        }
        Field::EntryPlanIdentity => lease.entry_plan_identity = "entry-plan:foreign".into(),
        Field::EntryPlanReceiptIdentity => {
            lease.entry_plan_admission_receipt_identity = "receipt:entry-plan:foreign".into()
        }
        Field::CandidateWholesale => lease.candidate = candidate(20),
        Field::CandidateAcceptance => {
            lease.candidate.executable_tcb_acceptance = acceptance("substituted-era", 999)
        }
        Field::EraIdentityForeign => lease.era_identity = 99,
        Field::ForeignLease => *lease = replayed_lease(donor),
    }
}

/// Only the out-of-roster era escapes the exact-release join: it fails the
/// record lookup instead.
fn epoch_lease_outcome(
    field: EpochLeaseFieldForTest,
) -> MutationOutcome<ProgramLocalRootEpochLeaseReleaseRejection> {
    MutationOutcome::ExactError(match field {
        EpochLeaseFieldForTest::EraIdentityForeign => {
            ProgramLocalRootEpochLeaseReleaseRejection::ForeignEra
        }
        _ => ProgramLocalRootEpochLeaseReleaseRejection::ExactPublishedEraJoin,
    })
}

#[test]
fn program_local_root_epoch_lease_rejects_every_one_field_substitution() {
    // Every binding the lease carries replays against the retained era
    // record and the ledger's own identities at release.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "program-local root epoch lease",
        fields: EpochLeaseFieldForTest::INVENTORY,
        honest: &honest_epoch_lease,
        donor: foreign_epoch_lease(),
        custody: &EpochLeaseCustody::of,
        substitute: &substitute_epoch_lease_for_test,
        check: &replay_epoch_lease_release,
        outcome: &epoch_lease_outcome,
        joined_replay: None,
    });

    // A lease rebound to another live era binds the wrong retained
    // candidate.
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
    assert_eq!(
        error.rejection(),
        ProgramLocalRootEpochLeaseReleaseRejection::ExactPublishedEraJoin
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
    assert_eq!(
        validating.validate_program_local_root_epoch_lease(&lease),
        Err(ProgramLocalRootEpochLeaseValidationRejection::NotLiveInCurrentOpenEra),
        "a substituted lease must not validate"
    );
    lease.artifact_instance_compatibility_report_identity = 1_010;
    publish(&mut validating, 20, 101);
    assert_eq!(
        validating.validate_program_local_root_epoch_lease(&lease),
        Err(ProgramLocalRootEpochLeaseValidationRejection::NotLiveInCurrentOpenEra),
        "a closing era cannot establish fresh authority"
    );
    validating
        .release_program_local_root_epoch_lease(lease)
        .expect("a closing era still releases its exact hold");
}

/// The request `acquire_program_local_root_epoch_lease` replays: the minted
/// lease identity, the era it names, and the entry contract it claims.
#[derive(Debug, Clone, PartialEq)]
struct EpochLeaseAcquisition {
    identity: ProgramLocalRootEpochLeaseId,
    era_identity: u64,
    entry_contract_identity: String,
}

/// An honest acquisition request against a ledger whose era 10 is superseded
/// by era 20 — staging that also covers the replayed-identity and stale-era
/// axes: hold 900 is consumed and the current era is 20.
fn honest_epoch_lease_acquisition() -> EpochLeaseAcquisition {
    EpochLeaseAcquisition {
        identity: lease_id(901),
        era_identity: 20,
        entry_contract_identity: "CodecEntry/v1".into(),
    }
}

/// An authentic request minted under `foreign_ledger`'s contracts.
fn foreign_epoch_lease_acquisition() -> EpochLeaseAcquisition {
    EpochLeaseAcquisition {
        identity: lease_id(901),
        era_identity: 20,
        entry_contract_identity: "ForeignEntry/v1".into(),
    }
}

/// The acquisition family's independent checker: replay `acquire` on the
/// superseded-era staging. Rejection grants no hold on the current era.
fn replay_epoch_lease_acquisition(
    request: &EpochLeaseAcquisition,
) -> Result<EpochLeaseAcquisition, ProgramLocalRootEpochLeaseAcquisitionRejection> {
    let mut ledger = ledger(4);
    publish(&mut ledger, 10, 100);
    let _hold = ledger
        .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
        .expect("staging hold");
    publish(&mut ledger, 20, 101);
    match ledger.acquire_program_local_root_epoch_lease(
        request.identity,
        request.era_identity,
        &request.entry_contract_identity,
    ) {
        Ok(_lease) => Ok(request.clone()),
        Err(error) => {
            assert_eq!(
                ledger.program_local_root_authority_holds(20),
                Some(0),
                "rejection grants no hold on the current era"
            );
            Err(error.rejection())
        }
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the epoch-lease acquisition request;
    /// checked by `acquire`'s replay order — consumed identity, current-era
    /// join, then the exact open contract.
    pub enum EpochLeaseAcquisitionFieldForTest {
        // An already-issued identity.
        IdentityReplayed,
        // An era this ledger never published.
        EraIdentityUnpublished,
        // A live-but-superseded era.
        EraIdentitySuperseded,
        EntryContractIdentityCleared,
        EntryContractIdentityForeign,
        // An authentic request minted under a foreign ledger's contracts.
        ForeignRequest,
    }
}

/// Substitute exactly one acquisition axis.
fn substitute_epoch_lease_acquisition_for_test(
    request: &mut EpochLeaseAcquisition,
    field: EpochLeaseAcquisitionFieldForTest,
    donor: &EpochLeaseAcquisition,
) {
    use EpochLeaseAcquisitionFieldForTest as Field;
    match field {
        Field::IdentityReplayed => request.identity = lease_id(900),
        Field::EraIdentityUnpublished => request.era_identity = 99,
        Field::EraIdentitySuperseded => request.era_identity = 10,
        Field::EntryContractIdentityCleared => request.entry_contract_identity.clear(),
        Field::EntryContractIdentityForeign => {
            request.entry_contract_identity = "ForeignEntry/v1".into()
        }
        Field::ForeignRequest => *request = donor.clone(),
    }
}

/// The request legs reject in replay order: the consumed identity first,
/// then the current-era join, then the contract join.
fn epoch_lease_acquisition_outcome(
    field: EpochLeaseAcquisitionFieldForTest,
) -> MutationOutcome<ProgramLocalRootEpochLeaseAcquisitionRejection> {
    use EpochLeaseAcquisitionFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::IdentityReplayed => ProgramLocalRootEpochLeaseAcquisitionRejection::IdentityConsumed,
        Field::EraIdentityUnpublished | Field::EraIdentitySuperseded => {
            ProgramLocalRootEpochLeaseAcquisitionRejection::NotCurrentEra
        }
        _ => ProgramLocalRootEpochLeaseAcquisitionRejection::OpenContractJoin,
    })
}

#[test]
fn program_local_root_epoch_lease_acquisition_rejects_every_one_field_substitution() {
    // Acquisition replays its own axes: consumed identities, non-current or
    // unpublished eras, and contract identities that diverge from either the
    // ledger or the retained candidate.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "program-local root epoch lease acquisition",
        fields: EpochLeaseAcquisitionFieldForTest::INVENTORY,
        honest: &honest_epoch_lease_acquisition,
        donor: foreign_epoch_lease_acquisition(),
        custody: &Clone::clone,
        substitute: &substitute_epoch_lease_acquisition_for_test,
        check: &replay_epoch_lease_acquisition,
        outcome: &epoch_lease_acquisition_outcome,
        joined_replay: None,
    });
}
