use super::render::reason_token;
use super::*;
use crate::declarations::BuildDeclarationKind;
use crate::graph::{ResolvedPackageClosure, ResolvedPackageNode, ResolvedSourceIdentity};
use crate::identity::{PackageKey, PackageName};
use crate::review::compare::compare_review_only_root_role_graphs;
use omega_package_source::{GitCommitId, GitTreeId, ImmutableSourceResolution, SourceLineage};

fn role_test_key() -> PackageKey {
    PackageKey::new(
        PackageName::parse("role-probe").unwrap(),
        SourceLineage::git("https://github.com/CathedralOS/role-probe.git").unwrap(),
    )
}

fn role_test_graph(role: BuildDeclarationKind) -> ResolvedPackageClosure {
    let key = role_test_key();
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&"11".repeat(20)).unwrap(),
        GitTreeId::parse_hex(&"22".repeat(20)).unwrap(),
    )
    .unwrap();
    ResolvedPackageClosure::new(
        key.clone(),
        role,
        vec![ResolvedPackageNode::new(
            ResolvedSourceIdentity::new(key, resolution).unwrap(),
            vec![],
        )],
    )
    .unwrap()
}

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
    assert_eq!(
        reason_token(PackageTriageReason::RootLostDependencyCompatibility),
        "root_lost_dependency_compatibility"
    );
    assert_eq!(
        reason_token(PackageTriageReason::RootLostApplicationActivation),
        "root_lost_application_activation"
    );
}

#[test]
fn directional_root_role_change_blocks_the_exact_root_triage_decision() {
    let package = role_test_graph(BuildDeclarationKind::Package);
    let application = role_test_graph(BuildDeclarationKind::Application);
    let change = compare_review_only_root_role_graphs(&package, &application)
        .unwrap()
        .unwrap();
    let key = role_test_key();
    let mut triage = CompilerReviewTriage {
        decisions: vec![PackageTriageDecision {
            package_name: "role-probe".to_owned(),
            baseline_key: Some(key.clone()),
            candidate_key: Some(key),
            disposition: PackageTriageDisposition::Admitted,
            reasons: vec![],
        }],
    };

    apply_root_role_change(&mut triage, &change);

    assert_eq!(
        triage.disposition(),
        PackageTriageDisposition::BlockedCapabilityChange
    );
    assert_eq!(
        triage.decisions()[0].reasons(),
        &[PackageTriageReason::RootLostDependencyCompatibility]
    );
    let rendered = triage.render_bounded(1_024).unwrap();
    assert!(rendered.starts_with("OMEGA_PACKAGE_SOURCE_TRIAGE_V2\n"));
    assert!(rendered.contains("reason root_lost_dependency_compatibility\n"));
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
