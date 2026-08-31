use super::fixture::ExactCompilerRowScenario;
use super::*;

pub(super) fn assert_candidate_binding(
    scenario: &ExactCompilerRowScenario,
    conflicts: &omega_package_manager::review::ReviewOnlyCapabilityConflictSet,
) {
    let [package] = conflicts.packages() else {
        panic!("one package has candidate-bound conflicts")
    };
    let [conflict, _] = package.conflicts() else {
        panic!("two added public proposition rows")
    };
    let first_accept = package
        .root_policy_decision(
            conflict,
            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        )
        .expect("bind first exact blocking row");

    write_package(
        &scenario.live,
        r#"boundary machine trusted_zero() -> u64
ensures result == 0;
"#,
    );
    let accepted_claim_baseline_sources = resolve_external_local_package_closure(
        &scenario.live,
        ExternalSourceContext::derive(b"accepted-claim-conflict-test-lock"),
        &scenario.accepted_claim_baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve accepted-claim baseline");
    let accepted_claim_baseline_reviews = compile_resolved_package_reviews(
        &accepted_claim_baseline_sources,
        "windows_x86_64",
        &scenario.build_root,
    )
    .expect("compile accepted-claim baseline");

    write_package(
        &scenario.live,
        r#"boundary machine trusted_zero() -> u64
ensures result == 1;
"#,
    );
    let accepted_claim_candidate_sources = resolve_external_local_package_closure(
        &scenario.live,
        ExternalSourceContext::derive(b"accepted-claim-conflict-test-lock"),
        &scenario.accepted_claim_candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve changed accepted claim");
    let accepted_claim_candidate_reviews = compile_resolved_package_reviews(
        &accepted_claim_candidate_sources,
        "windows_x86_64",
        &scenario.build_root,
    )
    .expect("compile changed accepted claim");
    let accepted_claim_conflicts = compare_review_only_capabilities(
        &accepted_claim_baseline_reviews,
        &accepted_claim_candidate_reviews,
        &accepted_claim_candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare changed accepted claim");
    let accepted_claim_conflict = accepted_claim_conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .expect("changed accepted guarantee produces an exact trust conflict");
    assert_eq!(
        accepted_claim_conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert_eq!(
        accepted_claim_conflict.risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    assert!(accepted_claim_conflict.is_blocking());
    let accepted_claim_render = accepted_claim_conflicts
        .render_bounded(1024 * 1024)
        .expect("render changed accepted claim");
    assert!(accepted_claim_render.contains("baseline_location contract_clause package "));
    assert!(accepted_claim_render.contains("candidate_location contract_clause package "));
    let accepted_claim_package = accepted_claim_conflicts
        .packages()
        .iter()
        .find(|package| {
            package
                .conflicts()
                .iter()
                .any(|conflict| conflict.fingerprint() == accepted_claim_conflict.fingerprint())
        })
        .expect("accepted-claim package");
    let wrong_candidate_decision = accepted_claim_package
        .root_policy_decision(
            accepted_claim_conflict,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        )
        .expect("bind other candidate decision");
    assert!(matches!(
        resolve_review_only_root_policy_decisions(
            &conflicts,
            &[wrong_candidate_decision, first_accept]
        ),
        Err(ReviewOnlyRootPolicyResolutionError::WrongCandidateClosure { .. })
    ));
    assert!(matches!(
        package.root_policy_decision(
            accepted_claim_conflict,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        ),
        Err(ReviewOnlyRootPolicyResolutionError::ConflictDoesNotBelongToPackage { .. })
    ));
    assert!(
        accepted_claim_conflict
            .candidate_source()
            .and_then(PackageReviewCanonicalRowSource::authored_locations)
            .expect("accepted-claim conflict has declaration provenance")
            .iter()
            .any(|location| location.relative_path() == "main.omg")
    );
    assert_eq!(
        triage_review_update(
            &accepted_claim_baseline_reviews,
            &accepted_claim_baseline_reviews,
            &BTreeSet::new(),
        )
        .disposition(),
        PackageTriageDisposition::NoReviewBlocker,
        "an unchanged accepted baseline remains visible without blanket reapproval"
    );
}
