use super::{
    claim_free_review_fixture, graph_workbench_question, join_question, remove_temporary_tree,
    resolve_external_closure, split_question, temporary_root,
};
use crate::package_reconstruction_question::accepted_policy_fixture;
use package_evidence::ledger::OrdinaryPackageObligationStatus;
use package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewDangerousAuthorityClass,
};
use package_manager::admission::{
    ACCEPTED_ORDINARY_EVIDENCE_SCHEMA_VERSION, AcceptedOrdinaryEvidenceError,
    accept_ordinary_closure_evidence, accepted_terminal_authority_permission_policy,
};
use package_manager::resolution::graph::CanonicalSourceClosureSubject;
use package_manager::review::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionLimits,
    FreshPackageRootPolicyError, LocallyComposedPackageObligationResults,
    ReviewOnlyCapabilityConflictLimits, SemanticBindingReview, bind_fresh_package_root_policy,
    compare_review_only_initial_capabilities, compile_resolved_package_reviews,
};

#[test]
fn all_fresh_association_paths_reject_reviews_for_another_requested_target() {
    let (temporary, closure, reviews) = claim_free_review_fixture("requested-target");
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let conflict_limits = ReviewOnlyCapabilityConflictLimits::default();
    let target = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let original = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &target, &reviews, limits,
    )
    .expect("question for the reviewed target");
    let policy = accepted_policy_fixture::accepted_policy(&target, &reviews);
    accept_ordinary_closure_evidence(&target, &reviews, limits, conflict_limits, Some(&policy))
        .expect("matching source-only review reuses project acceptance");

    let other_target = closure.for_exact_target(target::TargetProfile::LinuxX64);
    assert!(
        CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
            &other_target,
            &reviews,
            limits,
        )
        .is_err(),
        "a source subject cannot relabel Windows review as Linux review"
    );
    assert!(
        LocallyComposedPackageObligationResults::from_resolved_and_reviews(
            &other_target,
            &reviews,
            limits,
        )
        .is_err(),
        "composition must retain the caller's exact requested target"
    );
    assert!(
        bind_fresh_package_root_policy(
            &other_target,
            &reviews,
            limits,
            conflict_limits,
            Some(&policy)
        )
        .is_err(),
        "root-policy binding cannot pair a different requested target"
    );
    assert!(
        accept_ordinary_closure_evidence(
            &other_target,
            &reviews,
            limits,
            conflict_limits,
            Some(&policy)
        )
        .is_err(),
        "in-memory acceptance cannot promote cross-target review"
    );

    let alternate_subject =
        CanonicalSourceClosureSubject::from_resolved(&other_target, limits.source_closure)
            .expect("canonical source subject for the other target");
    let (version, _, ledgers) = split_question(original.canonical_bytes());
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, alternate_subject.canonical_bytes(), &ledgers),
            limits,
        )
        .is_err(),
        "recovery must rejoin the source target and ledger target too"
    );
    remove_temporary_tree(&temporary);
}

#[test]
fn all_fresh_association_paths_reject_stale_review_for_same_named_source() {
    let (temporary, original_closure, reviews) = claim_free_review_fixture("stale-review");
    let policy = accepted_policy_fixture::accepted_policy(
        &original_closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
    );
    let root = temporary.join("root");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 2 }\n")
        .expect("change implementation without changing package name or public signature");
    let changed_closure = resolve_external_closure(&root, temporary.join("changed-cache"));
    assert_eq!(
        original_closure.graph().root(),
        changed_closure.graph().root(),
        "the immutable revision changes beneath the same source-qualified package key"
    );
    let target = changed_closure.for_exact_target(target::TargetProfile::WindowsX64);
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let conflict_limits = ReviewOnlyCapabilityConflictLimits::default();
    assert!(
        CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
            &target, &reviews, limits
        )
        .is_err()
    );
    assert!(
        LocallyComposedPackageObligationResults::from_resolved_and_reviews(
            &target, &reviews, limits
        )
        .is_err()
    );
    assert!(
        bind_fresh_package_root_policy(&target, &reviews, limits, conflict_limits, Some(&policy))
            .is_err()
    );
    assert!(
        accept_ordinary_closure_evidence(&target, &reviews, limits, conflict_limits, Some(&policy))
            .is_err()
    );

    let fresh_reviews = compile_resolved_package_reviews(
        &target,
        &temporary.join("changed-build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile the changed source at its own immutable resolution");
    let evidence = accept_ordinary_closure_evidence(
        &target,
        &fresh_reviews,
        limits,
        conflict_limits,
        Some(&policy),
    )
    .expect("changed implementation reuses unchanged accepted requirements");
    assert!(!evidence.policy_changes().requires_decision());
    assert!(evidence.policy_changes().source_subject_changed());
    assert!(evidence.policy_changes().audit_recommended());
    remove_temporary_tree(&temporary);
}

