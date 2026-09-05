use super::*;
use omega_package_manager::lock::{
    HistoricalPackagePolicyDecisionSubject, HistoricalPackagePolicyError,
};

#[test]
fn removed_package_choices_roundtrip_without_old_source_and_never_become_acceptance() {
    let tree = Tree::new();
    source(
        &tree,
        "pub const VALUE: u64 = 7;\n",
        " builder.depend_as(\"selected\", Source::Path { location: \"../old\" });\n",
    );
    package(&tree.path("sources/old"), "same-name", "");
    fs::write(
        tree.path("sources/old/main.omg"),
        concat!(
            "pub const FIRST: u64 = 1;\n",
            "pub const SECOND: u64 = 2;\n",
            "pub const THIRD: u64 = 3;\n",
        ),
    )
    .unwrap();
    let accepted = {
        let (sources, reviews) = candidate(&tree, "old");
        lock_from_reviews(&sources, &reviews)
    };
    let old_key = accepted
        .target(TARGET)
        .unwrap()
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "same-name")
        .unwrap()
        .key()
        .clone();
    fs::rename(tree.path("sources/old"), tree.path("unavailable-old")).unwrap();
    fs::rename(tree.path("old-cache"), tree.path("unavailable-cache")).unwrap();
    source(
        &tree,
        "pub const VALUE: u64 = 7;\n",
        " builder.depend_as(\"selected\", Source::Path { location: \"../new\" });\n",
    );
    package(&tree.path("sources/new"), "same-name", "");
    let (sources, reviews) = candidate(&tree, "new");
    let changes = compare(accepted.target(TARGET), &sources, &reviews);
    for disposition in [ACCEPT, REJECT] {
        let resolved = resolution(&changes, disposition);
        assert_eq!(
            resolved.all_required_changes_accepted(),
            disposition == ACCEPT
        );
        let lock = history_lock(&sources, &reviews, &changes, &resolved);
        let target = lock.target(TARGET).unwrap();
        let history = target.decisions();
        assert!(!history.decisions().is_empty());
        assert!(
            history
                .decisions()
                .iter()
                .all(|decision| decision.disposition() == disposition)
        );
        let removed = history.decisions().iter().filter(|decision| matches!(
            decision.subject(), HistoricalPackagePolicyDecisionSubject::RemovedPackage { key } if key == &old_key
        )).collect::<Vec<_>>();
        assert!(!removed.is_empty());
        assert!(
            removed
                .iter()
                .all(|decision| decision.package_index().is_none())
        );
        assert!(
            !target
                .source()
                .packages()
                .iter()
                .any(|source| source.key() == &old_key)
        );
        let replacement = target
            .source()
            .packages()
            .iter()
            .find(|source| source.key().name().as_str() == "same-name")
            .unwrap();
        assert_ne!(replacement.key(), &old_key);
        let text = history
            .canonical_text(target.source(), HistoricalPackagePolicyLimits::default())
            .unwrap();
        let fragment_lengths = text
            .lines()
            .filter_map(|line| line.strip_prefix("decision removed "))
            .map(|line| line.split_once(' ').unwrap().0.parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        assert!(fragment_lengths.len() >= 3);
        assert!(
            fragment_lengths
                .iter()
                .all(|length| *length == fragment_lengths[0])
        );
        let maximum_bytes = fragment_lengths[0] + 1;
        assert!(maximum_bytes < fragment_lengths.iter().sum::<usize>());
        // Each removed API choice repeats this exact inert key fragment. A
        // per-fragment limit cannot replace the aggregate capture limit.
        assert_eq!(
            HistoricalPackagePolicyDecisions::capture_policy_changes(
                target.source(),
                &changes,
                &resolved,
                HistoricalPackagePolicyLimits::new(maximum_bytes, history.decisions().len()),
            )
            .unwrap_err(),
            HistoricalPackagePolicyError::ByteLimitExceeded
        );
        let (recovered, usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
            &text,
            target.source(),
            HistoricalPackagePolicyLimits::default(),
            usize::MAX,
        )
        .unwrap();
        assert_eq!(&recovered, history);
        assert_eq!(usage.decisions(), history.decisions().len());
        assert!(usage.owned_bytes() > 0);
        assert!(
            HistoricalPackagePolicyDecisions::recover_text_with_usage(
                &text,
                target.source(),
                HistoricalPackagePolicyLimits::default(),
                usage.owned_bytes(),
            )
            .is_ok()
        );
        assert_eq!(
            HistoricalPackagePolicyDecisions::recover_text_with_usage(
                &text,
                target.source(),
                HistoricalPackagePolicyLimits::default(),
                usage.owned_bytes() - 1,
            )
            .unwrap_err(),
            HistoricalPackagePolicyError::AllocationLimitExceeded
        );
        // One enclosing allowance cannot be reset for a second history child.
        let remaining = usage.owned_bytes() * 2 - 1 - usage.owned_bytes();
        assert_eq!(
            HistoricalPackagePolicyDecisions::recover_text_with_usage(
                &text,
                target.source(),
                HistoricalPackagePolicyLimits::default(),
                remaining,
            )
            .unwrap_err(),
            HistoricalPackagePolicyError::AllocationLimitExceeded
        );
        for malformed in [
            format!("{text}trailing\n"),
            text[..text.len() - 1].to_owned(),
            text.replacen(
                "omega-policy-decisions 2\n",
                "omega-policy-decisions 02\n",
                1,
            ),
        ] {
            assert_ne!(malformed, text);
            assert!(
                HistoricalPackagePolicyDecisions::recover_text(
                    &malformed,
                    target.source(),
                    HistoricalPackagePolicyLimits::default(),
                )
                .is_err()
            );
        }
        if disposition == ACCEPT {
            let lock_text = lock.canonical_text().unwrap();
            let (mut lower, mut upper) =
                (0, PackageLockRecoveryLimits::default().maximum_owned_bytes);
            while lower < upper {
                let middle = lower + (upper - lower) / 2;
                if PackageLock::recover_text(
                    &lock_text,
                    PackageLockRecoveryLimits {
                        maximum_owned_bytes: middle,
                        ..Default::default()
                    },
                )
                .is_ok()
                {
                    upper = middle;
                } else {
                    lower = middle + 1;
                }
            }
            assert!(
                lower > usage.owned_bytes(),
                "source and policy children share the history budget"
            );
            let exact = PackageLockRecoveryLimits {
                maximum_owned_bytes: lower,
                ..Default::default()
            };
            assert!(PackageLock::recover_text(&lock_text, exact).is_ok());
            let short = PackageLockRecoveryLimits {
                maximum_owned_bytes: lower - 1,
                ..Default::default()
            };
            assert!(PackageLock::recover_text(&lock_text, short).is_err());
            let reader_minimum = lower;
            // Writing additionally owns its fragments/output and validation
            // temporaries, so its shared allowance need not equal reading's.
            let (mut lower, mut upper) =
                (0, PackageLockRecoveryLimits::default().maximum_owned_bytes);
            while lower < upper {
                let middle = lower + (upper - lower) / 2;
                if lock
                    .canonical_text_with_limits(PackageLockRecoveryLimits {
                        maximum_owned_bytes: middle,
                        ..Default::default()
                    })
                    .is_ok()
                {
                    upper = middle;
                } else {
                    lower = middle + 1;
                }
            }
            assert!(lower >= reader_minimum);
            let exact = PackageLockRecoveryLimits {
                maximum_owned_bytes: lower,
                ..Default::default()
            };
            assert_eq!(lock.canonical_text_with_limits(exact).unwrap(), lock_text);
            let short = PackageLockRecoveryLimits {
                maximum_owned_bytes: lower - 1,
                ..Default::default()
            };
            assert!(lock.canonical_text_with_limits(short).is_err());
        }
    }
}

