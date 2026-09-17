use super::super::PackageSourceVerificationPhase;
use super::{
    CandidateSourcePreparation, CompileResolvedPackageReviewsError, SemanticBindingReview,
    TargetEntryDiscovery, candidate_semantic_binding_inputs, compile_pass,
    compile_resolved_package_candidate_for_check,
    compile_resolved_package_candidate_for_production, compile_resolved_package_reviews,
    compile_resolved_package_reviews_reusing,
};
use crate::resolution::graph::GitResolutionOptions;
use crate::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure,
};
use package_source::PrimaryGitChoices;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

#[test]
fn production_rejects_package_role_before_either_binding_mode_creates_a_session() {
    let fixture = SourcePreparationFixture::new();
    let closure = fixture.closure();
    let exact = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let build = fixture.0.join("production");
    for bindings in [
        SemanticBindingReview::Discover,
        SemanticBindingReview::Explicit(&[]),
    ] {
        assert!(matches!(
            compile_resolved_package_candidate_for_production(&exact, &build, bindings),
            Err(CompileResolvedPackageReviewsError::InvalidProductionRootRole { .. })
        ));
        assert!(!build.exists());
    }
}

#[test]
fn check_retains_requested_package_entry_after_disposal_without_production() {
    let fixture = SourcePreparationFixture::new();
    fs::write(
        fixture.0.join("package/main.omg"),
        "invalid unselected source",
    )
    .unwrap();
    fs::write(
        fixture.0.join("package/entry.omg"),
        "pub const VALUE: u32 = 8;\n",
    )
    .unwrap();
    let closure = fixture.closure();
    let target = target::TargetProfile::WindowsX64;
    let entry = closure
        .source_root(closure.graph().root())
        .unwrap()
        .join("entry.omg");
    let build = fixture.0.join("check");
    let checked = compile_resolved_package_candidate_for_check(
        &closure.for_exact_target(target),
        &build,
        &entry,
    )
    .expect("check accepts a package and does not read its unselected main");
    assert_eq!(checked.selected_target_profile(), Some(target));
    assert!(fs::read_dir(&build).unwrap().next().is_none());
    checked.verify_current_source_consumption().unwrap();
}

struct SourcePreparationFixture(PathBuf);

impl SourcePreparationFixture {
    fn new() -> Self {
        Self::with_main("pub const VALUE: u32 = 7;\n")
    }

    fn with_main(main_source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-candidate-source-preparation-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("package")).unwrap();
        fs::write(
            root.join("package/build.omg"),
            "machine build(builder: &mut Build) { builder.package(\"prepared-package\"); }\n",
        )
        .unwrap();
        fs::write(root.join("package/main.omg"), main_source).unwrap();
        Self(root)
    }

    fn closure(&self) -> crate::resolution::graph::ResolvedPackageSourceClosure {
        let storage = SourceResolverStorage::for_hardened_base(
            self.0.join("resolved"),
            PrimaryGitChoices::default(),
        )
        .unwrap();
        resolve_external_local_project_closure(
            self.0.join("package"),
            ExternalSourceContext::derive(b"candidate-source-preparation"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            GitResolutionOptions::default(),
        )
        .unwrap()
    }
}

