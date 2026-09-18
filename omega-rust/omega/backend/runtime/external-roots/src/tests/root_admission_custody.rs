//! One-field substitution custody over `RootAdmission` at the installed-root
//! ledger boundary.
//!
//! The admission is the provider-minted commitment that publishes one
//! validated root, its provider execution evidence, the installed-code
//! occurrence, and the owner-controlled slot into a single reportable record.
//! `InstalledRootLedger::install` is the independent replay: every admission
//! field is bound either to the retained arguments (root, code, slot
//! authority) or to the retained execution evidence, and the evidence itself
//! is replayed against the exact validated root — its root-bound
//! coordinates, its honestly recomputed compact report identity, and its
//! opaque exit assurance's own validation all run again at admission time.
//! The only adopted-verbatim field is the admission's own minted identity,
//! which nothing outside the provider could contradict; the ledger still
//! covers it under the installed-set fingerprint honestly.

use super::boundary_fixtures::candidate_for_code_with_root;
use super::{
    boundary, candidate_for_code, entry_id, installed_code,
    installed_code_with_fill_and_installation_identity, provider_execution, root_id, slot,
};
use crate::{
    InstalledRootLedger, OpaqueProviderExitAssurance, ProviderExecution, ProviderExecutionId,
    ProviderPlanId, RootAdmission, RootAdmissionId, RootInstallError, RootSlotAuthority,
    RootSlotId, RootSlotOwnerId, TrustReceiptId, ValidatedExternalRoot, validate_external_root,
};
use executable_installation::InstalledCode;

/// The admission-binding join: copied coordinates that must equal the
/// retained root, installed-code occurrence, slot, owner, and trust set.
const ROOT_BINDING: &str = "does not bind the exact root, code, slot, owner, and trust receipts";
/// The retained-evidence replay join: the provider execution must still bind
/// the exact validated root, and its derived report identity recomputes and
/// its exit assurance revalidates against it.
const EVIDENCE_REPLAY: &str =
    "does not replay the exact validated root, its report identity, or its exit assurance";
/// The reportable-copy join: the record's provider plan and exit assurance
/// must equal the retained execution evidence's values.
const EVIDENCE_COPY: &str =
    "does not carry the admitted provider execution's exact provider plan and exit assurance";

/// One coherent install scenario: installed code, the validated root over it,
/// and the provider execution minted for that root.
struct AdmissionFixture {
    code: InstalledCode,
    validated: ValidatedExternalRoot,
    execution: ProviderExecution,
}

fn admission_fixture() -> AdmissionFixture {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let validated =
        validate_external_root(candidate_for_code(entry, &code), &boundary()).expect("root plan");
    let execution = provider_execution(&validated);
    AdmissionFixture {
        code,
        validated,
        execution,
    }
}

/// A second validated root over the same installed code and entry: every
/// coordinate that must bind the root changes while the code and entry stays
/// exact.
fn foreign_root(fixture: &AdmissionFixture) -> ValidatedExternalRoot {
    validate_external_root(
        candidate_for_code_with_root(entry_id(1001), &fixture.code, 2),
        &boundary(),
    )
    .expect("foreign root plan")
}

fn honest_admission(fixture: &AdmissionFixture) -> (RootSlotAuthority, RootAdmission) {
    let authority = slot();
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &fixture.validated,
        &fixture.execution,
        &fixture.code,
        &authority,
        fixture.validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("honest root admission");
    (authority, admission)
}

/// Apply one mutation to an honest admission and replay it against a fresh
/// ledger. The returned fingerprint is the ledger's sealed identity before
/// the attempt, which a rejection must leave untouched.
fn rejected_install(
    mutate: impl FnOnce(&mut RootAdmission, &AdmissionFixture),
) -> (u64, Box<RootInstallError>) {
    let mut fixture = admission_fixture();
    let (authority, mut admission) = honest_admission(&fixture);
    mutate(&mut admission, &fixture);
    let mut ledger = InstalledRootLedger::claim(&mut fixture.code).expect("canonical root ledger");
    let baseline = ledger.report_fingerprint();
    let error = ledger
        .install(
            &fixture.code,
            fixture.validated.clone(),
            authority,
            admission,
        )
        .expect_err("a substituted admission must not install");
    assert_eq!(
        ledger.report_fingerprint(),
        baseline,
        "a rejected install leaves the ledger's sealed identity untouched"
    );
    assert!(ledger.live_external_roots_are_empty());
    (baseline, error)
}

fn reject_with(fragment: &str, mutate: impl FnOnce(&mut RootAdmission, &AdmissionFixture)) {
    let (_, error) = rejected_install(mutate);
    assert!(
        error.diagnostic().0.contains(fragment),
        "expected `{fragment}` among `{}`",
        error.diagnostic().0
    );
}

