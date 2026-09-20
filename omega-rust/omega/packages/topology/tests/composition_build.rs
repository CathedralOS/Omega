//! Acceptance for the `tests/fixtures/packages/build-scope-topology`
//! project: the package composes the three admitted payment components over
//! `builder.source`-confined input bytes and publishes `payments.plan`, then
//! a consumer with no project source verifies the emitted bytes against a
//! separately supplied `TopologyRequest`.

mod support;

use package_manager::admission::{
    AcceptedNativeInput, AcceptedNativeRealizationRequest, accept_ordinary_closure_evidence,
    realize_accepted_native_report,
};
use package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockTarget,
};
use package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ExactTargetPackageSourceClosure, PackageSourceClosureLimits, resolve_workspace_project_closure,
};
use package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, CompilerIssuedPackageReviewSet,
    PackagePolicyChangeLimits, PackagePolicyDecision, PackagePolicyDecisionSubject,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDisposition,
    compare_package_policy_changes, compile_resolved_package_candidate_for_production,
    resolve_package_policy_decisions,
};
use package_manager::{declarations::DependencyPurpose, review::SemanticBindingReview};
use package_source::{
    LocalSourceLimits, PrimaryGitChoices, SourceLineage, SourceRelativePath, SourceResolverStorage,
};
use std::path::Path;
use support::*;
use topology_plan::*;

/// Explicit project acceptance through the ordinary policy workflow (mirrors
/// the package-manager support fixture).
fn accepted_policy(
    target: &ExactTargetPackageSourceClosure<'_>,
    reviews: &CompilerIssuedPackageReviewSet,
) -> PackageLockTarget {
    let changes =
        compare_package_policy_changes(None, reviews, target, PackagePolicyChangeLimits::default())
            .expect("compare explicit initial project acceptance");
    let decisions = changes
        .packages()
        .iter()
        .flat_map(|package| package.rows())
        .filter(|row| row.requires_decision())
        .map(|row| PackagePolicyDecision {
            subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
            disposition: ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        })
        .collect::<Vec<_>>();
    assert!(changes.root_role_change().is_none());
    assert!(changes.source_replacements().is_empty());
    let resolution =
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &decisions)
            .expect("resolve explicit project choices");
    assert!(resolution.all_required_changes_accepted());
    let source = CanonicalSourceClosureSubject::from_resolved(
        target,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("capture accepted source subject");
    let mut policies = Vec::new();
    for package in source.packages() {
        for purpose in DependencyPurpose::ALL {
            if let Some(review) = reviews.review_occurrence(package.key(), purpose) {
                policies.push((review.checked_context(), review.policy()));
            }
        }
    }
    let history = HistoricalPackagePolicyDecisions::capture_policy(
        &source,
        &changes,
        &resolution,
        HistoricalPackagePolicyLimits::default(),
    )
    .expect("retain explicit project choices");
    PackageLockTarget::from_policies(source, &policies, history).expect("accepted project policy")
}

fn copy_fixture(workspace: &Path) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .join("tests/fixtures/packages/build-scope-topology")
        .canonicalize()
        .expect("committed fixture exists");
    for entry in ["root/build.omg", "root/main.omg", "topology/build.omg"] {
        let data = std::fs::read(fixture.join(entry)).unwrap();
        let target = workspace.join(entry);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, data).unwrap();
    }
    for entry in [
        "topology/compose.omg",
        "topology/digest.omg",
        "topology/policies.omg",
    ] {
        let data = std::fs::read(fixture.join(entry)).unwrap();
        let target = workspace.join(entry);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, data).unwrap();
    }
    // Inputs ride the root package's own source tree; the test regenerates
    // them from real admissions rather than trusting the committed bytes.
    let (request, components, bindings) = build_scope_inputs();
    for (name, bytes) in [
        ("request.bin", request.as_slice()),
        ("components.bin", components.as_slice()),
        ("bindings.bin", bindings.as_slice()),
    ] {
        let target = workspace.join("root/inputs").join(name);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, bytes).unwrap();
        // The committed fixture inputs must stay in lockstep with the
        // admission pipeline; drift means the fixture was hand-edited.
        assert_eq!(
            std::fs::read(fixture.join("root/inputs").join(name)).unwrap(),
            bytes,
            "committed fixture input {name} drifted from build_scope_inputs()"
        );
    }
}

#[test]
fn the_package_composes_and_a_source_free_consumer_verifies() {
    let workspace = std::env::temp_dir().join(format!(
        "omega-topology-build-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&workspace).unwrap();
    copy_fixture(&workspace);

    let storage = SourceResolverStorage::for_hardened_base(
        workspace.join("cache"),
        PrimaryGitChoices::default(),
    )
    .unwrap();
    let closure = resolve_workspace_project_closure(
        &SourceLineage::git("https://example.com/topology-build-fixture.git").unwrap(),
        SourceRelativePath::parse("root").unwrap(),
        workspace.clone(),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("fixture closure resolves");

    let target = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let candidate = compile_resolved_package_candidate_for_production(
        &target,
        &workspace.join("review"),
        SemanticBindingReview::Explicit(&[]),
        None,
    )
    .expect("composition project compiles for production");

    let policy = accepted_policy(&target, candidate.reviews());
    let evidence = accept_ordinary_closure_evidence(
        &target,
        candidate.reviews(),
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&policy),
    )
    .unwrap();
    let report = realize_accepted_native_report(
        AcceptedNativeInput::Reviewed {
            candidate: Box::new(candidate),
            optimization_rollback: &compiler::OptimizationRollback::default(),
        },
        AcceptedNativeRealizationRequest {
            evidence: &evidence,
            profile: &proof_admission::AdmissionProfile::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            receiving_terminal_authority_permission_policy: None,
            imports: &[],
        },
    )
    .expect("ordinary accepted artifact-only production");
    let publication = workspace.join("published");
    let report = report
        .publish_completed_build_outputs(&publication)
        .expect("build outputs publish");
    let outputs = report.build_outputs().unwrap();
    let directory = outputs.published_directory().unwrap();
    let plan_bytes =
        std::fs::read(directory.join("files/payments.plan")).expect("payments.plan was published");

    // Source-free consumer: plan bytes + a separately supplied current request.
    let request = payment_request();
    let request_bytes = encode_request(&request).unwrap();
    let components = payment_components();
    verify_plan(&plan_bytes, &request_bytes, &components)
        .expect("composed plan verifies against the current request");

    // A corrupted plan is rejected, and the canonical bytes are stable.
    let mut corrupted = plan_bytes.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0x01;
    assert!(verify_plan(&corrupted, &request_bytes, &components).is_err());

    let _ = std::fs::remove_dir_all(&workspace);
}

/// Rewrites the committed `root/inputs/*.bin` when an upstream canonical
/// encoding moves them deliberately; run with
/// `TOPOLOGY_REGENERATE_INPUTS=1` (same convention as
/// `codec.rs::regenerate_golden`).
#[test]
fn regenerate_package_inputs() {
    if std::env::var("TOPOLOGY_REGENERATE_INPUTS").is_err() {
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .join("tests/fixtures/packages/build-scope-topology")
        .canonicalize()
        .expect("committed fixture exists");
    let (request, components, bindings) = build_scope_inputs();
    for (name, bytes) in [
        ("request.bin", request.as_slice()),
        ("components.bin", components.as_slice()),
        ("bindings.bin", bindings.as_slice()),
    ] {
        std::fs::write(fixture.join("root/inputs").join(name), bytes).unwrap();
    }
}
