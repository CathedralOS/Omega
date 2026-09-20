use super::ReviewOnlyCandidateClosureCommitment;
use super::commitments::{derive_candidate_closure_commitment, derive_candidate_graph_commitment};
use super::format::{
    row_kind_tag, row_kind_token, source_location_role_tag, source_location_role_token,
};
use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, PackageSourceClosureLimits, ResolvedPackageClosure,
    ResolvedPackageSourceClosure, resolve_external_local_package_closure_from_hardened_base,
};
use crate::review::ReviewOnlySourceConsumptionCommitment;
use crate::review::candidate::PackageReviewEvidence;
use package_evidence::record::PackageReviewCanonicalRow;
use package_evidence::record::{PackageReviewCanonicalRowKind, PackageReviewSourceLocationRole};
use package_source::{ExternalSourceContext, ImmutableSourceResolution, LocalSourceLimits};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn provider_grant_source_role_has_stable_review_rendering_identity() {
    assert_eq!(
        source_location_role_tag(PackageReviewSourceLocationRole::ProviderGrant),
        25
    );
    assert_eq!(
        source_location_role_token(PackageReviewSourceLocationRole::ProviderGrant),
        "provider_grant"
    );
}

#[test]
fn representation_selection_source_role_has_stable_review_rendering_identity() {
    assert_eq!(
        source_location_role_tag(PackageReviewSourceLocationRole::RepresentationSelection),
        28
    );
    assert_eq!(
        source_location_role_token(PackageReviewSourceLocationRole::RepresentationSelection),
        "representation_selection"
    );
}

#[test]
fn proof_only_quotient_review_rows_have_stable_comparison_identities() {
    assert_eq!(
        row_kind_tag(PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence),
        17
    );
    assert_eq!(
        row_kind_token(PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence),
        "non_executable_quotient_correspondence"
    );
    assert_eq!(
        source_location_role_tag(PackageReviewSourceLocationRole::QuotientOperationDeclaration),
        27
    );
    assert_eq!(
        source_location_role_token(PackageReviewSourceLocationRole::QuotientOperationDeclaration),
        "quotient_operation_declaration"
    );
}

#[test]
fn terminal_authority_permission_rows_have_stable_comparison_identities() {
    assert_eq!(
        row_kind_tag(PackageReviewCanonicalRowKind::TerminalAuthorityPermission),
        19
    );
    assert_eq!(
        row_kind_token(PackageReviewCanonicalRowKind::TerminalAuthorityPermission),
        "terminal_authority_permission"
    );
}

#[derive(Clone)]
struct TestReview {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    target: String,
    executable_incident_metadata: [u8; 32],
    source_consumption: ReviewOnlySourceConsumptionCommitment,
    build_observation: Option<[u8; 32]>,
    whole_review: [u8; 32],
    rows: Vec<PackageReviewCanonicalRow>,
}

impl PackageReviewEvidence for TestReview {
    fn key(&self) -> &PackageKey {
        &self.key
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    fn projection_identity_matches(&self) -> bool {
        true
    }

    fn target_name(&self) -> &str {
        &self.target
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        self.source_consumption
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation
    }

    fn whole_review_commitment(&self) -> [u8; 32] {
        self.whole_review
    }

    fn canonical_rows(&self) -> &[PackageReviewCanonicalRow] {
        &self.rows
    }
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-candidate-closure-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn write_package(root: &Path, name: &str, dependency: Option<&str>) {
    std::fs::create_dir_all(root).expect("create package root");
    let dependency = dependency.map_or_else(String::new, |location| {
        format!("    builder.depend(Source::Path {{\n        location: \"{location}\"\n    }});\n")
    });
    std::fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n{dependency}}}\n"
            ),
        )
        .expect("write package build declaration");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write package source");
}

fn commitment(
    closure: &ResolvedPackageSourceClosure,
    reviews: &[TestReview],
) -> ReviewOnlyCandidateClosureCommitment {
    let review_refs = reviews.iter().collect::<Vec<_>>();
    derive_candidate_closure_commitment(
        &closure.for_exact_target(target::TargetProfile::CrossPlatformCli),
        &review_refs,
    )
    .expect("derive candidate closure commitment")
}