#[test]
fn installed_root_admission_rejects_every_one_field_substitution() {
    // Every field the admission records beside the retained evidence is
    // substituted independently; the ledger replays each against the exact
    // validated root, installed-code occurrence, slot, and owner it claims.
    reject_with(ROOT_BINDING, |admission, fixture| {
        admission.root_evidence = foreign_root(fixture);
    });
    reject_with(ROOT_BINDING, |admission, fixture| {
        admission.provider_execution_evidence = provider_execution(&foreign_root(fixture));
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission.provider_execution_evidence.identity =
            root_id(77, ProviderExecutionId::from_normalized_identity);
    });
    reject_with(ROOT_BINDING, |admission, fixture| {
        admission.provider_execution_evidence.root_evidence = foreign_root(fixture);
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission
            .provider_execution_evidence
            .normalized_report_identity += 1;
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission.root_report_identity += 1;
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission.provider_execution = root_id(78, ProviderExecutionId::from_normalized_identity);
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission.provider_execution_report_fingerprint += 1;
    });
    // A distinct installation occurrence carries a distinct code identity
    // and receipt context; a distinct artifact digest carries only a foreign
    // artifact identity under the same occurrence.
    reject_with(ROOT_BINDING, |admission, _| {
        admission.installed_code =
            installed_code_with_fill_and_installation_identity(1, entry_id(1001), 0, 400)
                .identity();
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission.installed_code_context =
            installed_code_with_fill_and_installation_identity(1, entry_id(1001), 0, 400)
                .receipt_context();
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission.artifact = installed_code(9, entry_id(1001)).artifact();
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission.slot = root_id(88, RootSlotId::from_normalized_identity);
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission.owner = root_id(89, RootSlotOwnerId::from_normalized_identity);
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission
            .trust_receipts
            .pop_first()
            .expect("non-empty trust receipts");
    });
    reject_with(ROOT_BINDING, |admission, _| {
        admission
            .trust_receipts
            .insert(root_id(90, TrustReceiptId::from_normalized_identity));
    });

    // The retained execution evidence's own internals replay against the
    // validated root: the whole `matches_root` coordinate set, the honestly
    // recomputed compact report identity, and the exit assurance's
    // construction-time validation all run again at install.
    reject_with(EVIDENCE_REPLAY, |admission, _| {
        admission.provider_execution_evidence.provider_plan =
            root_id(91, ProviderPlanId::from_normalized_identity);
    });

    // The record's reportable copies of the execution's plan and exit
    // assurance must equal the retained evidence's values.
    reject_with(EVIDENCE_COPY, |admission, _| {
        admission.provider_plan = root_id(92, ProviderPlanId::from_normalized_identity);
    });
    reject_with(EVIDENCE_COPY, |admission, _| {
        admission.provider_exit_assurance = OpaqueProviderExitAssurance::HardwareIsolation {
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        };
    });
    reject_with(EVIDENCE_COPY, |admission, _| {
        admission.provider_exit_assurance_report_fingerprint += 1;
    });
}

#[test]
fn installed_root_admission_rejects_honestly_recomputed_substitutions() {
    // Substituting the execution identity on both sides of the pairwise join
    // keeps the copied coordinates consistent, but the honestly recomputed
    // report identity inside the evidence still exposes the mutation.
    reject_with(EVIDENCE_REPLAY, |admission, _| {
        let substituted = root_id(93, ProviderExecutionId::from_normalized_identity);
        admission.provider_execution_evidence.identity = substituted;
        admission.provider_execution = substituted;
    });
    // A consistent substitution of the evidence's provider plan on both the
    // evidence and its reportable copy still rejects: the evidence must bind
    // the validated root's own selected plan, not a self-consistent claim.
    reject_with(EVIDENCE_REPLAY, |admission, _| {
        let substituted = root_id(94, ProviderPlanId::from_normalized_identity);
        admission.provider_execution_evidence.provider_plan = substituted;
        admission.provider_plan = substituted;
    });
    // Swapping the complete retained execution, its identity copy, and its
    // report-fingerprint copy for one consistent foreign triple still cannot
    // install: the foreign evidence does not bind this validated root.
    reject_with(ROOT_BINDING, |admission, fixture| {
        let foreign = provider_execution(&foreign_root(fixture));
        admission.provider_execution = foreign.identity();
        admission.provider_execution_report_fingerprint = foreign.normalized_report_identity();
        admission.provider_plan = foreign.provider_plan();
        admission.provider_exit_assurance = foreign.exit_assurance();
        admission.provider_exit_assurance_report_fingerprint =
            foreign.exit_assurance_report_fingerprint();
        admission.provider_execution_evidence = foreign;
    });
}

