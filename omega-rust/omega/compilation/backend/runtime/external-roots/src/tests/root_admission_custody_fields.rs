//! Substitutable-field inventory for the installed-root admission custody
//! matrix driven by `root_admission_custody.rs`'s
//! `installed_root_admission_rejects_every_one_field_substitution`.
//!
//! The declared lanes are every `RootAdmission` field bound to retained
//! evidence at install time: the copied root/code/slot/owner/trust
//! coordinates, the retained provider-execution evidence's own internals,
//! and the reportable copies of its plan and exit assurance. Keeping the
//! inventory in its own file names the boundary between what is declared
//! substitutable and the shared `run_one_field_substitution_matrix` driver
//! that exercises every leg.
//!
//! What stays authored beside the matrix is not a one-field substitution:
//! honestly recomputed multi-field substitutions (the second test), the
//! admission's own minted identity — the one axis adopted verbatim, which
//! installs and moves the ledger's containing fingerprint — construction
//! refusal at `from_admitted_provider`, and ledger-state substitutions
//! (duplicate root, occupied slot).

use super::{
    admission_fixture, entry_id, foreign_root, installed_code,
    installed_code_with_fill_and_installation_identity, provider_execution, root_id, slot,
};
use crate::{
    OpaqueProviderExitAssurance, ProviderExecutionId, ProviderPlanId, RootAdmission,
    RootAdmissionId, RootSlotId, RootSlotOwnerId, TrustReceiptId,
};

optimization_core::custody_field_inventory! {
    /// One substitutable lane of a provider-minted `RootAdmission`; checked
    /// by `InstalledRootLedger::install`, which replays every field against
    /// the retained validated root, installed-code occurrence, slot
    /// authority, and execution evidence the record claims.
    pub enum RootAdmissionFieldForTest {
        // --- the binding join: copied coordinates equal the retained
        //     root, code occurrence, slot, owner, and trust receipts ---
        RootEvidence,
        ExecutionEvidence,
        ExecutionEvidenceIdentity,
        ExecutionEvidenceRoot,
        ExecutionEvidenceReportIdentity,
        RootReportIdentity,
        ExecutionIdentity,
        ExecutionReportFingerprint,
        InstalledCodeOccurrence,
        InstalledCodeContext,
        Artifact,
        Slot,
        Owner,
        TrustReceiptDropped,
        TrustReceiptForged,
        // --- the retained evidence's internals replay against the root ---
        EvidenceProviderPlan,
        // --- the reportable copies equal the retained evidence's values ---
        ProviderPlan,
        ExitAssurance,
        ExitAssuranceReportFingerprint,
    }
}

/// The named rejection `InstalledRootLedger::install` reports for a
/// substituted admission: the admission-binding join, the retained
/// evidence's replay against the validated root, or the reportable-copy
/// join. The diagnostic messages are fixed join names, so the kinds
/// classify exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootAdmissionCheck {
    /// A copied coordinate does not equal the retained root, code, slot,
    /// owner, or trust receipts it claims.
    RootBinding,
    /// The retained provider execution does not replay the exact validated
    /// root, its honestly recomputed report identity, or its exit
    /// assurance.
    EvidenceReplay,
    /// The record's reportable copies do not equal the retained execution
    /// evidence's provider plan, exit assurance, or its fingerprint.
    EvidenceCopy,
}

/// Classify an install rejection into the family's named vocabulary; any
/// other diagnostic means the substitution produced a verdict shape the
/// family does not declare.
pub fn root_admission_check_kind(diagnostic: &crate::ExternalRootDiagnostic) -> RootAdmissionCheck {
    use RootAdmissionCheck as Check;
    let message = diagnostic.0.as_str();
    if message.contains("does not bind the exact root, code, slot, owner, and trust receipts") {
        Check::RootBinding
    } else if message.contains(
        "does not replay the exact validated root, its report identity, or its exit assurance",
    ) {
        Check::EvidenceReplay
    } else if message.contains(
        "does not carry the admitted provider execution's exact provider plan and exit assurance",
    ) {
        Check::EvidenceCopy
    } else {
        panic!("install rejection outside the declared vocabulary: {message}")
    }
}

/// Rebuild a `RootAdmission` field-for-field: the record is deliberately
/// not `Clone` (construction is provider admission), so the checker and the
/// custody view take the same explicit snapshot.
pub fn admission_snapshot(record: &RootAdmission) -> RootAdmission {
    RootAdmission {
        identity: record.identity,
        root_evidence: record.root_evidence.clone(),
        provider_execution_evidence: record.provider_execution_evidence.clone(),
        root_report_identity: record.root_report_identity,
        provider_execution: record.provider_execution,
        provider_execution_report_fingerprint: record.provider_execution_report_fingerprint,
        provider_exit_assurance: record.provider_exit_assurance,
        provider_exit_assurance_report_fingerprint: record
            .provider_exit_assurance_report_fingerprint,
        provider_plan: record.provider_plan,
        installed_code: record.installed_code,
        installed_code_context: record.installed_code_context.clone(),
        artifact: record.artifact,
        slot: record.slot,
        owner: record.owner,
        trust_receipts: record.trust_receipts.clone(),
    }
}

