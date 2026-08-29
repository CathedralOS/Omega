use super::render::reason_token;
use super::*;

#[test]
fn disposition_order_keeps_blockers_above_recommendations() {
    assert!(
        PackageTriageDisposition::BlockedMissingAdmissionBaseline
            > PackageTriageDisposition::AdmittedWithAuditRecommended
    );
    assert!(
        PackageTriageDisposition::BlockedCapabilityChange
            > PackageTriageDisposition::BlockedMissingAdmissionBaseline
    );
    assert!(
        PackageTriageDisposition::BlockedProvenanceChange
            > PackageTriageDisposition::BlockedCapabilityChange
    );
}

#[test]
fn reason_tokens_are_fixed_and_source_text_free() {
    assert_eq!(
        reason_token(PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::Filesystem,
        )),
        "retained_dangerous_authority_filesystem"
    );
    assert_eq!(
        reason_token(PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::Process,
        )),
        "retained_dangerous_authority_process"
    );
    assert_eq!(
        reason_token(PackageTriageReason::DangerousAuthoritySlack(
            PackageReviewDangerousAuthorityClass::PortIo,
        )),
        "dangerous_authority_slack_port_io"
    );
    assert_eq!(
        reason_token(PackageTriageReason::MissingAdmissionBaseline),
        "missing_admission_baseline"
    );
    assert_eq!(
        reason_token(PackageTriageReason::AcceptedClaimRequiresResolution),
        "accepted_claim_requires_resolution"
    );
    assert_eq!(
        reason_token(PackageTriageReason::RepresentationTcbIntroducedOrChanged),
        "representation_tcb_introduced_or_changed"
    );
}

#[test]
fn bounded_render_rejects_instead_of_truncating_evidence() {
    let triage = CompilerReviewTriage {
        decisions: vec![PackageTriageDecision {
            package_name: "arithmetic-kernels".to_owned(),
            baseline_key: None,
            candidate_key: None,
            disposition: PackageTriageDisposition::Admitted,
            reasons: vec![PackageTriageReason::InitialAdmission],
        }],
    };
    let full = triage.render_bounded(1_024).unwrap();
    assert!(full.contains("package arithmetic-kernels\n"));
    let error = triage.render_bounded(32).unwrap_err();
    assert_eq!(error.maximum_bytes(), 32);
    assert!(error.required_bytes() > 32);
}
