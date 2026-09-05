use super::*;
use omega_package_evidence::encoding::PackagePolicyTextRecoveryLimits;
use omega_package_evidence::record::PackagePolicyBaseline;

#[test]
fn retained_rows_survive_old_source_loss_and_distinguish_replaced_same_name_packages() {
    let tree = Tree::new();
    source(
        &tree,
        "pub const CHANGED: u64 = 1;\npub const REMOVED: u64 = 1;\n",
        " builder.depend_as(\"dependency\", Source::Path { location: \"../old\" });\n",
    );
    package(&tree.path("sources/old"), "same-name", "");
    let lock = {
        let closure = resolve(&tree, "accepted");
        let (lock, _) = capture_lock(&closure, &tree.path("accepted-build"));
        assert_fresh_matches(&lock, &closure);
        lock
    };
    let accepted = lock.target(TARGET).unwrap();
    let old_key = accepted
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "same-name")
        .unwrap()
        .key()
        .clone();
    // Neither the removed checkout nor its old cache is available at its
    // recorded path. Only canonical accepted policy remains available here.
    fs::rename(tree.path("sources/old"), tree.path("unavailable-old")).unwrap();
    fs::rename(tree.path("accepted-cache"), tree.path("unavailable-cache")).unwrap();
    source(
        &tree,
        "pub const CHANGED: u64 = 2;\npub const ADDED: u64 = 1;\n",
        " builder.depend_as(\"dependency\", Source::Path { location: \"../new\" });\n",
    );
    package(&tree.path("sources/new"), "same-name", "");
    let (closure, reviews) = candidate(&tree, "candidate");
    let changes = compare_package_policy_changes(
        Some(accepted),
        &reviews,
        &closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert_eq!(changes.packages().len(), 3);
    assert!(changes.requires_decision());
    let repeated = compare_package_policy_changes(
        Some(accepted),
        &reviews,
        &closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert_eq!(changes.fingerprint(), repeated.fingerprint());
    for (first, second) in changes.packages().iter().zip(repeated.packages()) {
        assert_eq!(first.key(), second.key());
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.rows().len(), second.rows().len());
        for (first, second) in first.rows().iter().zip(second.rows()) {
            assert_eq!(first.key_bytes(), second.key_bytes());
            assert_eq!(first.fingerprint(), second.fingerprint());
        }
    }
    let root = changes
        .packages()
        .iter()
        .find(|package| package.key() == closure.graph().root())
        .unwrap();
    assert!(root.source_changed());
    assert!(root.audit_recommended());
    for (change, name) in [
        (PackagePolicyChangeKind::Added, "ADDED"),
        (PackagePolicyChangeKind::Changed, "CHANGED"),
        (PackagePolicyChangeKind::Removed, "REMOVED"),
    ] {
        let row = root
            .rows()
            .iter()
            .find(|row| row.kind() == PackagePolicyRowKind::PublicConst && row.change() == change)
            .unwrap();
        assert!(row.requires_decision());
        assert_eq!(
            row.baseline().is_some(),
            change != PackagePolicyChangeKind::Added
        );
        assert_eq!(
            row.candidate().is_some(),
            change != PackagePolicyChangeKind::Removed
        );
        assert!(
            row.baseline()
                .or(row.candidate())
                .unwrap()
                .canonical_text()
                .contains(&format!("string \"{name}\"\n"))
        );
    }
    let removed = changes
        .packages()
        .iter()
        .find(|package| package.key() == &old_key)
        .unwrap();
    assert!(removed.baseline_resolution().is_some());
    assert!(removed.candidate_resolution().is_none());
    assert!(removed.baseline_path().is_some());
    assert!(removed.candidate_path().is_none());
    assert!(!removed.rows().is_empty());
    assert!(
        removed
            .rows()
            .iter()
            .all(|row| row.change() == PackagePolicyChangeKind::Removed)
    );
    let added = changes
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "same-name" && package.key() != &old_key)
        .unwrap();
    assert!(added.baseline_resolution().is_none());
    assert!(added.candidate_resolution().is_some());
    assert!(added.baseline_path().is_none());
    assert!(added.candidate_path().is_some());
    assert!(
        added
            .rows()
            .iter()
            .all(|row| row.change() == PackagePolicyChangeKind::Added)
    );

    let first_fingerprint = changes.fingerprint().digest();
    fs::write(
        tree.path("sources/root/main.omg"),
        "// implementation source changed\npub const CHANGED: u64 = 2;\npub const ADDED: u64 = 1;\n",
    )
    .unwrap();
    let (next_sources, next_reviews) = candidate(&tree, "next");
    assert_eq!(
        reviews.review(closure.graph().root()).unwrap().policy(),
        next_reviews
            .review(next_sources.graph().root())
            .unwrap()
            .policy()
    );
    let next = compare_package_policy_changes(
        Some(accepted),
        &next_reviews,
        &next_sources.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert_ne!(first_fingerprint, next.fingerprint().digest());
    assert_ne!(
        changes.candidate_source_subject(),
        next.candidate_source_subject()
    );
    assert_eq!(
        changes.baseline_source_subject(),
        next.baseline_source_subject()
    );
    let mut altered_baselines = accepted.baselines().to_vec();
    let root_index = accepted
        .source()
        .packages()
        .iter()
        .position(|package| package.key() == closure.graph().root())
        .unwrap();
    let text = altered_baselines[root_index].canonical_text().unwrap();
    let original = "string \"REMOVED\"\n";
    assert_eq!(text.matches(original).count(), 1);
    altered_baselines[root_index] = PackagePolicyBaseline::recover_text(
        &text.replace(original, "string \"OTHER_REMOVED\"\n"),
        PackagePolicyTextRecoveryLimits::default(),
    )
    .unwrap();
    let altered_accepted = PackageLockTarget::from_parts(
        accepted.source().clone(),
        altered_baselines,
        accepted.decisions().clone(),
    )
    .unwrap();
    let changed_baseline = compare_package_policy_changes(
        Some(&altered_accepted),
        &reviews,
        &closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        changes.baseline_source_subject(),
        changed_baseline.baseline_source_subject()
    );
    assert_eq!(
        changes.candidate_source_subject(),
        changed_baseline.candidate_source_subject()
    );
    assert_ne!(changes.fingerprint(), changed_baseline.fingerprint());

    let alias_baseline = lock_from_reviews(&next_sources, &next_reviews);
    source(
        &tree,
        "// implementation source changed\npub const CHANGED: u64 = 2;\npub const ADDED: u64 = 1;\n",
        " builder.depend_as(\"renamed\", Source::Path { location: \"../new\" });\n",
    );
    let (alias_sources, alias_reviews) = candidate(&tree, "alias");
    let alias_changes = compare_package_policy_changes(
        alias_baseline.target(TARGET),
        &alias_reviews,
        &alias_sources.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert!(alias_changes.source_subject_changed());
    assert_ne!(
        alias_changes.baseline_source_subject().unwrap(),
        alias_changes.candidate_source_subject()
    );
    for package in alias_changes.packages() {
        assert!(
            package.rows().is_empty(),
            "alias spelling is source-graph meaning, not fabricated policy"
        );
        assert!(package.audit_recommended());
        if package.key() == alias_sources.graph().root() {
            assert!(package.source_changed());
        } else {
            assert!(!package.source_changed());
            assert!(package.source_association_changed());
            assert_eq!(
                package.baseline_resolution(),
                package.candidate_resolution()
            );
            assert_ne!(package.baseline_path(), package.candidate_path());
        }
    }

    for limits in [
        PackagePolicyChangeLimits {
            maximum_packages: 1,
            ..Default::default()
        },
        PackagePolicyChangeLimits {
            maximum_rows: 0,
            ..Default::default()
        },
        PackagePolicyChangeLimits {
            maximum_projection_owned_bytes: 0,
            ..Default::default()
        },
        PackagePolicyChangeLimits {
            maximum_changed_rows: 0,
            ..Default::default()
        },
        PackagePolicyChangeLimits {
            maximum_changed_owned_bytes: 0,
            ..Default::default()
        },
        PackagePolicyChangeLimits {
            maximum_projection_elements: 0,
            ..Default::default()
        },
        PackagePolicyChangeLimits {
            maximum_dependency_path_steps: 0,
            ..Default::default()
        },
        PackagePolicyChangeLimits {
            maximum_context_bytes: 0,
            ..Default::default()
        },
    ] {
        assert!(
            compare_package_policy_changes(
                Some(accepted),
                &reviews,
                &closure.for_exact_target(TARGET),
                limits,
            )
            .is_err()
        );
    }
}
