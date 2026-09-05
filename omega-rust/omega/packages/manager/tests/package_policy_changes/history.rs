use super::*;
#[path = "history/codec.rs"]
mod codec;
use package_manager::lock::{
    HistoricalPackagePolicyDecisionSubject as HistoricalSubject,
    HistoricalPackagePolicyError as Error,
};
use package_manager::review::{
    PackagePolicyChangeSet, PackagePolicyDecision, PackagePolicyDecisionSubject as Subject,
    PackagePolicyResolution, ReviewOnlyRootPolicyDisposition::*, resolve_package_policy_decisions,
};

const ASSUMPTIONS: &str = concat!(
    "pub const VALUE: u64 = 7;\n",
    "boundary machine trusted_zero() -> u64 ensures result == 0;\n",
    "boundary machine trusted_one() -> u64 ensures result == 1;\n",
);

fn hex(digest: [u8; 32]) -> String {
    use std::fmt::Write;
    digest.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").unwrap();
        text
    })
}

fn subject(closure: &ResolvedPackageSourceClosure) -> CanonicalSourceClosureSubject {
    CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap()
}

fn compare(
    baseline: Option<&PackageLockTarget>,
    closure: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
) -> PackagePolicyChangeSet {
    compare_package_policy_changes(
        baseline,
        reviews,
        &closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap()
}

fn choices(changes: &PackagePolicyChangeSet) -> Vec<PackagePolicyDecision> {
    let mut subjects = Vec::new();
    if changes.root_role_change().is_some() {
        subjects.push(Subject::RootRole);
    }
    subjects.extend(
        changes
            .source_replacements()
            .iter()
            .map(|replacement| Subject::SourceReplacement(replacement.fingerprint().digest())),
    );
    subjects.extend(
        changes
            .packages()
            .iter()
            .flat_map(|package| package.rows())
            .filter(|row| row.requires_decision())
            .map(|row| Subject::Row(row.fingerprint().digest())),
    );
    subjects.sort();
    subjects
        .into_iter()
        .map(|subject| PackagePolicyDecision {
            subject,
            disposition: AcceptCandidateChange,
        })
        .collect()
}

fn resolve_choices(
    changes: &PackagePolicyChangeSet,
    choices: &[PackagePolicyDecision],
) -> PackagePolicyResolution {
    resolve_package_policy_decisions(changes, changes.fingerprint().digest(), choices).unwrap()
}

fn capture(
    source: &CanonicalSourceClosureSubject,
    changes: &PackagePolicyChangeSet,
    resolution: &PackagePolicyResolution,
) -> HistoricalPackagePolicyDecisions {
    let history = HistoricalPackagePolicyDecisions::capture_policy(
        source,
        changes,
        resolution,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    assert_eq!(history.source_subject(), source.fingerprint());
    assert_eq!(history.comparison(), Some(changes.fingerprint().digest()));
    assert_eq!(
        history.baseline_source_subject(),
        changes
            .baseline_source_subject()
            .map(|value| *value.as_bytes())
    );
    assert_eq!(history.decisions().len(), resolution.decisions().len());
    for (retained, current) in history.decisions().iter().zip(resolution.decisions()) {
        let expected = match current.subject {
            Subject::RootRole => HistoricalSubject::RootRole,
            Subject::SourceReplacement(digest) => HistoricalSubject::SourceReplacement(digest),
            Subject::Row(digest) => HistoricalSubject::Row(digest),
        };
        assert_eq!(retained.subject(), expected);
        assert_eq!(retained.package_index(), None);
        assert_eq!(retained.conflict(), None);
        assert_eq!(retained.disposition(), current.disposition);
    }
    history
}

fn roundtrip_lock(
    source: CanonicalSourceClosureSubject,
    reviews: &CompilerIssuedPackageReviewSet,
    history: HistoricalPackagePolicyDecisions,
) -> PackageLock {
    let baselines = source
        .packages()
        .iter()
        .map(|package| reviews.review(package.key()).unwrap().policy().clone())
        .collect();
    let original = PackageLock::from_targets(vec![
        PackageLockTarget::from_parts(source, baselines, history).unwrap(),
    ])
    .unwrap();
    let text = original.canonical_text().unwrap();
    let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert_eq!(recovered, original);
    assert_eq!(recovered.canonical_text().unwrap(), text);
    recovered
}

#[test]
fn pure_initial_capture_keeps_empty_v2_context_through_the_full_lock() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (closure, reviews) = candidate(&tree, "pure-history");
    let source = subject(&closure);
    let changes = compare(None, &closure, &reviews);
    let resolution = resolve_choices(&changes, &[]);
    assert!(resolution.all_required_changes_accepted());
    let history = capture(&source, &changes, &resolution);
    assert!(history.decisions().is_empty());
    let text = history
        .canonical_text(&source, HistoricalPackagePolicyLimits::default())
        .unwrap();
    assert_eq!(
        text,
        format!(
            "omega-policy-decisions 2\nsource {}\nbaseline none\ncomparison {}\ndecisions 0\nend\n",
            source.fingerprint().to_hex(),
            hex(changes.fingerprint().digest()),
        )
    );
    let (recovered, usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
        &text,
        &source,
        HistoricalPackagePolicyLimits::new(text.len(), 0),
        0,
    )
    .unwrap();
    assert_eq!(recovered, history);
    assert_eq!(usage.owned_bytes(), 0);
    assert_eq!(usage.decisions(), 0);
    roundtrip_lock(source, &reviews, history);
}

#[test]
fn assumption_acceptance_and_each_retained_rejection_survive_full_lock_recovery() {
    let tree = Tree::new();
    source(&tree, ASSUMPTIONS, "");
    let (closure, reviews) = candidate(&tree, "assumption-history");
    let source = subject(&closure);
    let changes = compare(None, &closure, &reviews);
    let accepted = choices(&changes);
    assert!(accepted.len() >= 2);
    for rejected in 0..=accepted.len() {
        let mut decisions = accepted.clone();
        if rejected < decisions.len() {
            decisions[rejected].disposition = RejectCandidateChange;
        }
        decisions.reverse();
        let resolution = resolve_choices(&changes, &decisions);
        assert_eq!(
            resolution.all_required_changes_accepted(),
            rejected == accepted.len()
        );
        let history = capture(&source, &changes, &resolution);
        let lock = roundtrip_lock(source.clone(), &reviews, history.clone());
        assert_eq!(lock.target(TARGET).unwrap().decisions(), &history);
        assert_eq!(
            history
                .decisions()
                .iter()
                .filter(|decision| { decision.disposition() == RejectCandidateChange })
                .count(),
            usize::from(rejected < accepted.len())
        );
        assert!(
            history
                .decisions()
                .iter()
                .all(|decision| { matches!(decision.subject(), HistoricalSubject::Row(_)) })
        );
    }
}

#[test]
fn removed_package_history_needs_no_candidate_index_old_checkout_or_cache() {
    let tree = Tree::new();
    source(
        &tree,
        "pub const VALUE: u64 = 7;\n",
        " builder.depend_as(\"dependency\", Source::Path { location: \"../old\" });\n",
    );
    package(&tree.path("sources/old"), "removed-package", "");
    let baseline = {
        let (closure, reviews) = candidate(&tree, "accepted-history");
        lock_from_reviews(&closure, &reviews)
    };
    fs::rename(tree.path("sources/old"), tree.path("unavailable-old")).unwrap();
    fs::rename(
        tree.path("accepted-history-cache"),
        tree.path("unavailable-cache"),
    )
    .unwrap();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (closure, reviews) = candidate(&tree, "removed-history");
    let source = subject(&closure);
    let changes = compare(baseline.target(TARGET), &closure, &reviews);
    let removed = changes
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "removed-package")
        .unwrap();
    assert!(removed.candidate_resolution().is_none());
    assert!(
        source
            .packages()
            .iter()
            .all(|package| package.key() != removed.key())
    );
    let row = removed
        .rows()
        .iter()
        .find(|row| row.requires_decision())
        .unwrap();
    assert_eq!(row.change(), PackagePolicyChangeKind::Removed);
    let removed_subject = Subject::Row(row.fingerprint().digest());
    let mut decisions = choices(&changes);
    decisions
        .iter_mut()
        .find(|decision| decision.subject == removed_subject)
        .unwrap()
        .disposition = RejectCandidateChange;
    let resolution = resolve_choices(&changes, &decisions);
    assert!(!resolution.all_required_changes_accepted());
    let history = capture(&source, &changes, &resolution);
    let expected_subject = HistoricalSubject::Row(row.fingerprint().digest());
    let lock = roundtrip_lock(source, &reviews, history);
    let text = lock.canonical_text().unwrap();
    drop((baseline, closure, reviews, changes, resolution, lock));
    assert!(!tree.path("sources/old").exists());
    assert!(!tree.path("accepted-history-cache").exists());
    let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    let retained = recovered.target(TARGET).unwrap().decisions();
    let decision = retained
        .decisions()
        .iter()
        .find(|decision| decision.subject() == expected_subject)
        .unwrap();
    assert_eq!(decision.disposition(), RejectCandidateChange);
    assert!(retained.baseline_source_subject().is_some());
    assert_eq!(recovered.canonical_text().unwrap(), text);
}

