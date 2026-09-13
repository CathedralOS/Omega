use super::*;
use crate::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure_with_storage,
};
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct SourcePreparationFixture(PathBuf);

impl SourcePreparationFixture {
    fn new() -> Self {
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
        fs::write(root.join("package/main.omg"), "pub const VALUE: u32 = 7;\n").unwrap();
        Self(root)
    }

    fn closure(&self) -> crate::resolution::graph::ResolvedPackageSourceClosure {
        let storage = SourceResolverStorage::for_hardened_base(self.0.join("resolved")).unwrap();
        resolve_external_local_project_closure_with_storage(
            self.0.join("package"),
            ExternalSourceContext::derive(b"candidate-source-preparation"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
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
    let mut prepared_sources = vec![None; closure.graph().packages().len()];
    let discovery = compile_review_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        PackageSourcePreparation::Retain(&mut prepared_sources),
    )
    .expect("discovery retains immutable preparation");
    assert!(prepared_sources.iter().all(Option::is_some));
    assert!(
        candidate_semantic_binding_inputs(&discovery)
            .unwrap()
            .is_empty()
    );
    let consumed = compile_review_pass(
        &exact,
        &fixture.0.join("consumed"),
        &[],
        PackageSourcePreparation::Consume(&mut prepared_sources),
    )
    .expect("final pass consumes preparation under fresh sponsors");
    assert!(prepared_sources.iter().all(Option::is_none));
    let independent = compile_resolved_package_reviews(&exact, &fixture.0.join("independent"))
        .expect("independent reference pass");
    let candidate =
        compile_resolved_package_candidate_reviews(&exact, &fixture.0.join("candidate"))
            .expect("no binding returns discovery reviews");
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
    let mut prepared_sources = vec![None; closure.graph().packages().len()];
    compile_review_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        PackageSourcePreparation::Retain(&mut prepared_sources),
    )
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
    let result = compile_review_pass(
        &exact,
        &fixture.0.join("final"),
        &[],
        PackageSourcePreparation::Consume(&mut prepared_sources),
    );
    assert!(matches!(
        result,
        Err(CompileResolvedPackageReviewsError::SourceCustody {
            phase: PackageSourceVerificationPhase::BeforeCompilation,
            ..
        })
    ));
    assert!(prepared_sources.iter().all(Option::is_some));
    assert!(
        fs::read_dir(fixture.0.join("final"))
            .unwrap()
            .next()
            .is_none()
    );
}
