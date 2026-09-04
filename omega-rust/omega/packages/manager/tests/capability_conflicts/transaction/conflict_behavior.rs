use super::fixture::ExactCompilerRowScenario;
use super::*;

pub(super) fn assert_comparison_limits_and_risk_classes(
    scenario: &ExactCompilerRowScenario,
    conflicts: &omega_package_manager::review::ReviewOnlyCapabilityConflictSet,
) {
    let repeated = compare_review_only_capabilities(
        &scenario.baseline_reviews,
        &scenario.candidate_reviews,
        &scenario
            .candidate_sources
            .for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("repeat deterministic comparison");
    assert_eq!(&repeated, conflicts);

    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render bounded conflict evidence");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V20\n"));
    assert!(rendered.contains("change added\nkind public_proposition\nrisk blocking\n"));
    assert!(rendered.contains("candidate_location declaration package "));
    assert!(rendered.contains(" \"main.omg\"\n"));
    assert!(!rendered.contains(&scenario.live.display().to_string()));
    assert!(!rendered.contains(&scenario.candidate_cache.display().to_string()));
    assert_eq!(
        conflicts
            .render_bounded(rendered.len())
            .expect("exact output ceiling"),
        rendered
    );
    let too_small = conflicts
        .render_bounded(rendered.len() - 1)
        .expect_err("renderer rejects rather than truncating");
    assert_eq!(too_small.required_bytes(), Some(rendered.len()));

    let zero_conflicts = compare_review_only_capabilities(
        &scenario.baseline_reviews,
        &scenario.candidate_reviews,
        &scenario
            .candidate_sources
            .for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::new(
            4_096,
            131_072,
            16 * 1024 * 1024,
            32 * 1024 * 1024,
            262_144,
            16 * 1024 * 1024,
            0,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            1_024,
        ),
    )
    .expect_err("row-count ceiling rejects without a partial result");
    assert!(matches!(
        zero_conflicts,
        ReviewOnlyCapabilityConflictError::TooManyConflicts { maximum: 0 }
    ));
    let zero_bytes = compare_review_only_capabilities(
        &scenario.baseline_reviews,
        &scenario.candidate_reviews,
        &scenario
            .candidate_sources
            .for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::new(
            4_096,
            131_072,
            16 * 1024 * 1024,
            32 * 1024 * 1024,
            262_144,
            16 * 1024 * 1024,
            65_536,
            0,
            8 * 1024 * 1024,
            1_024,
        ),
    )
    .expect_err("changed-row byte ceiling rejects without a partial result");
    assert!(matches!(
        zero_bytes,
        ReviewOnlyCapabilityConflictError::ChangedRowBytesExceeded { maximum_bytes: 0 }
    ));

    let mismatched_custody = compare_review_only_capabilities(
        &scenario.baseline_reviews,
        &scenario.candidate_reviews,
        &scenario
            .baseline_sources
            .for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect_err("candidate evidence cannot detach from candidate custody");
    assert!(matches!(
        mismatched_custody,
        ReviewOnlyCapabilityConflictError::CandidateResolutionMismatch { .. }
    ));

    let unchanged = compare_review_only_capabilities(
        &scenario.baseline_reviews,
        &scenario.baseline_reviews,
        &scenario
            .baseline_sources
            .for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("unchanged rows compare cleanly");
    assert!(unchanged.is_empty());
    assert!(matches!(
        resolve_review_only_root_policy_decisions(&unchanged, &[]),
        Err(ReviewOnlyRootPolicyResolutionError::NoBlockingConflicts)
    ));

    let removal_conflicts = compare_review_only_capabilities(
        &scenario.candidate_reviews,
        &scenario.baseline_reviews,
        &scenario
            .baseline_sources
            .for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare blocking candidate removals");
    let [removal_package] = removal_conflicts.packages() else {
        panic!("one package removes public propositions")
    };
    assert_eq!(removal_package.conflicts().len(), 2);
    for removal in removal_package.conflicts() {
        assert_eq!(
            removal.kind(),
            PackageReviewCanonicalRowKind::PublicProposition
        );
        assert_eq!(
            removal.change(),
            ReviewOnlyCapabilityConflictChange::Removed
        );
        assert!(removal.baseline_row().is_some());
        assert!(removal.candidate_row().is_none());
        assert!(removal.is_blocking());
        removal_package
            .root_policy_decision(
                removal,
                ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
            )
            .expect("candidate-change decision also covers row removal");
    }

    let record_limits = ReviewOnlyRootPolicyRecordLimits::default();

    write_package(
        &scenario.live,
        r#"boundary data PlatformToken;

pub machine add_u64(left: u64, right: u64) -> u64 {
    left + right
}
"#,
    );
    let representation_sources = resolve_external_local_package_closure(
        &scenario.live,
        ExternalSourceContext::derive(b"capability-conflict-test-lock"),
        &scenario.representation_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve representation-TCB candidate");
    let representation_reviews = compile_resolved_package_reviews(
        &representation_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &scenario.build_root,
    )
    .expect("compile representation-TCB review");
    let representation_conflicts = compare_review_only_capabilities(
        &scenario.baseline_reviews,
        &representation_reviews,
        &representation_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare representation-TCB row");
    let [representation_package] = representation_conflicts.packages() else {
        panic!("one package has representation-TCB change")
    };
    let [representation_conflict] = representation_package.conflicts() else {
        panic!("one representation-TCB row is introduced")
    };
    assert_eq!(
        representation_conflict.kind(),
        PackageReviewCanonicalRowKind::RepresentationTcb
    );
    assert_eq!(
        representation_conflict.risk(),
        PackageReviewCanonicalRowRisk::AuditRecommended
    );
    assert!(!representation_conflict.is_blocking());
    assert!(matches!(
        representation_package.root_policy_decision(
            representation_conflict,
            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        ),
        Err(ReviewOnlyRootPolicyResolutionError::NonBlockingConflict { .. })
    ));
    let nonblocking_record = format!(
        "OMEGA_PACKAGE_ROOT_POLICY_RESOLUTION_V1\n\
candidate_closure {}\n\
decision_count 1\n\
decision {} accept_candidate_change\n\
resolution_commitment {}\n\
end_root_policy_resolution\n",
        hex_digest(representation_package.candidate_closure().digest()),
        hex_digest(representation_conflict.fingerprint().digest()),
        "0".repeat(64),
    );
    assert!(matches!(
        recover_review_only_root_policy_resolution(
            &representation_conflicts,
            nonblocking_record.as_bytes(),
            record_limits
        ),
        Err(ReviewOnlyRootPolicyRecordError::Resolution(
            ReviewOnlyRootPolicyResolutionError::NonBlockingConflict { .. }
        ))
    ));
    assert_eq!(
        triage_review_update(
            &scenario.baseline_reviews,
            &representation_reviews,
            &BTreeSet::new()
        )
        .disposition(),
        PackageTriageDisposition::NoReviewBlockerWithAuditRecommended
    );

    write_package(
        &scenario.live,
        r#"pub boundary trait FilesystemHost { }

pub machine reserved_filesystem_authority()
reaches FilesystemHost
{
}
"#,
    );
    let dangerous_slack_sources = resolve_external_local_package_closure(
        &scenario.live,
        ExternalSourceContext::derive(b"capability-conflict-test-lock"),
        &scenario.dangerous_slack_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve dangerous-slack candidate");
    let dangerous_slack_reviews = compile_resolved_package_candidate_reviews(
        &dangerous_slack_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &scenario.build_root,
    )
    .expect("compile dangerous-slack review");
    let dangerous_slack_conflicts = compare_review_only_capabilities(
        &scenario.baseline_reviews,
        &dangerous_slack_reviews,
        &dangerous_slack_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare dangerous-slack row");
    let dangerous_slack_conflict = dangerous_slack_conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::DangerousAuthoritySlack)
        .unwrap_or_else(|| {
            panic!(
                "declared-but-unused dangerous authority produces an exact slack conflict; got {:?}",
                dangerous_slack_conflicts
                    .packages()
                    .iter()
                    .flat_map(|package| package.conflicts())
                    .map(|conflict| conflict.kind())
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        dangerous_slack_conflict.change(),
        ReviewOnlyCapabilityConflictChange::Added
    );
    assert_eq!(
        dangerous_slack_conflict.risk(),
        PackageReviewCanonicalRowRisk::AuditRecommended
    );
    assert!(!dangerous_slack_conflict.is_blocking());
    let slack_locations = dangerous_slack_conflict
        .candidate_source()
        .and_then(PackageReviewCanonicalRowSource::authored_locations)
        .expect("dangerous slack has authority and callable provenance");
    assert!(slack_locations.iter().any(|location| {
        location.role()
            == omega_package_evidence::record::PackageReviewSourceLocationRole::AuthorityDeclaration
    }));
    assert!(slack_locations.iter().any(|location| {
        location.role()
            == omega_package_evidence::record::PackageReviewSourceLocationRole::AuthorityExposure
            && location.relative_path() == "main.omg"
    }));
    let slack_triage = triage_review_update(
        &scenario.baseline_reviews,
        &dangerous_slack_reviews,
        &BTreeSet::new(),
    );
    assert!(slack_triage.decisions().iter().any(|decision| {
        decision
            .reasons()
            .contains(&PackageTriageReason::DangerousAuthoritySlack(
                omega_package_evidence::record::PackageReviewDangerousAuthorityClass::Filesystem,
            ))
    }));
}