#[test]
fn root_role_replacements_and_rows_capture_in_canonical_subject_order() {
    let tree = Tree::new();
    let main = "data Main { }\nmachine Main::main(&mut self) { }\n";
    source(
        &tree,
        main,
        " builder.depend_as(\"service\", Source::Path { location: \"../old\" });\n",
    );
    package(&tree.path("sources/old"), "old-service", "");
    let baseline = {
        let (closure, reviews) = candidate(&tree, "role-baseline");
        lock_from_reviews(&closure, &reviews)
    };
    package(&tree.path("sources/new"), "new-service", "");
    fs::write(
        tree.path("sources/root/build.omg"),
        concat!(
            "machine build(builder: &mut Build) {\n",
            " builder.application(\"policy-fixture\");\n",
            " builder.depend_as(\"service\", Source::Path { location: \"../new\" });\n",
            " builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);\n}\n",
        ),
    )
    .unwrap();
    let (closure, reviews) = candidate(&tree, "role-history");
    let source = subject(&closure);
    let changes = compare(baseline.target(TARGET), &closure, &reviews);
    assert!(changes.root_role_change().is_some());
    assert_eq!(changes.source_replacements().len(), 1);
    let mut decisions = choices(&changes);
    assert_eq!(decisions[0].subject, Subject::RootRole);
    assert!(matches!(
        decisions[1].subject,
        Subject::SourceReplacement(_)
    ));
    assert!(matches!(decisions.last().unwrap().subject, Subject::Row(_)));
    decisions[0].disposition = RejectCandidateChange;
    decisions[1].disposition = RejectCandidateChange;
    decisions.reverse();
    let resolution = resolve_choices(&changes, &decisions);
    let history = capture(&source, &changes, &resolution);
    let text = history
        .canonical_text(&source, HistoricalPackagePolicyLimits::default())
        .unwrap();
    let lines: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("decision "))
        .collect();
    assert_eq!(lines[0], "decision root-role reject");
    assert_eq!(
        lines[1],
        format!(
            "decision source-replacement {} reject",
            hex(changes.source_replacements()[0].fingerprint().digest())
        )
    );
    assert!(
        lines[2..]
            .iter()
            .all(|line| line.starts_with("decision row "))
    );
    roundtrip_lock(source, &reviews, history);
}