#[test]
fn installed_root_admission_adopts_its_own_identity_verbatim() {
    // The admission's own identity is provider-minted sovereign state: no
    // retained evidence contradicts it, so a substitution installs, lands in
    // the record and retained evidence, and moves the ledger's containing
    // fingerprint honestly.
    let mut fixture = admission_fixture();
    let (authority, admission) = honest_admission(&fixture);
    let mut ledger = InstalledRootLedger::claim(&mut fixture.code).expect("canonical root ledger");
    ledger
        .install(
            &fixture.code,
            fixture.validated.clone(),
            authority,
            admission,
        )
        .expect("honest admission installs");
    let honest_fingerprint = ledger.report_fingerprint();
    let root_identity = fixture.validated.candidate().identity;
    let honest_admission_identity = ledger.record(root_identity).expect("record").admission;

    let mut fixture = admission_fixture();
    let (authority, mut admission) = honest_admission(&fixture);
    let substituted = root_id(95, RootAdmissionId::from_normalized_identity);
    assert_ne!(substituted, honest_admission_identity);
    admission.identity = substituted;
    let mut ledger = InstalledRootLedger::claim(&mut fixture.code).expect("canonical root ledger");
    let installed = ledger
        .install(
            &fixture.code,
            fixture.validated.clone(),
            authority,
            admission,
        )
        .expect("the admission's own minted identity is adopted verbatim");
    assert_eq!(
        ledger.record(installed.root()).expect("record").admission,
        substituted
    );
    assert_ne!(
        ledger.report_fingerprint(),
        honest_fingerprint,
        "the adopted identity still participates in the ledger's containing fingerprint"
    );
}

#[test]
fn installed_root_admission_construction_rejects_unmatched_execution() {
    // The encoding leg of this family is the admission's own construction:
    // provider admission refuses to mint a commitment whose execution
    // evidence does not bind the exact validated root.
    let fixture = admission_fixture();
    let foreign = provider_execution(&foreign_root(&fixture));
    let authority = slot();
    let diagnostic = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &fixture.validated,
        &foreign,
        &fixture.code,
        &authority,
        fixture.validated.candidate().trust_receipts.iter().copied(),
    )
    .expect_err("an execution minted for another root cannot enter an admission");
    assert!(
        diagnostic
            .0
            .contains("does not bind the exact validated root realization")
    );
}

#[test]
fn installed_root_admission_rejects_ledger_state_substitutions() {
    // The roster axes of the family: a duplicate root identity and an
    // occupied slot are non-canonical admissions rejected before any record
    // is published.
    let mut fixture = admission_fixture();
    let (authority, admission) = honest_admission(&fixture);
    let mut ledger = InstalledRootLedger::claim(&mut fixture.code).expect("canonical root ledger");
    ledger
        .install(
            &fixture.code,
            fixture.validated.clone(),
            authority,
            admission,
        )
        .expect("honest admission installs");
    let occupied = ledger.report_fingerprint();

    let (authority, duplicate) = honest_admission(&fixture);
    let error = ledger
        .install(
            &fixture.code,
            fixture.validated.clone(),
            authority,
            duplicate,
        )
        .expect_err("the same root identity cannot install twice");
    assert!(error.diagnostic().0.contains("already installed"));
    let (returned_root, returned_slot, returned_admission) = error.into_parts();
    assert_eq!(returned_root, fixture.validated);
    assert_eq!(returned_slot.slot(), slot().slot());
    assert_eq!(
        returned_admission.provider_plan,
        fixture.execution.provider_plan()
    );
    assert_eq!(ledger.report_fingerprint(), occupied);

    let foreign_validated = foreign_root(&fixture);
    let foreign_execution = provider_execution(&foreign_validated);
    let authority = slot();
    let occupied_slot_admission = RootAdmission::from_admitted_provider(
        root_id(96, RootAdmissionId::from_normalized_identity),
        &foreign_validated,
        &foreign_execution,
        &fixture.code,
        &authority,
        foreign_validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("foreign root admission");
    let error = ledger
        .install(
            &fixture.code,
            foreign_validated,
            authority,
            occupied_slot_admission,
        )
        .expect_err("a distinct root cannot occupy a live slot");
    assert!(error.diagnostic().0.contains("already occupied"));
    assert_eq!(ledger.report_fingerprint(), occupied);
}

#[test]
fn installed_root_admission_rejection_hands_every_consumed_authority_back() {
    // A rejected substitution must not consume the validated root, the slot
    // authority, or the admission: the error returns all three intact so the
    // honest commitment can still be presented.
    let mut fixture = admission_fixture();
    let (authority, mut admission) = honest_admission(&fixture);
    admission.slot = root_id(97, RootSlotId::from_normalized_identity);
    let mut ledger = InstalledRootLedger::claim(&mut fixture.code).expect("canonical root ledger");
    let baseline = ledger.report_fingerprint();
    let error = ledger
        .install(
            &fixture.code,
            fixture.validated.clone(),
            authority,
            admission,
        )
        .expect_err("substituted slot coordinate rejects");
    let (returned_root, returned_slot, returned_admission) = error.into_parts();
    assert_eq!(returned_root, fixture.validated);
    assert_eq!(returned_slot.slot(), slot().slot());
    assert_eq!(returned_slot.owner(), slot().owner());
    assert_eq!(
        returned_admission.slot,
        root_id(97, RootSlotId::from_normalized_identity)
    );
    assert_eq!(ledger.report_fingerprint(), baseline);

    let (authority, honest) = honest_admission(&fixture);
    let installed = ledger
        .install(&fixture.code, returned_root, authority, honest)
        .expect("the retained authority still admits the honest commitment");
    assert!(ledger.record(installed.root()).is_some());
    assert_ne!(ledger.report_fingerprint(), baseline);
}