#[test]
fn canonical_question_round_trips_and_freshly_reconstructs_complete_closure() {
    let (temporary, closure, reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();

    assert_eq!(question.entries().len(), closure.graph().packages().len());
    assert_eq!(question.target_name(), "windows_x86_64");
    assert!(
        question
            .entries()
            .iter()
            .map(|entry| entry.package())
            .eq(question
                .source_closure()
                .packages()
                .iter()
                .map(|source| source.key()))
    );
    for entry in question.entries() {
        assert_eq!(entry.obligations().package(), entry.package().identity());
        let expected_transitive_packages = match entry.package().name().as_str() {
            "graph-workbench" => 4,
            "file-journal" => 2,
            "arithmetic-kernels" | "host-services" => 1,
            package => panic!("unexpected graph-workbench package `{package}`"),
        };
        assert_eq!(
            entry.obligations().dependency_closure().packages().len(),
            expected_transitive_packages,
            "each ledger must retain its own exact transitive closure"
        );
    }
    let composed = LocallyComposedPackageObligationResults::from_resolved_and_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        limits,
    )
    .expect("compose graph-workbench open obligations");
    assert_eq!(
        composed.question(),
        &question,
        "composition retains the exact canonical question and encoding"
    );
    let dangerous_authorities = composed
        .root_open_dangerous_authorities()
        .collect::<Vec<_>>();
    let [(owner, context, authority)] = dangerous_authorities.as_slice() else {
        panic!("graph-workbench must propagate one dependency-owned dangerous authority")
    };
    assert_eq!(owner.name().as_str(), "file-journal");
    assert!(context.purpose().is_product());
    assert_eq!(
        authority.status(),
        OrdinaryPackageObligationStatus::OpenRootAdmission
    );
    assert_eq!(
        authority.authority().class(),
        PackageReviewDangerousAuthorityClass::Filesystem
    );
    assert_eq!(
        authority.row().kind(),
        PackageReviewCanonicalRowKind::DangerousAuthority
    );
    assert_eq!(
        authority.row().risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );

    let recovered =
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), limits)
            .expect("recover canonical reconstruction question");
    assert_eq!(recovered, question);
    assert_eq!(recovered.fingerprint(), question.fingerprint());
    assert!(
        recovered
            .matches_resolved_and_reviews(
                &closure.for_exact_target(target::TargetProfile::WindowsX64),
                &reviews,
                limits
            )
            .expect("fresh source and review reconstruction should succeed")
    );
    let conflicts = compare_review_only_initial_capabilities(
        &reviews,
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("derive graph-workbench fresh conflicts");
    assert!(!conflicts.is_empty(), "fixture retains structural blockers");
    let accepted_policy = accepted_policy_fixture::accepted_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
    );
    let accepted = bind_fresh_package_root_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        limits,
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted_policy),
    )
    .expect("every exact graph-workbench blocker is accepted");
    assert_eq!(
        accepted
            .obligations()
            .root_open_dangerous_authorities()
            .count(),
        1
    );
    assert!(!accepted.policy_changes().requires_decision());
    assert_eq!(
        accepted.obligations().question(),
        &question,
        "root decisions do not change the canonical source/review association"
    );
    let evidence = accept_ordinary_closure_evidence(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        limits,
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted_policy),
    )
    .expect("fresh reconstruction and exact root policy issue accepted evidence");
    assert_eq!(
        evidence.acceptance().obligations().question(),
        &question,
        "in-memory acceptance preserves every canonical question byte"
    );
    assert_eq!(
        evidence.schema().version(),
        ACCEPTED_ORDINARY_EVIDENCE_SCHEMA_VERSION
    );
    assert_eq!(evidence.packages().len(), closure.graph().packages().len());
    assert_eq!(evidence.policy_changes(), accepted.policy_changes());
    let file_journal = evidence
        .packages()
        .iter()
        .find(|package| package.package().name().as_str() == "file-journal")
        .expect("dependency evidence retains its original package owner");
    assert_eq!(
        file_journal.artifact().package(),
        file_journal.package().identity()
    );
    assert_eq!(file_journal.results().open_dangerous_authorities().len(), 1);
    assert_eq!(
        file_journal.source_consumption(),
        file_journal
            .generated_sources()
            .source_consumption_commitment()
    );
    remove_temporary_tree(&temporary);
}

