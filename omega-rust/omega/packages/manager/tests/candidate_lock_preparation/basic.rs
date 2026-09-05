use super::*;

#[test]
fn pure_candidate_prepares_complete_recoverable_policy_with_explicit_empty_decisions() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (sources, reviews) = candidate(&tree, "pure");
    let changes = compare(None, &sources, &reviews);
    let resolved = resolution(&changes, ACCEPT);
    assert!(resolved.decisions().is_empty());
    let expected = reviews.reviews()[0].policy().clone();
    let target = prepare_candidate_lock_target(
        None,
        &sources.for_exact_target(TARGET),
        reviews.clone(),
        &resolved,
        PrepareCandidateLockLimits::default(),
    )
    .unwrap();
    assert_eq!(target.baselines(), &[expected]);
    assert_eq!(target.decisions().version(), 2);
    assert!(target.decisions().decisions().is_empty());
    let lock = PackageLock::from_targets(vec![target]).unwrap();
    let text = lock.canonical_text().unwrap();
    assert_eq!(
        PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap(),
        lock
    );
    for limits in [
        PrepareCandidateLockLimits {
            maximum_history_owned_bytes: 0,
            ..Default::default()
        },
        PrepareCandidateLockLimits {
            comparison: PackagePolicyChangeLimits {
                maximum_packages: 0,
                ..Default::default()
            },
            ..Default::default()
        },
        PrepareCandidateLockLimits {
            source: CanonicalSourceClosureSubjectLimits {
                maximum_record_bytes: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    ] {
        assert!(
            prepare_candidate_lock_target(
                None,
                &sources.for_exact_target(TARGET),
                reviews.clone(),
                &resolved,
                limits,
            )
            .is_err()
        );
    }
}

#[test]
fn exact_claim_decisions_govern_preparation_but_unchanged_accepted_claims_need_no_reapproval() {
    let tree = Tree::new();
    source(
        &tree,
        "boundary machine accepted_value() -> u64 ensures result == 7;\n",
        "",
    );
    let (sources, reviews) = candidate(&tree, "claim");
    let changes = compare(None, &sources, &reviews);
    let accepted = resolution(&changes, ACCEPT);
    let rejected = resolution(&changes, REJECT);
    assert!(!accepted.decisions().is_empty());
    assert!(matches!(
        prepare_candidate_lock_target(
            None,
            &sources.for_exact_target(TARGET),
            reviews.clone(),
            &rejected,
            PrepareCandidateLockLimits::default(),
        ),
        Err(PrepareCandidateLockError::RejectedDecision)
    ));
    let target = prepare_candidate_lock_target(
        None,
        &sources.for_exact_target(TARGET),
        reviews.clone(),
        &accepted,
        PrepareCandidateLockLimits::default(),
    )
    .unwrap();
    let old_text = PackageLock::from_targets(vec![target.clone()])
        .unwrap()
        .canonical_text()
        .unwrap();
    let unchanged = compare(Some(&target), &sources, &reviews);
    let resolved = resolution(&unchanged, ACCEPT);
    assert!(resolved.decisions().is_empty());
    assert!(
        prepare_candidate_lock_target(
            Some(&target),
            &sources.for_exact_target(TARGET),
            reviews.clone(),
            &accepted,
            PrepareCandidateLockLimits::default(),
        )
        .is_err()
    );
    let updated = prepare_candidate_lock_target(
        Some(&target),
        &sources.for_exact_target(TARGET),
        reviews,
        &resolved,
        PrepareCandidateLockLimits::default(),
    )
    .unwrap();
    assert_eq!(updated.baselines(), target.baselines());
    assert!(updated.decisions().decisions().is_empty());
    assert_eq!(
        PackageLock::from_targets(vec![target])
            .unwrap()
            .canonical_text()
            .unwrap(),
        old_text
    );
}

#[test]
fn source_only_drift_and_wrong_target_cannot_reuse_a_current_decision_resolution() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (sources, reviews) = candidate(&tree, "before");
    let resolved = resolution(&compare(None, &sources, &reviews), ACCEPT);
    assert!(
        prepare_candidate_lock_target(
            None,
            &sources.for_exact_target(TargetProfile::WindowsX64),
            reviews.clone(),
            &resolved,
            PrepareCandidateLockLimits::default(),
        )
        .is_err()
    );
    source(&tree, "// changed source\npub const VALUE: u64 = 7;\n", "");
    let (next_sources, next_reviews) = candidate(&tree, "after");
    assert_eq!(
        reviews.reviews()[0].policy(),
        next_reviews.reviews()[0].policy()
    );
    assert!(
        prepare_candidate_lock_target(
            None,
            &next_sources.for_exact_target(TARGET),
            next_reviews,
            &resolved,
            PrepareCandidateLockLimits::default(),
        )
        .is_err()
    );
}