/// An authentic foreign admission: minted against a distinct validated root
/// over the same installed code and entry, so its root-bound evidence and
/// execution supply real donor values for the nested-custody legs.
pub fn foreign_admission_donor() -> RootAdmission {
    let fixture = admission_fixture();
    let foreign = foreign_root(&fixture);
    let foreign_execution = provider_execution(&foreign);
    RootAdmission::from_admitted_provider(
        root_id(96, RootAdmissionId::from_normalized_identity),
        &foreign,
        &foreign_execution,
        &fixture.code,
        &slot(),
        foreign.candidate().trust_receipts.iter().copied(),
    )
    .expect("the foreign admission is authentic")
}

/// The family's honest-recomputation hook: mutate exactly the declared
/// field of `admission`. `donor` is the authentic foreign admission — its
/// retained validated root and provider execution are the foreign evidence
/// the nested-custody legs borrow; the remaining legs take fixed
/// non-identity alternates. There is no containing identity to recompute:
/// the admission's own minted identity is sovereign state adopted verbatim,
/// covered by its own authored test beside the matrix.
pub fn substitute_root_admission_for_test(
    admission: &mut RootAdmission,
    field: RootAdmissionFieldForTest,
    donor: &RootAdmission,
) {
    use RootAdmissionFieldForTest as Leg;
    match field {
        Leg::RootEvidence => admission.root_evidence = donor.root_evidence.clone(),
        Leg::ExecutionEvidence => {
            admission.provider_execution_evidence = donor.provider_execution_evidence.clone();
        }
        Leg::ExecutionEvidenceIdentity => {
            admission.provider_execution_evidence.identity =
                root_id(77, ProviderExecutionId::from_normalized_identity);
        }
        Leg::ExecutionEvidenceRoot => {
            admission.provider_execution_evidence.root_evidence = donor.root_evidence.clone();
        }
        Leg::ExecutionEvidenceReportIdentity => {
            admission
                .provider_execution_evidence
                .normalized_report_identity += 1;
        }
        Leg::RootReportIdentity => admission.root_report_identity += 1,
        Leg::ExecutionIdentity => {
            admission.provider_execution =
                root_id(78, ProviderExecutionId::from_normalized_identity);
        }
        Leg::ExecutionReportFingerprint => {
            admission.provider_execution_report_fingerprint += 1;
        }
        // A distinct installation occurrence carries a distinct code
        // identity and receipt context; a distinct artifact digest carries
        // only a foreign artifact identity under the same occurrence.
        Leg::InstalledCodeOccurrence => {
            admission.installed_code =
                installed_code_with_fill_and_installation_identity(1, entry_id(1001), 0, 400)
                    .identity();
        }
        Leg::InstalledCodeContext => {
            admission.installed_code_context =
                installed_code_with_fill_and_installation_identity(1, entry_id(1001), 0, 400)
                    .receipt_context();
        }
        Leg::Artifact => {
            admission.artifact = installed_code(9, entry_id(1001)).artifact();
        }
        Leg::Slot => {
            admission.slot = root_id(88, RootSlotId::from_normalized_identity);
        }
        Leg::Owner => {
            admission.owner = root_id(89, RootSlotOwnerId::from_normalized_identity);
        }
        Leg::TrustReceiptDropped => {
            admission
                .trust_receipts
                .pop_first()
                .expect("non-empty trust receipts");
        }
        Leg::TrustReceiptForged => {
            admission
                .trust_receipts
                .insert(root_id(90, TrustReceiptId::from_normalized_identity));
        }
        Leg::EvidenceProviderPlan => {
            admission.provider_execution_evidence.provider_plan =
                root_id(91, ProviderPlanId::from_normalized_identity);
        }
        Leg::ProviderPlan => {
            admission.provider_plan = root_id(92, ProviderPlanId::from_normalized_identity);
        }
        Leg::ExitAssurance => {
            admission.provider_exit_assurance = OpaqueProviderExitAssurance::HardwareIsolation {
                validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
            };
        }
        Leg::ExitAssuranceReportFingerprint => {
            admission.provider_exit_assurance_report_fingerprint += 1;
        }
    }
}

/// The exact checker verdict each declared leg must produce: every lane is
/// refused by one of the ledger's three named joins.
pub fn root_admission_custody_outcome(
    field: RootAdmissionFieldForTest,
) -> optimization_core::MutationOutcome<RootAdmissionCheck> {
    use RootAdmissionCheck as Check;
    use RootAdmissionFieldForTest as Leg;
    let check = match field {
        Leg::RootEvidence
        | Leg::ExecutionEvidence
        | Leg::ExecutionEvidenceIdentity
        | Leg::ExecutionEvidenceRoot
        | Leg::ExecutionEvidenceReportIdentity
        | Leg::RootReportIdentity
        | Leg::ExecutionIdentity
        | Leg::ExecutionReportFingerprint
        | Leg::InstalledCodeOccurrence
        | Leg::InstalledCodeContext
        | Leg::Artifact
        | Leg::Slot
        | Leg::Owner
        | Leg::TrustReceiptDropped
        | Leg::TrustReceiptForged => Check::RootBinding,
        Leg::EvidenceProviderPlan => Check::EvidenceReplay,
        Leg::ProviderPlan | Leg::ExitAssurance | Leg::ExitAssuranceReportFingerprint => {
            Check::EvidenceCopy
        }
    };
    optimization_core::MutationOutcome::ExactError(check)
}
