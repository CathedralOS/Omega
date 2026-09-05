use super::fixture::ExactCompilerRowScenario;
use super::*;
use omega_package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyError, HistoricalPackagePolicyLimits,
};
use omega_package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
};
use omega_package_manager::review::{
    ReviewOnlyCapabilityConflictSet, ReviewOnlyRootPolicyResolution,
};
use omega_target::TargetProfile;

pub(super) fn resolve_all(
    conflicts: &ReviewOnlyCapabilityConflictSet,
    disposition: ReviewOnlyRootPolicyDisposition,
) -> ReviewOnlyRootPolicyResolution {
    let mut decisions = Vec::new();
    for package in conflicts.packages() {
        for conflict in package
            .conflicts()
            .iter()
            .filter(|conflict| conflict.is_blocking())
        {
            decisions.push(package.root_policy_decision(conflict, disposition).unwrap());
        }
    }
    decisions.reverse();
    resolve_review_only_root_policy_decisions(conflicts, &decisions)
        .expect("resolve exact blocking rows in noncanonical input order")
}

// Deliberately takes only persisted strings, not source custody, reviews,
// conflicts, or a fresh resolution.
fn recover_offline(source_text: &str, decision_text: &str) -> HistoricalPackagePolicyDecisions {
    let subject = CanonicalSourceClosureSubject::recover_text(
        source_text,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("recover retained source subject without resolution or compilation");
    HistoricalPackagePolicyDecisions::recover_text(
        decision_text,
        &subject,
        HistoricalPackagePolicyLimits::default(),
    )
    .expect("recover inert historical decisions without current conflicts")
}

pub(super) fn assert_historical_policy(
    scenario: &ExactCompilerRowScenario,
    conflicts: &ReviewOnlyCapabilityConflictSet,
) {
    let source_limits = CanonicalSourceClosureSubjectLimits::default();
    let limits = HistoricalPackagePolicyLimits::default();
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &scenario
            .candidate_sources
            .for_exact_target(TargetProfile::WindowsX64),
        source_limits,
    )
    .unwrap();
    assert_eq!(conflicts.source_subject(), subject.fingerprint());
    let resolution = resolve_all(
        conflicts,
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
    );
    let historical =
        HistoricalPackagePolicyDecisions::capture(&subject, conflicts, Some(&resolution), limits)
            .expect("capture current choices as historical policy");
    assert_eq!(historical.source_subject(), subject.fingerprint());
    assert_eq!(historical.decisions().len(), 2);
    for decision in historical.decisions() {
        assert_eq!(
            subject.packages()[decision.package_index().unwrap()].key(),
            scenario.candidate_sources.graph().root()
        );
        assert!(
            resolution
                .decisions()
                .iter()
                .any(
                    |fresh| Some(fresh.conflict().digest()) == decision.conflict()
                        && fresh.disposition() == decision.disposition()
                )
        );
    }
    let text = historical.canonical_text(&subject, limits).unwrap();
    let source_text = subject.canonical_text(source_limits).unwrap();
    let recovered = recover_offline(&source_text, &text);
    assert_eq!(recovered, historical);
    assert_eq!(recovered.canonical_text(&subject, limits).unwrap(), text);
    assert_eq!(
        HistoricalPackagePolicyDecisions::capture(
            &subject,
            conflicts,
            Some(&resolve_all(
                conflicts,
                ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
            )),
            limits,
        )
        .unwrap()
        .canonical_text(&subject, limits)
        .unwrap(),
        text
    );

    assert_source_association(
        scenario,
        conflicts,
        &resolution,
        &subject,
        &historical,
        &text,
    );
    assert_rejections_empty_and_stale(scenario, conflicts, &subject, &resolution);
    assert_text_validation(&subject, conflicts, &resolution, &historical, &text);
    assert_requested_target_is_checked(scenario);
}

fn assert_source_association(
    scenario: &ExactCompilerRowScenario,
    conflicts: &ReviewOnlyCapabilityConflictSet,
    resolution: &ReviewOnlyRootPolicyResolution,
    subject: &CanonicalSourceClosureSubject,
    historical: &HistoricalPackagePolicyDecisions,
    text: &str,
) {
    let source_limits = CanonicalSourceClosureSubjectLimits::default();
    let limits = HistoricalPackagePolicyLimits::default();
    let source_text = subject.canonical_text(source_limits).unwrap();
    let changed_role = source_text.replacen("role package\n", "role application\n", 1);
    assert_ne!(changed_role, source_text);
    let role_subject = CanonicalSourceClosureSubject::recover_text(&changed_role, source_limits)
        .expect("construct a distinct source-only root-role subject");
    for foreign in [
        CanonicalSourceClosureSubject::from_resolved(
            &scenario
                .candidate_sources
                .for_exact_target(TargetProfile::LinuxX64),
            source_limits,
        )
        .unwrap(),
        CanonicalSourceClosureSubject::from_resolved(
            &scenario
                .baseline_sources
                .for_exact_target(TargetProfile::WindowsX64),
            source_limits,
        )
        .unwrap(),
        role_subject,
    ] {
        assert_eq!(
            HistoricalPackagePolicyDecisions::capture(
                &foreign,
                conflicts,
                Some(resolution),
                limits,
            ),
            Err(HistoricalPackagePolicyError::SourceSubjectMismatch)
        );
        assert_eq!(
            HistoricalPackagePolicyDecisions::recover_text(text, &foreign, limits),
            Err(HistoricalPackagePolicyError::SourceSubjectMismatch)
        );
        assert_eq!(
            historical.canonical_text(&foreign, limits),
            Err(HistoricalPackagePolicyError::SourceSubjectMismatch)
        );
    }
}

