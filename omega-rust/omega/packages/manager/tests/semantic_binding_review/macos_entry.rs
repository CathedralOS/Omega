use super::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole,
    CanonicalPackageReconstructionQuestionLimits, ConsumerScopedSemanticBindingReviewInput,
    ExternalSourceContext, FreshPackageRootPolicyError, LocalSourceLimits,
    PackageSourceClosureLimits, Path, ReviewOnlyCapabilityConflictLimits, SemanticBindingReview,
    SourceResolverStorage, TemporaryTree, bind_fresh_package_root_policy,
    compile_resolved_package_candidate_for_production, compile_resolved_package_reviews,
    resolve_external_local_project_closure, write_file,
};
use package_manager::resolution::graph::GitResolutionOptions;
use package_source::PrimaryGitChoices;
#[test]
fn target_entry_dependency_discovery_requires_explicit_consumer_acceptance() {
    let temporary = TemporaryTree::new();
    let application = temporary.package("application");
    let standard_library = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root")
        .join("source/library/std");
    write_file(
        application.join("main.omg"),
        "data Main { count: u64; }\nmachine Main::main(&mut self) { self.count = 1; }\n",
    );
    write_file(
        application.join("build.omg"),
        &format!(
            "machine build(builder: &mut Build) {{ builder.application(\"hosted-consumer\"); builder.depend_as(\"ordinary_std\", Source::Path {{ location: {:?} }}); builder.roots.bind(macos_arm64::ProgramEntry, Main::main); }}\n",
            standard_library.to_str().expect("fixture source path")
        ),
    );
    let storage = SourceResolverStorage::for_hardened_base(
        temporary.0.join("resolved"),
        PrimaryGitChoices::default(),
    )
    .unwrap();
    let closure = resolve_external_local_project_closure(
        &application,
        ExternalSourceContext::derive(b"macos-entry-dependency-discovery"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .expect("resolve unchanged hosted app plus ordinary std dependency");
    let target = closure.for_exact_target(target::TargetProfile::MacosArm64);
    let candidate = compile_resolved_package_candidate_for_production(
        &target,
        &temporary.0.join("review"),
        SemanticBindingReview::Discover,
        None,
    )
    .expect("checked dependency schema feeds strict application review without an authored import");
    let review = candidate.reviews().review(closure.graph().root()).unwrap();
    let bindings = review.semantic_bindings();
    let entry = bindings
        .iter()
        .find(|binding| binding.role() == AcceptedSemanticBindingRole::MacosArm64ProgramEntry)
        .expect("review retains the exact newly proposed semantic role");
    assert_eq!(entry.declaration_path(), "MacosApplication");
    assert_ne!(entry.package(), closure.graph().root().identity());
    assert!(
        entry.selected_provider_plan_digest().is_none(),
        "entry schema does not synthesize a provider"
    );
    assert!(
        matches!(
            bind_fresh_package_root_policy(
                &target,
                candidate.reviews(),
                CanonicalPackageReconstructionQuestionLimits::default(),
                ReviewOnlyCapabilityConflictLimits::default(),
                None,
            ),
            Err(FreshPackageRootPolicyError::ReviewRequired(_))
        ),
        "candidate compilation is not native publication acceptance"
    );
    assert!(
        compile_resolved_package_reviews(
            &target,
            &temporary.0.join("strict-unbound"),
            SemanticBindingReview::Explicit(&[]),
        )
        .is_err(),
        "ordinary review cannot discover an accepted role implicitly"
    );
    let exact = ConsumerScopedSemanticBindingReviewInput::new(
        closure.graph().root().clone(),
        entry.clone(),
    );
    let explicit = compile_resolved_package_reviews(
        &target,
        &temporary.0.join("strict-bound"),
        SemanticBindingReview::Explicit(std::slice::from_ref(&exact)),
    )
    .expect("exact discovered candidate survives independent explicit checking");
    assert_eq!(
        explicit
            .review(closure.graph().root())
            .unwrap()
            .semantic_bindings(),
        bindings
    );
    let changed = AcceptedSemanticBinding::new_service(
        entry.role(),
        entry.package(),
        "DifferentApplication",
        entry.normalized_schema_digest(),
    )
    .unwrap();
    let changed =
        ConsumerScopedSemanticBindingReviewInput::new(closure.graph().root().clone(), changed);
    assert!(
        compile_resolved_package_reviews(
            &target,
            &temporary.0.join("strict-changed"),
            SemanticBindingReview::Explicit(std::slice::from_ref(&changed)),
        )
        .is_err(),
        "explicit stale entry must reject rather than rediscover its replacement"
    );
}