#[test]
fn legacy_history_keeps_exact_bytes_and_new_empty_resolution_has_explicit_v2_history() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (sources, reviews) = candidate(&tree, "pure");
    let legacy = lock_from_reviews(&sources, &reviews);
    let target = legacy.target(TARGET).unwrap();
    let text = format!(
        "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
        target.source().fingerprint().to_hex()
    );
    assert_eq!(target.decisions().version(), 1);
    assert_eq!(target.decisions().comparison(), None);
    assert_eq!(
        target
            .decisions()
            .canonical_text(target.source(), HistoricalPackagePolicyLimits::default())
            .unwrap(),
        text
    );
    let recovered = HistoricalPackagePolicyDecisions::recover_text(
        &text,
        target.source(),
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    assert_eq!(
        recovered
            .canonical_text(target.source(), HistoricalPackagePolicyLimits::default())
            .unwrap(),
        text
    );
    let nonempty = format!(
        "omega-policy-decisions 1\nsource {}\ndecisions 2\ndecision 0 {} accept\ndecision 0 {} reject\nend\n",
        target.source().fingerprint().to_hex(),
        "12".repeat(32),
        "23".repeat(32),
    );
    let recovered = HistoricalPackagePolicyDecisions::recover_text(
        &nonempty,
        target.source(),
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    assert_eq!(recovered.version(), 1);
    assert_eq!(recovered.decisions()[0].package_index(), Some(0));
    assert_eq!(recovered.decisions()[0].disposition(), ACCEPT);
    assert_eq!(recovered.decisions()[1].disposition(), REJECT);
    assert_eq!(
        recovered
            .canonical_text(target.source(), HistoricalPackagePolicyLimits::default())
            .unwrap(),
        nonempty
    );
    let changes = compare(None, &sources, &reviews);
    let resolved = resolution(&changes, ACCEPT);
    let current = history_lock(&sources, &reviews, &changes, &resolved);
    assert!(
        current
            .target(TARGET)
            .unwrap()
            .decisions()
            .decisions()
            .is_empty()
    );
    assert_eq!(current.target(TARGET).unwrap().decisions().version(), 2);
}