#[test]
fn fresh_closure_without_blockers_requires_and_reuses_project_acceptance() {
    let temporary = temporary_root("no-root-policy");
    let root = temporary.join("root");
    std::fs::create_dir_all(&root).expect("create claim-free root");
    std::fs::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("claim-free");
}
"#,
    )
    .expect("write claim-free build");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write claim-free source");

    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile claim-free package");

    assert!(matches!(
        bind_fresh_package_root_policy(
            &closure.for_exact_target(target::TargetProfile::WindowsX64), &reviews,
            CanonicalPackageReconstructionQuestionLimits::default(),
            ReviewOnlyCapabilityConflictLimits::default(), None,
        ),
        Err(FreshPackageRootPolicyError::ReviewRequired(changes))
            if changes.baseline_source_subject().is_none() && !changes.requires_decision()
    ));
    assert!(matches!(
        accept_ordinary_closure_evidence(
            &closure.for_exact_target(target::TargetProfile::WindowsX64),
            &reviews,
            CanonicalPackageReconstructionQuestionLimits::default(),
            ReviewOnlyCapabilityConflictLimits::default(),
            None,
        ),
        Err(AcceptedOrdinaryEvidenceError::RootPolicy(
            FreshPackageRootPolicyError::ReviewRequired(_)
        ))
    ));
    let policy = accepted_policy_fixture::accepted_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
    );
    let accepted = bind_fresh_package_root_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&policy),
    )
    .expect("claim-free closure reuses project acceptance");
    assert!(!accepted.policy_changes().requires_decision());
    assert!(
        accepted
            .obligations()
            .root_open_accepted_claims()
            .next()
            .is_none()
    );
    assert!(
        accepted
            .obligations()
            .root_open_dangerous_authorities()
            .next()
            .is_none()
    );
    let evidence = accept_ordinary_closure_evidence(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&policy),
    )
    .expect("blocker-free closure reuses accepted project policy");
    assert_eq!(evidence.policy_changes(), accepted.policy_changes());
    assert_eq!(evidence.packages().len(), 1);
    let empty_permission_policy = accepted_terminal_authority_permission_policy(&evidence)
        .expect("blocker-free accepted evidence projects deny-by-absence permission policy");
    assert!(empty_permission_policy.rows().is_empty());
    assert_eq!(
        empty_permission_policy.identity(),
        native_realization::current_terminal_authority_permission_policy().identity(),
    );

    remove_temporary_tree(&temporary);
}

#[test]
// Clearing readonly is the exact Windows operation needed to exercise custody
// tampering; Unix uses owner-write mode bits above.
#[allow(clippy::permissions_set_readonly_false)]
fn accepted_evidence_rechecks_live_source_custody_after_review() {
    let temporary = temporary_root("accepted-source-custody");
    let root = temporary.join("root");
    std::fs::create_dir_all(&root).expect("create accepted-evidence root");
    std::fs::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("custody-canary");
}
"#,
    )
    .expect("write custody-canary build");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write custody-canary source");

    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile custody-canary review");
    let policy = accepted_policy_fixture::accepted_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
    );
    let selected_root = closure.graph().root().clone();
    let snapshot_main = closure
        .source_root(&selected_root)
        .expect("root source custody")
        .join("main.omg");
    let mut permissions = std::fs::metadata(&snapshot_main)
        .expect("snapshot source metadata")
        .permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    std::fs::set_permissions(&snapshot_main, permissions).expect("make snapshot writable");
    std::fs::write(&snapshot_main, "pub machine altered() -> u64 { 2 }\n")
        .expect("tamper reviewed snapshot");

    assert!(matches!(
        accept_ordinary_closure_evidence(
            &closure.for_exact_target(target::TargetProfile::WindowsX64),
            &reviews,
            CanonicalPackageReconstructionQuestionLimits::default(),
            ReviewOnlyCapabilityConflictLimits::default(),
            Some(&policy),
        ),
        Err(AcceptedOrdinaryEvidenceError::SourceCustody { package, .. })
            if package == selected_root
    ));

    remove_temporary_tree(&temporary);
}
