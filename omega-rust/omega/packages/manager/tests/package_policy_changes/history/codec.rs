use super::*;

fn authored_text(source: &CanonicalSourceClosureSubject, rows: &[String]) -> String {
    format!(
        "omega-policy-decisions 2\nsource {}\nbaseline {}\ncomparison {}\ndecisions {}\n{}end\n",
        source.fingerprint().to_hex(),
        "ab".repeat(32),
        "cd".repeat(32),
        rows.len(),
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>(),
    )
}

#[test]
fn recovery_trusts_edited_choices_and_context_without_historical_row_payloads() {
    let tree = Tree::new();
    source(&tree, ASSUMPTIONS, "");
    let (closure, reviews) = candidate(&tree, "edited-history");
    let source = subject(&closure);
    let changes = compare(None, &closure, &reviews);
    let history = capture(
        &source,
        &changes,
        &resolve_choices(&changes, &choices(&changes)),
    );
    let limits = HistoricalPackagePolicyLimits::default();
    let original = history.canonical_text(&source, limits).unwrap();
    let edited = original
        .replacen(" accept\n", " reject\n", 1)
        .replacen(
            "baseline none\n",
            &format!("baseline {}\n", "ab".repeat(32)),
            1,
        )
        .replacen(
            &format!("comparison {}\n", hex(changes.fingerprint().digest())),
            &format!("comparison {}\n", "cd".repeat(32)),
            1,
        );
    let recovered =
        HistoricalPackagePolicyDecisions::recover_text(&edited, &source, limits).unwrap();
    assert_eq!(recovered.baseline_source_subject(), Some([0xab; 32]));
    assert_eq!(recovered.comparison(), Some([0xcd; 32]));
    assert_eq!(
        recovered.decisions()[0].disposition(),
        RejectCandidateChange
    );
    assert_eq!(recovered.canonical_text(&source, limits).unwrap(), edited);
    roundtrip_lock(source.clone(), &reviews, recovered);
    // A digest is an inert subject, not proof that a historical row existed.
    let authored = authored_text(
        &source,
        &[
            "decision root-role reject".into(),
            format!("decision source-replacement {} accept", "12".repeat(32)),
            format!("decision row {} reject", "34".repeat(32)),
        ],
    );
    let recovered =
        HistoricalPackagePolicyDecisions::recover_text(&authored, &source, limits).unwrap();
    assert_eq!(
        recovered.decisions()[2].subject(),
        HistoricalSubject::Row([0x34; 32])
    );
    assert_eq!(recovered.canonical_text(&source, limits).unwrap(), authored);
    roundtrip_lock(source, &reviews, recovered);
}

#[test]
fn v1_history_keeps_legacy_subjects_and_canonical_bytes() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (closure, reviews) = candidate(&tree, "legacy-history");
    let source = subject(&closure);
    for rows in [
        String::new(),
        format!("decision 0 {} reject\n", "12".repeat(32)),
    ] {
        let text = format!(
            "omega-policy-decisions 1\nsource {}\ndecisions {}\n{rows}end\n",
            source.fingerprint().to_hex(),
            usize::from(!rows.is_empty())
        );
        let limits = HistoricalPackagePolicyLimits::default();
        let history =
            HistoricalPackagePolicyDecisions::recover_text(&text, &source, limits).unwrap();
        assert_eq!(history.comparison(), None);
        assert_eq!(history.baseline_source_subject(), None);
        if !rows.is_empty() {
            assert_eq!(
                history.decisions()[0].subject(),
                HistoricalSubject::LegacyConflict {
                    package_index: 0,
                    conflict: [0x12; 32],
                }
            );
            assert_eq!(history.decisions()[0].disposition(), RejectCandidateChange);
        }
        assert_eq!(history.canonical_text(&source, limits).unwrap(), text);
        roundtrip_lock(source.clone(), &reviews, history);
    }
}

