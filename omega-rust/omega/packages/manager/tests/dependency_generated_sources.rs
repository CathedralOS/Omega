use package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolveWorkspacePackageClosureError, ResolvedPackageSourceClosure,
    resolve_workspace_package_closure_with_storage,
};
use package_manager::resolution::source::ResolvePackageSourceError;
use package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDisposition,
    compare_review_only_initial_capabilities, compile_resolved_package_candidate_for_production,
    compile_resolved_package_reviews, resolve_review_only_root_policy_decisions,
};
use package_source::{LocalSourceLimits, SourceLineage, SourceRelativePath, SourceResolverStorage};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| ancestor.join("tests/fixtures/packages").is_dir())
        .expect("package-manager should live beneath the Omega workspace")
        .to_path_buf()
}

fn temporary_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-dependency-generated-source-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ))
}

fn resolve_workspace_package_closure(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_workspace_package_closure_with_storage(
        workspace_root_source,
        root_member_path,
        live_workspace_root,
        &storage,
        source_limits,
        closure_limits,
    )
}

#[test]
fn dependency_generated_source_enters_consumer_without_rerunning_the_dependency_build() {
    let temporary = temporary_root();
    let fixtures = workspace_root().join("tests/fixtures/packages");
    let lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let closure = resolve_workspace_package_closure(
        &lineage,
        SourceRelativePath::parse("generated-consumer").unwrap(),
        &fixtures,
        temporary.join("cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve generated producer and consumer");

    assert!(matches!(
        compile_resolved_package_candidate_for_production(
            &closure.for_exact_target(target::TargetProfile::WindowsX64),
            &temporary.join("invalid-native-production"),
        ),
        Err(
            CompileResolvedPackageReviewsError::InvalidProductionRootRole {
                role: package_compilation::BuildDeclarationKind::Package,
                ..
            }
        )
    ));

    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("compiler-build"),
    )
    .expect("dependency generated source should enter consumer compilation");

    let producer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-table")
        .expect("producer review");
    let consumer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-consumer")
        .expect("consumer review");
    let [generated] = producer.generated_source_bundle().sources() else {
        panic!("producer should retain exactly one generated source")
    };
    assert_eq!(generated.relative_path(), b"table.generated.omg");
    assert_eq!(
        generated.bytes(),
        b"pub machine table_size() -> u64 {\n    3\n}\n"
    );
    assert!(consumer.generated_source_bundle().sources().is_empty());
    for review in [producer, consumer] {
        let policy = review.policy();
        assert_eq!(policy.package(), review.key().identity());
        assert_eq!(policy.target(), target::TargetProfile::WindowsX64);
        let bytes = policy
            .canonical_bytes()
            .expect("encode generated-source policy");
        let recovered = package_evidence::record::PackagePolicyBaseline::recover_canonical(
            &bytes,
            package_evidence::encoding::PackagePolicyRecoveryLimits::default(),
        )
        .expect("recover generated-source policy without build replay");
        assert_eq!(&recovered, policy);
        assert_eq!(
            policy
                .callables()
                .callables()
                .iter()
                .filter(|callable| callable.role()
                    == package_evidence::record::PackagePolicyCallableRole::Public)
                .count(),
            review
                .projection()
                .callables()
                .iter()
                .filter(|callable| callable.role()
                    == package_evidence::record::PackageReviewCallableRole::Public)
                .count(),
            "normalized policy includes the same checked generated public callables",
        );
    }
    assert_eq!(
        producer
            .build_observation_summary()
            .expect("producer build observations")
            .filesystem_operation_attempts()
            .len(),
        6,
        "dependency build must execute only in its own checked run",
    );
    assert_eq!(
        consumer
            .build_observation_summary()
            .expect("consumer build observations")
            .filesystem_operation_attempts()
            .len(),
        0,
        "consumer compilation must inject retained bytes without rerunning dependency build",
    );
    assert!(
        consumer
            .projection()
            .callables()
            .iter()
            .any(|callable| { callable.identity().path() == "consume_generated_table" })
    );

    let target = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let conflict_limits = ReviewOnlyCapabilityConflictLimits::default();
    let conflicts = compare_review_only_initial_capabilities(&reviews, &target, conflict_limits)
        .expect("derive exact generated-source review decisions");
    let decisions = conflicts
        .packages()
        .iter()
        .flat_map(|package| {
            package
                .conflicts()
                .iter()
                .filter(|conflict| conflict.is_blocking())
                .map(|conflict| {
                    package
                        .root_policy_decision(
                            conflict,
                            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
                        )
                        .expect("bind generated-source decision to its exact package row")
                })
        })
        .collect::<Vec<_>>();
    let policy = (!decisions.is_empty()).then(|| {
        resolve_review_only_root_policy_decisions(&conflicts, &decisions)
            .expect("resolve generated-source blockers")
    });
    let accepted = accept_ordinary_closure_evidence(
        &target,
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        conflict_limits,
        policy.as_ref(),
    )
    .expect("source-only acceptance retains generated-source ownership");
    for review in reviews.reviews() {
        let package = accepted
            .packages()
            .iter()
            .find(|package| package.package() == review.key())
            .expect("accepted package");
        assert_eq!(
            package.generated_sources(),
            review.generated_source_bundle(),
            "each package keeps its own complete generated-source bundle"
        );
        assert_eq!(
            package.source_consumption(),
            review.source_consumption_commitment()
        );
        assert_eq!(
            package.build_observation(),
            review.build_observation_summary(),
            "acceptance preserves the observed build rather than executing it again"
        );
    }

    let _ = std::fs::remove_dir_all(temporary);
}
use package_manager::admission::accept_ordinary_closure_evidence;