#[test]
fn capture_rejects_foreign_sources_and_whole_comparisons_even_with_identical_choices() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (closure, reviews) = candidate(&tree, "context-history");
    let current = subject(&closure);
    let baseline = lock_from_reviews(&closure, &reviews);
    let initial = compare(None, &closure, &reviews);
    let unchanged = compare(baseline.target(TARGET), &closure, &reviews);
    let initial_resolution = resolve_choices(&initial, &[]);
    let unchanged_resolution = resolve_choices(&unchanged, &[]);
    assert_eq!(
        initial_resolution.decisions(),
        unchanged_resolution.decisions()
    );
    assert_ne!(initial.fingerprint(), unchanged.fingerprint());
    let limits = HistoricalPackagePolicyLimits::default();
    for (changes, resolution) in [
        (&initial, &unchanged_resolution),
        (&unchanged, &initial_resolution),
    ] {
        assert_eq!(
            HistoricalPackagePolicyDecisions::capture_policy(&current, changes, resolution, limits),
            Err(Error::ResolutionMismatch)
        );
    }
    let history = capture(&current, &initial, &initial_resolution);
    let text = history.canonical_text(&current, limits).unwrap();
    source(
        &tree,
        "// source-only edit\npub const VALUE: u64 = 7;\n",
        "",
    );
    let (updated_closure, updated_reviews) = candidate(&tree, "updated-history");
    let updated = compare(None, &updated_closure, &updated_reviews);
    let updated_resolution = resolve_choices(&updated, &[]);
    assert_eq!(
        updated_resolution.decisions(),
        initial_resolution.decisions()
    );
    let updated_subject = subject(&updated_closure);
    assert_eq!(
        HistoricalPackagePolicyDecisions::capture_policy(
            &updated_subject,
            &updated,
            &initial_resolution,
            limits
        ),
        Err(Error::ResolutionMismatch)
    );
    let other = Tree::new();
    source(&other, "pub const VALUE: u64 = 7;\n", "");
    let foreign_closure = resolve(&other, "foreign-history");
    let foreign_target = CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(TargetProfile::LinuxArm64),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    for foreign in [updated_subject, subject(&foreign_closure), foreign_target] {
        assert_ne!(foreign.fingerprint(), current.fingerprint());
        assert_eq!(
            HistoricalPackagePolicyDecisions::capture_policy(
                &foreign,
                &initial,
                &initial_resolution,
                limits
            ),
            Err(Error::SourceSubjectMismatch)
        );
        assert_eq!(
            HistoricalPackagePolicyDecisions::recover_text(&text, &foreign, limits),
            Err(Error::SourceSubjectMismatch)
        );
        assert_eq!(
            history.canonical_text(&foreign, limits),
            Err(Error::SourceSubjectMismatch)
        );
    }
}