fn assert_rejections_empty_and_stale(
    scenario: &ExactCompilerRowScenario,
    conflicts: &ReviewOnlyCapabilityConflictSet,
    subject: &CanonicalSourceClosureSubject,
    accepted: &ReviewOnlyRootPolicyResolution,
) {
    let limits = HistoricalPackagePolicyLimits::default();
    let rejected = resolve_all(
        conflicts,
        ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
    );
    assert!(!rejected.all_blocking_rows_accepted());
    let historical =
        HistoricalPackagePolicyDecisions::capture(subject, conflicts, Some(&rejected), limits)
            .unwrap();
    let text = historical.canonical_text(subject, limits).unwrap();
    let recovered = HistoricalPackagePolicyDecisions::recover_text(&text, subject, limits).unwrap();
    assert_eq!(recovered, historical);
    assert!(
        recovered
            .decisions()
            .iter()
            .all(|decision| decision.disposition()
                == ReviewOnlyRootPolicyDisposition::RejectCandidateChange)
    );
    assert_eq!(
        HistoricalPackagePolicyDecisions::capture(subject, conflicts, None, limits),
        Err(HistoricalPackagePolicyError::ResolutionMismatch)
    );

    let same = compare_review_only_capabilities(
        &scenario.candidate_reviews,
        &scenario.candidate_reviews,
        &scenario
            .candidate_sources
            .for_exact_target(TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .unwrap();
    assert_eq!(same.conflict_count(), 0);
    let empty = HistoricalPackagePolicyDecisions::capture(subject, &same, None, limits).unwrap();
    assert!(empty.decisions().is_empty());
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text(
            &empty.canonical_text(subject, limits).unwrap(),
            subject,
            limits,
        )
        .unwrap(),
        empty
    );
    assert_eq!(
        HistoricalPackagePolicyDecisions::capture(subject, &same, Some(accepted), limits),
        Err(HistoricalPackagePolicyError::ResolutionMismatch)
    );

    let stale_conflicts = compare_review_only_capabilities(
        &scenario.stale_baseline_reviews,
        &scenario.candidate_reviews,
        &scenario
            .candidate_sources
            .for_exact_target(TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .unwrap();
    assert_eq!(stale_conflicts.conflict_count(), 1);
    let stale = resolve_all(
        &stale_conflicts,
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
    );
    assert_eq!(
        HistoricalPackagePolicyDecisions::capture(subject, conflicts, Some(&stale), limits),
        Err(HistoricalPackagePolicyError::ResolutionMismatch)
    );
}

fn assert_text_validation(
    subject: &CanonicalSourceClosureSubject,
    conflicts: &ReviewOnlyCapabilityConflictSet,
    resolution: &ReviewOnlyRootPolicyResolution,
    historical: &HistoricalPackagePolicyDecisions,
    text: &str,
) {
    let limits = HistoricalPackagePolicyLimits::default();
    let exact_limits = HistoricalPackagePolicyLimits::new(text.len(), historical.decisions().len());
    assert_eq!(
        historical.canonical_text(subject, exact_limits).unwrap(),
        text
    );
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text(text, subject, exact_limits).unwrap(),
        *historical
    );
    assert_recovery_accounting(subject, historical, text);
    let raised_limits = HistoricalPackagePolicyLimits::new(usize::MAX, usize::MAX);
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text(
            &text.replacen("decisions 2\n", "decisions 65537\n", 1),
            subject,
            raised_limits,
        ),
        Err(HistoricalPackagePolicyError::DecisionLimitExceeded)
    );
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text(
            &" ".repeat(8 * 1024 * 1024 + 1),
            subject,
            raised_limits,
        ),
        Err(HistoricalPackagePolicyError::ByteLimitExceeded)
    );
    for restricted in [
        HistoricalPackagePolicyLimits::new(text.len() - 1, 65_536),
        HistoricalPackagePolicyLimits::new(8 * 1024 * 1024, 1),
    ] {
        assert!(
            HistoricalPackagePolicyDecisions::capture(
                subject,
                conflicts,
                Some(resolution),
                restricted,
            )
            .is_err()
        );
        assert!(historical.canonical_text(subject, restricted).is_err());
        assert!(HistoricalPackagePolicyDecisions::recover_text(text, subject, restricted).is_err());
    }
    assert!(text.is_ascii());
    for length in 0..text.len() {
        assert!(
            HistoricalPackagePolicyDecisions::recover_text(&text[..length], subject, limits)
                .is_err()
        );
    }
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 6);
    assert!(lines[3].starts_with("decision 0 "));
    assert!(lines[4].starts_with("decision 0 "));
    let reordered = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n",
        lines[0], lines[1], lines[2], lines[4], lines[3], lines[5]
    );
    let duplicate = text.replacen(lines[4], lines[3], 1);
    for malformed in [
        reordered,
        duplicate,
        text.replacen("omega-policy-decisions 1", "omega-policy-decisions 99", 1),
        text.replacen("decisions 2\n", "decisions 02\n", 1),
        text.replacen("decisions 2\n", "decisions 184467440737095516160\n", 1),
        text.replacen("decision 0 ", "decision 1 ", 1),
        text.replacen("decision 0 ", "decision 00 ", 1),
        text.replacen(" accept\n", " approve\n", 1),
        text.replacen(" accept\n", " accept extra\n", 1),
        text.replace('\n', "\r\n"),
        format!("{text}\n"),
    ] {
        assert_ne!(malformed, text);
        assert!(
            HistoricalPackagePolicyDecisions::recover_text(&malformed, subject, limits).is_err()
        );
    }
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text(
            &text.replacen("decision 0 ", "decision 1 ", 1),
            subject,
            limits,
        ),
        Err(HistoricalPackagePolicyError::UnknownPackage)
    );
}