#[test]
fn candidate_closure_binds_review_evidence_from_every_package() {
    let parent = temp_root("evidence");
    let root = parent.join("root");
    let dependency = parent.join("dependency");
    let cache = temp_root("cache");
    write_package(&dependency, "closure-dependency", None);
    write_package(&root, "closure-root", Some("../dependency"));
    let closure = resolve_external_local_package_closure_from_hardened_base(
        &root,
        ExternalSourceContext::derive(b"candidate-closure-review-evidence"),
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve two-package source closure");

    let mut reviews = closure
        .graph()
        .packages()
        .iter()
        .enumerate()
        .map(|(index, package)| TestReview {
            key: package.source().key().clone(),
            resolution: package.source().resolution().clone(),
            target: "windows_x86_64".to_owned(),
            executable_incident_metadata: [1; 32],
            source_consumption: ReviewOnlySourceConsumptionCommitment::for_test_digest([2; 32]),
            build_observation: None,
            whole_review: [u8::try_from(index + 3).expect("small fixture index"); 32],
            rows: Vec::new(),
        })
        .collect::<Vec<_>>();
    reviews.sort_by(|left, right| left.key.cmp(&right.key));
    let dependency_index = reviews
        .iter()
        .position(|review| review.key.name().as_str() == "closure-dependency")
        .expect("dependency review");
    let baseline = commitment(&closure, &reviews);

    let mut metadata_only = reviews.clone();
    metadata_only[dependency_index].executable_incident_metadata = [9; 32];
    assert_eq!(commitment(&closure, &metadata_only), baseline);

    for change in 0..4 {
        let mut changed = reviews.clone();
        let review = &mut changed[dependency_index];
        match change {
            0 => review.target = "linux_x86_64".to_owned(),
            1 => {
                review.source_consumption =
                    ReviewOnlySourceConsumptionCommitment::for_test_digest([9; 32])
            }
            2 => review.build_observation = Some([9; 32]),
            3 => review.whole_review = [9; 32],
            _ => unreachable!("four semantic evidence axes"),
        }
        assert_ne!(commitment(&closure, &changed), baseline);
    }

    let _ = std::fs::remove_dir_all(parent);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn candidate_closure_binds_the_selected_target_profile() {
    let root = temp_root("target-profile");
    let cache = temp_root("target-profile-cache");
    write_package(&root, "profile-probe", None);
    let closure = resolve_external_local_package_closure_from_hardened_base(
        &root,
        ExternalSourceContext::derive(b"candidate-closure-target-profile"),
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve source closure once");
    let windows = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let linux = closure.for_exact_target(target::TargetProfile::LinuxX64);

    let reviews = closure
        .graph()
        .packages()
        .iter()
        .map(|package| TestReview {
            key: package.source().key().clone(),
            resolution: package.source().resolution().clone(),
            target: "same-compiler-target".to_owned(),
            executable_incident_metadata: [1; 32],
            source_consumption: ReviewOnlySourceConsumptionCommitment::for_test_digest([2; 32]),
            build_observation: None,
            whole_review: [3; 32],
            rows: Vec::new(),
        })
        .collect::<Vec<_>>();
    let review_refs = reviews.iter().collect::<Vec<_>>();

    assert_ne!(
        derive_candidate_closure_commitment(&windows, &review_refs).expect("commit Windows child"),
        derive_candidate_closure_commitment(&linux, &review_refs).expect("commit Linux child"),
        "review identity must bind the package closure's selected profile"
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn candidate_closure_binds_the_exact_root_role() {
    let root = temp_root("root-role");
    let cache = temp_root("root-role-cache");
    write_package(&root, "role-probe", None);
    let closure = resolve_external_local_package_closure_from_hardened_base(
        &root,
        ExternalSourceContext::derive(b"candidate-closure-root-role"),
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve one-package source closure");
    let package_graph = closure.graph().clone();
    let application_graph = ResolvedPackageClosure::new(
        package_graph.root().clone(),
        BuildDeclarationKind::Application,
        package_graph.packages().to_vec(),
    )
    .expect("same package graph may select an application root");
    let reviews = package_graph
        .packages()
        .iter()
        .map(|package| TestReview {
            key: package.source().key().clone(),
            resolution: package.source().resolution().clone(),
            target: "windows_x86_64".to_owned(),
            executable_incident_metadata: [1; 32],
            source_consumption: ReviewOnlySourceConsumptionCommitment::for_test_digest([2; 32]),
            build_observation: None,
            whole_review: [3; 32],
            rows: Vec::new(),
        })
        .collect::<Vec<_>>();
    let review_refs = reviews.iter().collect::<Vec<_>>();

    assert_ne!(
        derive_candidate_graph_commitment(&package_graph, None, &review_refs)
            .expect("commit package-root graph"),
        derive_candidate_graph_commitment(&application_graph, None, &review_refs)
            .expect("commit application-root graph"),
        "candidate closure identity must bind root role"
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn package_changes_join_both_sides_to_the_occurrence_roster() {
    let Some(target) = target::TargetProfile::host_if_supported() else {
        eprintln!("SKIP: package build review requires a catalogued execution host");
        return;
    };
    let parent = temp_root("occurrence-roster");
    let baseline_root = parent.join("baseline").join("root");
    let dep = parent.join("dep");
    let candidate_root = parent.join("candidate").join("root");
    write_package(&dep, "occurrence-dep", None);
    std::fs::create_dir_all(&baseline_root).expect("create baseline root");
    std::fs::write(
        baseline_root.join("build.omg"),
        concat!(
            "machine build(builder: &mut Build) {\n",
            "    builder.package(\"baseline-root\");\n",
            "    builder.depend(Source::Path { location: \"../../dep\" });\n",
            "}\n"
        ),
    )
    .expect("write baseline root");
    std::fs::write(
        baseline_root.join("main.omg"),
        "pub machine value() -> u64 { 1 }\n",
    )
    .expect("write baseline root source");
    std::fs::create_dir_all(&candidate_root).expect("create candidate root");
    std::fs::write(
        candidate_root.join("build.omg"),
        concat!(
            "machine build(builder: &mut Build) {\n",
            "    builder.package(\"candidate-root\");\n",
            "    builder.build_depend_as(\"dep_build\", Source::Path { location: \"../../dep\" });\n",
            "}\n"
        ),
    )
    .expect("write build-purpose candidate root");
    std::fs::write(
        candidate_root.join("main.omg"),
        "pub machine value() -> u64 { 2 }\n",
    )
    .expect("write candidate root source");

    // A purpose change is reviewable without collapsing two simultaneous
    // occurrences into one package review. Dual-purpose review production
    // remains rejected until it can publish one review per occurrence.
    let baseline_closure = resolve_external_local_package_closure_from_hardened_base(
        &baseline_root,
        ExternalSourceContext::derive(b"occurrence-roster-context"),
        parent.join("baseline-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve baseline closure");
    let baseline_target = baseline_closure.for_exact_target(target);
    let baseline_reviews = crate::review::compile_resolved_package_reviews(
        &baseline_target,
        &parent.join("baseline-build"),
        crate::review::SemanticBindingReview::Discover,
    )
    .expect("compile baseline reviews");
    let initial = crate::review::compare_package_policy_changes(
        None,
        &baseline_reviews,
        &baseline_target,
        super::PackagePolicyChangeLimits::default(),
    )
    .expect("initial comparison");
    let choices = initial
        .packages()
        .iter()
        .flat_map(|package| package.rows())
        .filter(|row| row.requires_decision())
        .map(|row| crate::review::PackagePolicyDecision {
            subject: crate::review::PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
            disposition: crate::review::ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        })
        .collect::<Vec<_>>();
    let resolution = crate::review::resolve_package_policy_decisions(
        &initial,
        initial.fingerprint().digest(),
        &choices,
    )
    .expect("resolve initial choices");
    let baseline_source = CanonicalSourceClosureSubject::from_resolved(
        &baseline_target,
        crate::resolution::graph::CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("baseline subject");
    let baselines = baseline_source
        .packages()
        .iter()
        .map(|package| {
            baseline_reviews
                .review(package.key())
                .expect("baseline review")
                .policy()
                .clone()
        })
        .collect();
    let history = crate::lock::HistoricalPackagePolicyDecisions::capture_policy(
        &baseline_source,
        &initial,
        &resolution,
        crate::lock::HistoricalPackagePolicyLimits::default(),
    )
    .expect("capture initial choices");
    let accepted = crate::lock::PackageLockTarget::from_parts(baseline_source, baselines, history)
        .expect("accepted baseline");

    // The unchanged closure keeps one product occurrence per package.
    let stable = crate::review::compare_package_policy_changes(
        Some(&accepted),
        &baseline_reviews,
        &baseline_target,
        super::PackagePolicyChangeLimits::default(),
    )
    .expect("stable comparison");
    let stable_dep = stable
        .packages()
        .iter()
        .find(|change| change.key().name().as_str() == "occurrence-dep")
        .expect("dep change row");
    assert_eq!(
        stable_dep.baseline_occurrence_purposes(),
        Some(&[crate::declarations::dependencies::DependencyPurpose::Product][..])
    );
    assert_eq!(
        stable_dep.candidate_occurrence_purposes(),
        stable_dep.baseline_occurrence_purposes()
    );
    assert!(!stable_dep.occurrence_purposes_changed());

    let candidate_closure = resolve_external_local_package_closure_from_hardened_base(
        &candidate_root,
        ExternalSourceContext::derive(b"occurrence-roster-context"),
        parent.join("candidate-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve candidate closure");
    let candidate_target = candidate_closure.for_exact_target(target);
    let candidate_reviews = crate::review::compile_resolved_package_reviews(
        &candidate_target,
        &parent.join("candidate-build"),
        crate::review::SemanticBindingReview::Discover,
    )
    .expect("compile candidate reviews");
    let changes = crate::review::compare_package_policy_changes(
        Some(&accepted),
        &candidate_reviews,
        &candidate_target,
        super::PackagePolicyChangeLimits::default(),
    )
    .expect("product-to-build purpose comparison");
    let dep_change = changes
        .packages()
        .iter()
        .find(|change| change.key().name().as_str() == "occurrence-dep")
        .expect("dep still shares custody");
    assert_eq!(
        dep_change.baseline_occurrence_purposes(),
        Some(&[crate::declarations::dependencies::DependencyPurpose::Product][..])
    );
    assert_eq!(
        dep_change.candidate_occurrence_purposes(),
        Some(&[crate::declarations::dependencies::DependencyPurpose::Build][..])
    );
    assert!(dep_change.occurrence_purposes_changed());
    assert!(
        dep_change.audit_recommended(),
        "gaining a build occurrence recommends audit"
    );
    let _ = std::fs::remove_dir_all(parent);
}