#[test]
fn v2_framing_rejects_unknown_misordered_duplicate_and_truncated_choices() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let source = subject(&resolve(&tree, "framing-history"));
    let rows = vec![
        "decision root-role reject".into(),
        format!("decision source-replacement {} accept", "12".repeat(32)),
        format!("decision source-replacement {} reject", "34".repeat(32)),
        format!("decision row {} accept", "56".repeat(32)),
        format!("decision row {} reject", "78".repeat(32)),
    ];
    let text = authored_text(&source, &rows);
    let limits = HistoricalPackagePolicyLimits::default();
    let recovered = HistoricalPackagePolicyDecisions::recover_text(&text, &source, limits).unwrap();
    assert_eq!(recovered.canonical_text(&source, limits).unwrap(), text);
    for length in 0..text.len() {
        assert!(
            HistoricalPackagePolicyDecisions::recover_text(&text[..length], &source, limits)
                .is_err(),
            "accepted truncation at {length}"
        );
    }
    for index in 0..rows.len() - 1 {
        let mut reordered = rows.clone();
        reordered.swap(index, index + 1);
        assert!(
            HistoricalPackagePolicyDecisions::recover_text(
                &authored_text(&source, &reordered),
                &source,
                limits
            )
            .is_err()
        );
    }
    for index in 0..rows.len() {
        let mut duplicate = rows.clone();
        duplicate.insert(index, rows[index].replace(" accept", " reject"));
        assert!(
            HistoricalPackagePolicyDecisions::recover_text(
                &authored_text(&source, &duplicate),
                &source,
                limits
            )
            .is_err()
        );
    }
    for (from, to) in [
        ("omega-policy-decisions 2", "omega-policy-decisions 99"),
        ("decisions 5\n", "decisions 05\n"),
        ("decisions 5\n", "decisions +5\n"),
        ("decisions 5\n", "decisions 4\n"),
        ("decisions 5\n", "decisions 6\n"),
        ("decisions 5\n", "decisions 184467440737095516160\n"),
        ("baseline ", "previous "),
        ("comparison ", "unknown "),
        ("decision root-role reject", "decision root-role 0 reject"),
        ("decision root-role", "decision unknown"),
        ("decision source-replacement ", "decision replacement "),
        ("decision row ", "decision 0 "),
        (" accept\n", " approve\n"),
        (" accept\n", " accept extra\n"),
        (" reject\n", "  reject\n"),
        ("end\n", "end\ntrailing\n"),
    ] {
        let malformed = text.replacen(from, to, 1);
        assert_ne!(malformed, text);
        assert!(
            HistoricalPackagePolicyDecisions::recover_text(&malformed, &source, limits).is_err(),
            "accepted {to:?}"
        );
    }
    for digest in [
        "ab".repeat(32),
        "cd".repeat(32),
        "12".repeat(32),
        "56".repeat(32),
    ] {
        for invalid in [
            "none".to_owned(),
            "AB".repeat(32),
            "gg".repeat(32),
            "0".repeat(63),
            "0".repeat(65),
        ] {
            if digest == "ab".repeat(32) && invalid == "none" {
                continue;
            }
            assert!(
                HistoricalPackagePolicyDecisions::recover_text(
                    &text.replacen(&digest, &invalid, 1),
                    &source,
                    limits
                )
                .is_err()
            );
        }
    }
    for malformed in [
        text.replace('\n', "\r\n"),
        format!("{text}\n"),
        text.replacen("decision row ", "decision\trow ", 1),
    ] {
        assert!(
            HistoricalPackagePolicyDecisions::recover_text(&malformed, &source, limits).is_err()
        );
    }
}

#[test]
fn v2_capture_codec_and_enclosing_lock_enforce_byte_count_and_storage_limits() {
    let tree = Tree::new();
    source(&tree, ASSUMPTIONS, "");
    let (closure, reviews) = candidate(&tree, "limited-history");
    let source = subject(&closure);
    let changes = compare(None, &closure, &reviews);
    let resolution = resolve_choices(&changes, &choices(&changes));
    let history = capture(&source, &changes, &resolution);
    let text = history
        .canonical_text(&source, HistoricalPackagePolicyLimits::default())
        .unwrap();
    let count = history.decisions().len();
    assert!(count >= 2);
    let exact = HistoricalPackagePolicyLimits::new(text.len(), count);
    assert_eq!(
        HistoricalPackagePolicyDecisions::capture_policy(&source, &changes, &resolution, exact)
            .unwrap(),
        history
    );
    assert_eq!(history.canonical_text(&source, exact).unwrap(), text);
    for (limits, error) in [
        (
            HistoricalPackagePolicyLimits::new(text.len() - 1, count),
            Error::ByteLimitExceeded,
        ),
        (
            HistoricalPackagePolicyLimits::new(text.len(), count - 1),
            Error::DecisionLimitExceeded,
        ),
    ] {
        assert_eq!(
            HistoricalPackagePolicyDecisions::capture_policy(
                &source,
                &changes,
                &resolution,
                limits
            ),
            Err(error)
        );
        assert_eq!(history.canonical_text(&source, limits), Err(error));
        assert_eq!(
            HistoricalPackagePolicyDecisions::recover_text(&text, &source, limits),
            Err(error)
        );
    }
    let (recovered, usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
        &text,
        &source,
        exact,
        usize::MAX,
    )
    .unwrap();
    assert_eq!(recovered, history);
    assert_eq!(usage.decisions(), count);
    assert_eq!(
        usage.owned_bytes(),
        std::mem::size_of_val(history.decisions())
    );
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text_with_usage(
            &text,
            &source,
            exact,
            usage.owned_bytes()
        )
        .unwrap(),
        (history.clone(), usage)
    );
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text_with_usage(
            &text,
            &source,
            exact,
            usage.owned_bytes() - 1
        ),
        Err(Error::AllocationLimitExceeded)
    );
    let raised = HistoricalPackagePolicyLimits::new(usize::MAX, usize::MAX);
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text(
            &text.replacen(&format!("\ndecisions {count}\n"), "\ndecisions 65537\n", 1),
            &source,
            raised
        ),
        Err(Error::DecisionLimitExceeded)
    );
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text(
            &" ".repeat(8 * 1024 * 1024 + 1),
            &source,
            raised
        ),
        Err(Error::ByteLimitExceeded)
    );
    let lock = roundtrip_lock(source, &reviews, history);
    let lock_text = lock.canonical_text().unwrap();
    let exact_lock = PackageLockRecoveryLimits {
        maximum_bytes: lock_text.len(),
        maximum_decisions: count,
        ..Default::default()
    };
    assert_eq!(
        lock.canonical_text_with_limits(exact_lock).unwrap(),
        lock_text
    );
    assert_eq!(
        PackageLock::recover_text(&lock_text, exact_lock).unwrap(),
        lock
    );
    for limits in [
        PackageLockRecoveryLimits {
            maximum_bytes: lock_text.len() - 1,
            ..exact_lock
        },
        PackageLockRecoveryLimits {
            maximum_decisions: count - 1,
            ..exact_lock
        },
        PackageLockRecoveryLimits {
            maximum_owned_bytes: usage.owned_bytes() - 1,
            ..exact_lock
        },
    ] {
        assert!(lock.canonical_text_with_limits(limits).is_err());
        assert!(PackageLock::recover_text(&lock_text, limits).is_err());
    }
}