impl Drop for SourcePreparationFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn retained_source_review_matches_independent_and_no_binding_candidates() {
    let fixture = SourcePreparationFixture::new();
    let closure = fixture.closure();
    let exact = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let discovery = compile_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        None,
        TargetEntryDiscovery::Dependencies,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("discovery retains immutable preparation");
    assert!(preparation.slots.iter().all(Option::is_some));
    assert_eq!(
        preparation.fresh_preparation_count(),
        closure.graph().packages().len()
    );
    assert!(
        candidate_semantic_binding_inputs(&discovery)
            .unwrap()
            .is_empty()
    );
    let consumed = compile_pass(
        &exact,
        &fixture.0.join("consumed"),
        &[],
        None,
        TargetEntryDiscovery::Disabled,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("final pass reuses preparation under fresh sponsors");
    assert!(preparation.slots.iter().all(Option::is_some));
    assert_eq!(
        preparation.fresh_preparation_count(),
        closure.graph().packages().len(),
        "the final pass prepared nothing fresh"
    );
    let independent = compile_resolved_package_reviews(
        &exact,
        &fixture.0.join("independent"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("independent reference pass");
    let candidate = compile_resolved_package_reviews_reusing(
        &exact,
        &fixture.0.join("candidate"),
        SemanticBindingReview::Discover,
        &mut preparation,
    )
    .expect("a repeated candidate reuses the same preparation");
    assert_eq!(
        preparation.fresh_preparation_count(),
        closure.graph().packages().len(),
        "an unchanged repeated candidate prepared no source again"
    );
    let reference = independent.review(closure.graph().root()).unwrap();
    for reviews in [&discovery, &consumed, &candidate] {
        let review = reviews.review(closure.graph().root()).unwrap();
        assert_eq!(
            review.source_consumption_commitment(),
            reference.source_consumption_commitment()
        );
        assert_eq!(
            review.canonical_review_bytes,
            reference.canonical_review_bytes
        );
        assert_eq!(review.semantic_bindings(), reference.semantic_bindings());
    }
    for directory in ["discovery", "consumed", "independent", "candidate"] {
        assert!(
            fs::read_dir(fixture.0.join(directory))
                .unwrap()
                .next()
                .is_none()
        );
    }
}

#[test]
#[cfg_attr(not(unix), allow(clippy::permissions_set_readonly_false))]
fn retained_source_review_rejects_source_drift_before_consuming_checkpoint() {
    let fixture = SourcePreparationFixture::new();
    let closure = fixture.closure();
    let exact = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    compile_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        None,
        TargetEntryDiscovery::Dependencies,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("discovery completes before source drift");
    let main = closure
        .source_root(closure.graph().root())
        .unwrap()
        .join("main.omg");
    let mut permissions = fs::metadata(&main).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    fs::set_permissions(&main, permissions).unwrap();
    fs::write(&main, "pub const VALUE: u32 = 9;\n").unwrap();
    let result = compile_pass(
        &exact,
        &fixture.0.join("final"),
        &[],
        None,
        TargetEntryDiscovery::Disabled,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews);
    assert!(matches!(
        result,
        Err(CompileResolvedPackageReviewsError::SourceCustody {
            phase: PackageSourceVerificationPhase::BeforeCompilation,
            ..
        })
    ));
    assert!(preparation.slots.iter().all(Option::is_some));
    assert!(
        fs::read_dir(fixture.0.join("final"))
            .unwrap()
            .next()
            .is_none()
    );
}

/// The effects canary source rides the review route unchanged: one store
/// serves repeated and cross-target candidates while custody still rejects a
/// changed source before its retained checkpoint can be consumed.
#[test]
#[cfg_attr(not(unix), allow(clippy::permissions_set_readonly_false))]
fn shared_preparation_serves_cross_target_reviews_and_still_rejects_drift() {
    let fixture = SourcePreparationFixture::with_main(include_str!(
        "../../../../../../../../tests/omega/pass/effects/nominal_callback_const_reach/main.omg"
    ));
    let closure = fixture.closure();
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let package_count = closure.graph().packages().len();

    let mut references = Vec::new();
    let mut reused = Vec::new();
    for target in [
        target::TargetProfile::WindowsX64,
        target::TargetProfile::LinuxX64,
    ] {
        let exact = closure.for_exact_target(target);
        references.push(
            compile_resolved_package_reviews(
                &exact,
                &fixture.0.join(format!(
                    "reference-{}",
                    exact.target_profile().target_name()
                )),
                SemanticBindingReview::Discover,
            )
            .expect("independent reference review"),
        );
        reused.push(
            compile_resolved_package_reviews_reusing(
                &exact,
                &fixture
                    .0
                    .join(format!("reused-{}", exact.target_profile().target_name())),
                SemanticBindingReview::Discover,
                &mut preparation,
            )
            .expect("cross-target review reuses prepared sources"),
        );
    }
    assert_eq!(
        preparation.fresh_preparation_count(),
        package_count,
        "every package prepared once across both target reviews"
    );
    for (reused, reference) in reused.iter().zip(&references) {
        let review = reused.review(closure.graph().root()).unwrap();
        let reference = reference.review(closure.graph().root()).unwrap();
        assert_eq!(
            review.source_consumption_commitment(),
            reference.source_consumption_commitment()
        );
        assert_eq!(
            review.canonical_review_bytes,
            reference.canonical_review_bytes
        );
        assert_eq!(review.semantic_bindings(), reference.semantic_bindings());
    }

    // A changed source invalidates: custody rejects before the retained
    // checkpoint can supply its stale parse frontier.
    let main = closure
        .source_root(closure.graph().root())
        .unwrap()
        .join("main.omg");
    let mut permissions = fs::metadata(&main).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    fs::set_permissions(&main, permissions).unwrap();
    fs::write(&main, "pub const VALUE: u32 = 9;\n").unwrap();
    let result = compile_resolved_package_reviews_reusing(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &fixture.0.join("drifted"),
        SemanticBindingReview::Discover,
        &mut preparation,
    );
    assert!(matches!(
        result,
        Err(CompileResolvedPackageReviewsError::SourceCustody {
            phase: PackageSourceVerificationPhase::BeforeCompilation,
            ..
        })
    ));
}
