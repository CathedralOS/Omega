use super::*;
use omega_package_evidence::encoding::PackagePolicyTextRecoveryLimits;
use omega_package_evidence::record::PackagePolicyBaseline;
use omega_package_manager::review::PackagePolicyDecisionError;

const CLAIMS: &str = concat!(
    "boundary machine first() -> u64 ensures result == 0;\n",
    "boundary machine second() -> u64 ensures result == 1;\n",
);

#[test]
fn pure_initial_policy_has_an_empty_resolution_while_every_assumption_needs_a_choice() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (sources, reviews) = candidate(&tree, "pure");
    let pure = compare(None, &sources, &reviews);
    let empty = resolution(&pure, ACCEPT);
    assert!(empty.decisions().is_empty());
    assert!(empty.all_required_changes_accepted());
    let package = &pure.packages()[0];
    let row = &package.rows()[0];
    let advisory = PackagePolicyDecision {
        subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
        disposition: ACCEPT,
    };
    assert!(
        resolve_package_policy_decisions(&pure, pure.fingerprint().digest(), &[advisory]).is_err()
    );

    source(&tree, CLAIMS, "");
    let (sources, reviews) = candidate(&tree, "claims");
    let changes = compare(None, &sources, &reviews);
    let selected = decisions(&changes, ACCEPT);
    assert_eq!(selected.len(), 2);
    let accepted = resolution(&changes, ACCEPT);
    let rejected = resolution(&changes, REJECT);
    assert!(accepted.all_required_changes_accepted());
    assert!(!rejected.all_required_changes_accepted());
    assert!(
        accepted
            .decisions()
            .iter()
            .all(|decision| decision.disposition == ACCEPT)
    );
    assert!(
        rejected
            .decisions()
            .iter()
            .all(|decision| decision.disposition == REJECT)
    );
    assert_ne!(accepted, rejected);
    let mut mixed = selected.clone();
    mixed[1].disposition = REJECT;
    let mixed =
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &mixed).unwrap();
    assert!(!mixed.all_required_changes_accepted());
    assert_eq!(
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &selected[..1],)
            .unwrap_err(),
        PackagePolicyDecisionError::MissingDecision(selected[1].subject)
    );
    let duplicate = vec![selected[0], selected[0]];
    assert_eq!(
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &duplicate,)
            .unwrap_err(),
        PackagePolicyDecisionError::DuplicateDecision(selected[0].subject)
    );
    assert!(
        resolve_package_policy_decisions(&pure, changes.fingerprint().digest(), &selected).is_err()
    );
    for limits in [
        PackagePolicyDecisionLimits {
            maximum_decisions: 1,
            ..Default::default()
        },
        PackagePolicyDecisionLimits {
            maximum_owned_bytes: 0,
            ..Default::default()
        },
        PackagePolicyDecisionLimits {
            maximum_changes: 0,
            ..Default::default()
        },
    ] {
        assert!(
            resolve_package_policy_decisions_with_limits(
                &changes,
                changes.fingerprint().digest(),
                &selected,
                limits
            )
            .is_err()
        );
    }
    let text = accepted
        .canonical_text(PackagePolicyDecisionLimits::default())
        .unwrap();
    let mut lines = text.lines();
    let impossible = format!(
        "{}\n{}\ndecisions 65536\n",
        lines.next().unwrap(),
        lines.next().unwrap(),
    );
    assert_eq!(
        recover_package_policy_decisions(
            &impossible,
            &changes,
            PackagePolicyDecisionLimits {
                maximum_owned_bytes: 0,
                ..Default::default()
            },
        )
        .unwrap_err(),
        PackagePolicyDecisionError::InvalidFraming,
        "a truncated declared count must reject before allocating blocker/decision storage"
    );
    for malformed in [
        text.replacen("decisions 2\n", "decisions 02\n", 1),
        text.replacen("accept_candidate_change", "approve", 1),
        format!("{text}trailing\n"),
        text[..text.len() - 1].to_owned(),
    ] {
        assert_ne!(malformed, text);
        assert!(
            recover_package_policy_decisions(
                &malformed,
                &changes,
                PackagePolicyDecisionLimits::default(),
            )
            .is_err()
        );
    }
    assert!(
        recover_package_policy_decisions(
            &text,
            &changes,
            PackagePolicyDecisionLimits {
                maximum_bytes: text.len() - 1,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn baseline_only_and_source_only_changes_make_old_decisions_stale() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 1;\n", "");
    let accepted = {
        let sources = resolve(&tree, "old");
        let (accepted, _) = capture_lock(&sources, &tree.path("old-build"));
        assert_fresh_matches(&accepted, &sources);
        accepted
    };
    source(&tree, "pub const VALUE: u64 = 2;\n", "");
    let (sources, reviews) = candidate(&tree, "current");
    let changes = compare(accepted.target(TARGET), &sources, &reviews);
    let accepted_resolution = resolution(&changes, ACCEPT);
    let text = accepted_resolution
        .canonical_text(PackagePolicyDecisionLimits::default())
        .unwrap();
    let previous = accepted.target(TARGET).unwrap();
    let mut baselines = previous.baselines().to_vec();
    let baseline_text = baselines[0].canonical_text().unwrap();
    let scalar = "string \"VALUE\"\n";
    assert_eq!(baseline_text.matches(scalar).count(), 1);
    baselines[0] = PackagePolicyBaseline::recover_text(
        &baseline_text.replace(scalar, "string \"PREVIOUS_VALUE\"\n"),
        PackagePolicyTextRecoveryLimits::default(),
    )
    .unwrap();
    let different_baseline = PackageLockTarget::from_parts(
        previous.source().clone(),
        baselines,
        previous.decisions().clone(),
    )
    .unwrap();
    let changed_baseline = compare(Some(&different_baseline), &sources, &reviews);
    assert_eq!(
        changes.candidate_source_subject(),
        changed_baseline.candidate_source_subject()
    );
    assert!(
        recover_package_policy_decisions(
            &text,
            &changed_baseline,
            PackagePolicyDecisionLimits::default()
        )
        .is_err()
    );

    source(
        &tree,
        "// source-only drift\npub const VALUE: u64 = 2;\n",
        "",
    );
    let (next_sources, next_reviews) = candidate(&tree, "next");
    assert_eq!(
        reviews.reviews()[0].policy(),
        next_reviews.reviews()[0].policy()
    );
    let changed_source = compare(accepted.target(TARGET), &next_sources, &next_reviews);
    assert!(
        recover_package_policy_decisions(
            &text,
            &changed_source,
            PackagePolicyDecisionLimits::default()
        )
        .is_err()
    );
    assert!(
        resolve_package_policy_decisions(
            &changed_source,
            changes.fingerprint().digest(),
            accepted_resolution.decisions(),
        )
        .is_err()
    );
}