fn assert_recovery_accounting(
    subject: &CanonicalSourceClosureSubject,
    historical: &HistoricalPackagePolicyDecisions,
    text: &str,
) {
    use omega_package_manager::lock::HistoricalPackagePolicyDecision;

    let count = historical.decisions().len();
    let exact_owned = count * (std::mem::size_of::<HistoricalPackagePolicyDecision>() + 32);
    let limits = HistoricalPackagePolicyLimits::new(text.len(), count);
    let (recovered, usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
        text,
        subject,
        limits,
        exact_owned,
    )
    .unwrap();
    assert_eq!(recovered, *historical);
    assert_eq!(usage.owned_bytes(), exact_owned);
    assert_eq!(usage.decisions(), count);
    assert_eq!(recovered.canonical_text(subject, limits).unwrap(), text);
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text_with_usage(
            text,
            subject,
            limits,
            exact_owned - 1,
        ),
        Err(HistoricalPackagePolicyError::AllocationLimitExceeded),
    );

    // Simulate two independently valid sections sharing one enclosing budget.
    // Retained rows and transient validation scratch are charged cumulatively.
    let mut remaining_owned = exact_owned * 2 - 1;
    let (_, first_usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
        text,
        subject,
        limits,
        remaining_owned,
    )
    .unwrap();
    remaining_owned -= first_usage.owned_bytes();
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text_with_usage(
            text,
            subject,
            limits,
            remaining_owned,
        ),
        Err(HistoricalPackagePolicyError::AllocationLimitExceeded),
    );
    let (_, second_usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
        text,
        subject,
        limits,
        remaining_owned + 1,
    )
    .unwrap();
    assert_eq!(first_usage, second_usage);

    let empty = format!(
        "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
        subject.fingerprint().to_hex(),
    );
    let (empty_policy, empty_usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
        &empty,
        subject,
        HistoricalPackagePolicyLimits::new(empty.len(), 0),
        0,
    )
    .unwrap();
    assert!(empty_policy.decisions().is_empty());
    assert_eq!(empty_usage.owned_bytes(), 0);
    assert_eq!(empty_usage.decisions(), 0);
}

fn assert_requested_target_is_checked(scenario: &ExactCompilerRowScenario) {
    let linux = scenario
        .candidate_sources
        .for_exact_target(TargetProfile::LinuxX64);
    let limits = ReviewOnlyCapabilityConflictLimits::default();
    for result in [
        compare_review_only_initial_capabilities(&scenario.candidate_reviews, &linux, limits),
        compare_review_only_capabilities(
            &scenario.baseline_reviews,
            &scenario.candidate_reviews,
            &linux,
            limits,
        ),
        compare_review_only_capabilities(
            &scenario.candidate_reviews,
            &scenario.candidate_reviews,
            &linux,
            limits,
        ),
    ] {
        assert!(matches!(result,
            Err(ReviewOnlyCapabilityConflictError::CandidateTargetMismatch { package })
                if package.as_ref() == scenario.candidate_sources.graph().root()));
    }
}
