use super::{
    QUESTION_MAGIC, find_subslice, graph_workbench_question, join_question, remove_temporary_tree,
    resolve_external_closure, resolve_workspace_package_closure_from_hardened_base, split_question,
    temporary_root, workspace_root,
};
use crate::package_reconstruction_question::accepted_policy_fixture;
use package_evidence::ledger::OrdinaryPackageObligationStatus;
use package_evidence::record::{
    PackageReviewCallableSupply, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewExternalBinding, PackageReviewForeignLocator,
};
use package_manager::admission::{AcceptedOrdinaryEvidenceError, accept_ordinary_closure_evidence};
use package_manager::resolution::graph::PackageSourceClosureLimits;
use package_manager::review::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionLimits,
    FreshPackageRootPolicyError, LocallyComposedPackageObligationResults,
    PackagePolicyChangeLimits, PackagePolicyDecision, PackagePolicyDecisionError,
    PackagePolicyDecisionSubject, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyRootPolicyDisposition, SemanticBindingReview, bind_fresh_package_root_policy,
    compare_package_policy_changes, compare_review_only_initial_capabilities,
    compile_resolved_package_reviews, resolve_package_policy_decisions,
};
use package_source::{LocalSourceLimits, SourceLineage, SourceRelativePath};

#[test]
fn dependency_open_claims_require_exact_fresh_root_policy() {
    let temporary = temporary_root("open-claim-composition");
    let dependency = temporary.join("dependency");
    let root = temporary.join("root");
    std::fs::create_dir_all(&dependency).expect("create claim dependency");
    std::fs::create_dir_all(&root).expect("create claim consumer");
    std::fs::write(
        dependency.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("claim-dependency");
}
"#,
    )
    .expect("write claim dependency build");
    std::fs::write(
        dependency.join("main.omg"),
        r#"boundary machine trusted_zero() -> u64
ensures result == 0;
"#,
    )
    .expect("write open accepted claim");
    std::fs::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("claim-consumer");
    builder.depend(Source::Path {
        location: "../dependency"
    });
}
"#,
    )
    .expect("write claim consumer build");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write claim consumer source");

    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile two-package claim closure");
    let composed = LocallyComposedPackageObligationResults::from_resolved_and_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
    )
    .expect("compose locally reconstructed open claims");

    assert_eq!(composed.entries().len(), 2);
    let root_entry = composed
        .entries()
        .iter()
        .find(|entry| entry.package() == closure.graph().root())
        .expect("selected root result");
    assert!(root_entry.results().open_accepted_claims().is_empty());
    let propagated = composed.root_open_accepted_claims().collect::<Vec<_>>();
    let [(owner, context, claim)] = propagated.as_slice() else {
        panic!("one dependency claim must propagate to the root")
    };
    assert_eq!(owner.name().as_str(), "claim-dependency");
    assert!(context.purpose().is_product());
    assert_eq!(
        claim.status(),
        OrdinaryPackageObligationStatus::OpenRootAdmission
    );
    assert_eq!(
        claim.callable().supply(),
        PackageReviewCallableSupply::AdmissionClaim
    );
    assert_eq!(
        claim.row().kind(),
        package_evidence::record::PackageReviewCanonicalRowKind::AcceptedClaim
    );
    assert_eq!(
        composed.question().source_closure().packages().len(),
        closure.graph().packages().len()
    );

    let conflicts = compare_review_only_initial_capabilities(
        &reviews,
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("derive exact fresh-admission conflicts");
    let claim_package = conflicts
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "claim-dependency")
        .expect("dependency owns its accepted-claim conflict");
    let claim_conflict = claim_package
        .conflicts()
        .iter()
        .find(|conflict| {
            conflict.kind()
                == package_evidence::record::PackageReviewCanonicalRowKind::AcceptedClaim
        })
        .expect("fresh accepted claim is blocking");

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

    assert!(claim_conflict.is_blocking());
    let changes = compare_package_policy_changes(
        None,
        &reviews,
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        PackagePolicyChangeLimits::default(),
    )
    .expect("compare fresh claim policy");
    let decisions = changes
        .packages()
        .iter()
        .flat_map(|package| package.rows())
        .filter(|row| row.requires_decision())
        .map(|row| PackagePolicyDecision {
            subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
            disposition: ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        })
        .collect::<Vec<_>>();
    assert!(!decisions.is_empty());
    let rejected =
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &decisions)
            .expect("complete ordinary rejection");
    assert!(!rejected.all_required_changes_accepted());
    assert!(matches!(
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &[]),
        Err(PackagePolicyDecisionError::MissingDecision(_))
    ));
    let accepted_policy = accepted_policy_fixture::accepted_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
    );
    let accepted = bind_fresh_package_root_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted_policy),
    )
    .expect("exact fresh policy admits the open claim in memory");
    assert!(!accepted.policy_changes().requires_decision());
    let accepted_claims = accepted
        .obligations()
        .root_open_accepted_claims()
        .collect::<Vec<_>>();
    let [(owner, _, _)] = accepted_claims.as_slice() else {
        panic!("one accepted dependency claim")
    };
    assert_eq!(owner.name().as_str(), "claim-dependency");

    std::fs::write(
        dependency.join("main.omg"),
        "boundary machine trusted_zero() -> u64\nensures result == 0;\n// source-only change\n",
    )
    .expect("change claim source without changing its contract");
    let source_only_closure = resolve_external_closure(&root, temporary.join("source-only-cache"));
    let source_only_reviews = compile_resolved_package_reviews(
        &source_only_closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("source-only-build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile source-only claim change");
    let reused = accept_ordinary_closure_evidence(
        &source_only_closure.for_exact_target(target::TargetProfile::WindowsX64),
        &source_only_reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted_policy),
    )
    .expect("unchanged claim requirements reuse project acceptance");
    assert!(!reused.policy_changes().requires_decision());
    assert!(reused.policy_changes().source_subject_changed());
    assert!(reused.policy_changes().audit_recommended());
    assert_eq!(
        reused
            .acceptance()
            .obligations()
            .root_open_accepted_claims()
            .len(),
        1
    );

    std::fs::write(
        dependency.join("main.omg"),
        r#"boundary machine trusted_zero() -> u64
ensures result == 1;
"#,
    )
    .expect("change accepted claim");
    let changed_closure = resolve_external_closure(&root, temporary.join("changed-cache"));
    let changed_reviews = compile_resolved_package_reviews(
        &changed_closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("changed-build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile changed claim closure");
    let error = accept_ordinary_closure_evidence(
        &changed_closure.for_exact_target(target::TargetProfile::WindowsX64),
        &changed_reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted_policy),
    )
    .expect_err("changed claim contract must require ordinary review");
    let message = error.to_string();
    let AcceptedOrdinaryEvidenceError::RootPolicy(FreshPackageRootPolicyError::ReviewRequired(
        changed,
    )) = error
    else {
        panic!("changed claim must return modern policy findings")
    };
    assert!(changed.requires_decision());
    assert!(
        changed
            .packages()
            .iter()
            .any(|package| package.rows().iter().any(|row| {
                row.kind() == package_evidence::record::PackagePolicyRowKind::Callable
                    && row.change() == package_manager::review::PackagePolicyChangeKind::Changed
                    && row.requires_decision()
                    && row
                        .baseline()
                        .zip(row.candidate())
                        .is_some_and(|(baseline, candidate)| {
                            baseline.key_bytes() == candidate.key_bytes()
                                && baseline.canonical_bytes() != candidate.canonical_bytes()
                        })
            }))
    );
    let rendered =
        package_manager::review::render_package_policy_review(&changed, 16 * 1024 * 1024)
            .expect("bounded modern review");
    assert!(message.contains("run omega update"));
    assert!(message.ends_with(&rendered));
    assert!(matches!(
        resolve_package_policy_decisions(&changed, changes.fingerprint().digest(), &decisions),
        Err(PackagePolicyDecisionError::WrongComparison)
    ));

    remove_temporary_tree(&temporary);
}

#[test]
fn dependency_external_executable_supply_requires_exact_fresh_root_policy() {
    let temporary = temporary_root("open-external-supply-composition");
    let dependency = temporary.join("dependency");
    let root = temporary.join("root");
    std::fs::create_dir_all(&dependency).expect("create external-supply dependency");
    std::fs::create_dir_all(&root).expect("create external-supply consumer");
    std::fs::write(
        dependency.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("foreign-surface");
}
"#,
    )
    .expect("write external-supply dependency build");
    std::fs::write(
        dependency.join("main.omg"),
        r#"use omega::language::core::external_binding;

pub boundary trait ForeignSurface {
    machine invoke() reaches ForeignSurface;
}
pub windows_x86_64 machine invoke_binding() -> Binding<10, 9, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "omega-host",
            export: "invoke_v1",
        },
    }
}
pub windows_x86_64 machine invoke_leaf()
    satisfies ForeignSurface::invoke
    via invoke_binding();
"#,
    )
    .expect("write external executable supply");
    std::fs::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("foreign-consumer");
    builder.depend(Source::Path {
        location: "../dependency"
    });
}
"#,
    )
    .expect("write external-supply consumer build");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write external-supply consumer source");

    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile external-supply closure");
    let composed = LocallyComposedPackageObligationResults::from_resolved_and_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
    )
    .expect("compose locally reconstructed external-supply obligation");

    let root_entry = composed
        .entries()
        .iter()
        .find(|entry| entry.package() == closure.graph().root())
        .expect("selected root result");
    assert!(
        root_entry
            .results()
            .open_external_executable_supplies()
            .is_empty()
    );
    let propagated = composed
        .root_open_external_executable_supplies()
        .collect::<Vec<_>>();
    let [(owner, _, supply)] = propagated.as_slice() else {
        panic!("one dependency external executable supply must propagate to the root")
    };
    assert_eq!(owner.name().as_str(), "foreign-surface");
    assert_eq!(
        supply.status(),
        OrdinaryPackageObligationStatus::OpenRootAdmission
    );
    assert_eq!(
        supply.row().kind(),
        PackageReviewCanonicalRowKind::ExternalExecutableSupply
    );
    assert_eq!(
        supply.row().risk(),
        PackageReviewCanonicalRowRisk::OpaqueBlocking
    );
    assert!(matches!(
        supply.supply().binding(),
        PackageReviewExternalBinding::NormalizedImport(import)
            if matches!(
                import.locator(),
                PackageReviewForeignLocator::PeByName {
                    library,
                    export,
                } if library.as_slice() == "omega-host".as_bytes()
                    && export.as_slice() == "invoke_v1".as_bytes()
            )
    ));

    let conflicts = compare_review_only_initial_capabilities(
        &reviews,
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("derive exact fresh external-supply conflicts");
    assert!(
        conflicts
            .packages()
            .iter()
            .any(|package| package.conflicts().iter().any(|conflict| {
                conflict.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply
                    && conflict.is_blocking()
            }))
    );
    assert!(matches!(
        bind_fresh_package_root_policy(
            &closure.for_exact_target(target::TargetProfile::WindowsX64),
            &reviews,
            CanonicalPackageReconstructionQuestionLimits::default(),
            ReviewOnlyCapabilityConflictLimits::default(),
            None,
        ),
        Err(FreshPackageRootPolicyError::ReviewRequired(_))
    ));

    let accepted_policy = accepted_policy_fixture::accepted_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
    );
    let accepted = bind_fresh_package_root_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted_policy),
    )
    .expect("exact fresh policy admits the external supply in memory");
    assert!(!accepted.policy_changes().requires_decision());
    assert_eq!(
        accepted
            .obligations()
            .root_open_external_executable_supplies()
            .len(),
        1
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn dependency_contract_entailment_stand_down_propagates_but_cannot_be_admitted() {
    let temporary = temporary_root("open-contract-entailment-composition");
    let dependency = temporary.join("dependency");
    let root = temporary.join("root");
    std::fs::create_dir_all(&dependency).expect("create contract dependency");
    std::fs::create_dir_all(&root).expect("create contract consumer");
    std::fs::write(
        dependency.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("contract-surface");
}
"#,
    )
    .expect("write contract dependency build");
    // The ordinary exit preserves its exact entry assumption. The separate
    // polynomial entailment tier still cannot represent min and must stand down.
    std::fs::write(
        dependency.join("main.omg"),
        r#"pub machine unchecked_claim(a: u64, b: u64)
requires
    min(a, b) >= 1
ensures
    min(a, b) >= 1
{
}
"#,
    )
    .expect("write unresolved contract entailment");
    std::fs::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("contract-consumer");
    builder.depend(Source::Path {
        location: "../dependency"
    });
}
"#,
    )
    .expect("write contract consumer build");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write contract consumer source");

    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile unresolved contract-entailment closure");
    let reconstruction_limits = CanonicalPackageReconstructionQuestionLimits::default();
    let conflict_limits = ReviewOnlyCapabilityConflictLimits::default();
    let composed = LocallyComposedPackageObligationResults::from_resolved_and_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        reconstruction_limits,
    )
    .expect("compose locally reconstructed contract-entailment obligation");

    let root_entry = composed
        .entries()
        .iter()
        .find(|entry| entry.package() == closure.graph().root())
        .expect("selected root result");
    assert!(
        root_entry
            .results()
            .open_contract_entailment_obligations()
            .is_empty()
    );
    let propagated = composed
        .root_open_contract_entailment_obligations()
        .collect::<Vec<_>>();
    let [(owner, _, obligation)] = propagated.as_slice() else {
        panic!("one dependency contract-entailment obligation must propagate to the root")
    };
    assert_eq!(owner.name().as_str(), "contract-surface");
    assert_eq!(
        obligation.status(),
        OrdinaryPackageObligationStatus::OpenLaterDischarge
    );
    assert_eq!(
        obligation.row().kind(),
        PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation
    );
    assert_eq!(
        obligation.row().risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );

    let conflicts = compare_review_only_initial_capabilities(
        &reviews,
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        conflict_limits,
    )
    .expect("derive exact fresh contract-entailment conflicts");
    assert!(conflicts.packages().iter().any(|package| {
        package.conflicts().iter().any(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation
                && conflict.risk() == PackageReviewCanonicalRowRisk::Blocking
        })
    }));
    assert!(matches!(
        bind_fresh_package_root_policy(
            &closure.for_exact_target(target::TargetProfile::WindowsX64),
            &reviews,
            reconstruction_limits,
            conflict_limits,
            None,
        ),
        Err(FreshPackageRootPolicyError::UnresolvedLaterDischarge(
            PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation
        ))
    ));
    let accepted_policy = accepted_policy_fixture::accepted_policy(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
    );
    assert!(matches!(
        bind_fresh_package_root_policy(
            &closure.for_exact_target(target::TargetProfile::WindowsX64),
            &reviews,
            reconstruction_limits,
            conflict_limits,
            Some(&accepted_policy),
        ),
        Err(FreshPackageRootPolicyError::UnresolvedLaterDischarge(
            PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation
        ))
    ));
    assert!(matches!(
        accept_ordinary_closure_evidence(
            &closure.for_exact_target(target::TargetProfile::WindowsX64),
            &reviews,
            reconstruction_limits,
            conflict_limits,
            Some(&accepted_policy),
        ),
        Err(AcceptedOrdinaryEvidenceError::RootPolicy(
            FreshPackageRootPolicyError::UnresolvedLaterDischarge(
                PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation
            )
        ))
    ));

    remove_temporary_tree(&temporary);
}

#[test]
fn dependency_contract_assumption_certificate_closes_the_later_discharge() {
    let temporary = temporary_root("contract-assumption-discharge-composition");
    let dependency = temporary.join("dependency");
    let root = temporary.join("root");
    std::fs::create_dir_all(&dependency).expect("create contract dependency");
    std::fs::create_dir_all(&root).expect("create contract consumer");
    std::fs::write(
        dependency.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("contract-surface");
}
"#,
    )
    .expect("write contract dependency build");
    std::fs::write(
        dependency.join("main.omg"),
        r#"pub machine retain(value: u64) -> u64
requires
    value >= 1
ensures
    value >= 1
{
    let retained: u64 = value;
    retained
}
"#,
    )
    .expect("write assumption-discharged contract");
    std::fs::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("contract-consumer");
    builder.depend(Source::Path {
        location: "../dependency"
    });
}
"#,
    )
    .expect("write contract consumer build");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write contract consumer source");

    let target = target::TargetProfile::WindowsX64;
    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target),
        &temporary.join("build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile assumption-discharged contract closure");
    let reconstruction_limits = CanonicalPackageReconstructionQuestionLimits::default();
    let conflict_limits = ReviewOnlyCapabilityConflictLimits::default();
    let composed = LocallyComposedPackageObligationResults::from_resolved_and_reviews(
        &closure.for_exact_target(target),
        &reviews,
        reconstruction_limits,
    )
    .expect("compose locally discharged contract obligation");

    let dependency_entry = composed
        .entries()
        .iter()
        .find(|entry| entry.package().name().as_str() == "contract-surface")
        .expect("dependency result");
    assert!(
        dependency_entry
            .results()
            .open_contract_entailment_obligations()
            .is_empty()
    );
    assert_eq!(
        dependency_entry
            .results()
            .contract_entailment_assumption_discharges()
            .len(),
        1
    );
    assert_eq!(
        dependency_entry
            .results()
            .contract_entailment_assumption_discharges()[0]
            .status(),
        OrdinaryPackageObligationStatus::Discharged
    );
    assert_eq!(
        composed.root_open_contract_entailment_obligations().count(),
        0
    );
    let root_discharges = composed
        .root_contract_entailment_assumption_discharges()
        .collect::<Vec<_>>();
    let [(discharge_owner, _, root_discharge)] = root_discharges.as_slice() else {
        panic!("one dependency discharge must compose to the root")
    };
    assert_eq!(discharge_owner.name().as_str(), "contract-surface");
    assert_eq!(
        *root_discharge,
        &dependency_entry
            .results()
            .contract_entailment_assumption_discharges()[0]
    );

    let conflicts = compare_review_only_initial_capabilities(
        &reviews,
        &closure.for_exact_target(target),
        conflict_limits,
    )
    .expect("derive exact fresh contract conflicts");
    assert!(conflicts.packages().iter().any(|package| {
        package.conflicts().iter().any(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation
                && conflict.risk() == PackageReviewCanonicalRowRisk::Blocking
        })
    }));
    let accepted_policy =
        accepted_policy_fixture::accepted_policy(&closure.for_exact_target(target), &reviews);
    accept_ordinary_closure_evidence(
        &closure.for_exact_target(target),
        &reviews,
        reconstruction_limits,
        conflict_limits,
        Some(&accepted_policy),
    )
    .expect("locally rechecked discharge permits in-memory acceptance");

    remove_temporary_tree(&temporary);
}

#[test]
fn exact_nested_source_request_changes_question_with_identical_ledgers_and_fresh_match_rejects() {
    let (temporary, _closure, reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let fixture_root = workspace_root().join("tests/fixtures/packages");
    let alternate_request_spelling = fixture_root.join(".");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let alternate_closure = resolve_workspace_package_closure_from_hardened_base(
        &workspace_lineage,
        SourceRelativePath::parse("graph-workbench").unwrap(),
        &alternate_request_spelling,
        temporary.join("alternate-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve the same source through alternate exact request spelling");
    let alternate = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &alternate_closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        limits,
    )
    .expect("reuse identical locally reconstructed ledgers with alternate source request");

    assert_ne!(
        question.source_closure().canonical_bytes(),
        alternate.source_closure().canonical_bytes(),
        "exact caller request spelling belongs to the nested source subject"
    );
    assert!(
        question
            .entries()
            .iter()
            .map(|entry| entry.obligations())
            .eq(alternate.entries().iter().map(|entry| entry.obligations())),
        "the alternate question reuses byte-identical obligation ledgers"
    );
    assert_ne!(question.canonical_bytes(), alternate.canonical_bytes());
    assert_ne!(question.fingerprint(), alternate.fingerprint());
    assert!(
        !question
            .matches_resolved_and_reviews(
                &alternate_closure.for_exact_target(target::TargetProfile::WindowsX64),
                &reviews,
                limits
            )
            .expect("alternate fresh reconstruction remains structurally valid"),
        "fresh match must reject a different exact source question"
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn recovery_rejects_missing_duplicate_reordered_and_source_inconsistent_ledgers() {
    let (temporary, _closure, _reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let (version, source, ledgers) = split_question(question.canonical_bytes());
    assert_eq!(ledgers.len(), 4);

    let mut missing = ledgers.clone();
    missing.pop();
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &missing),
            limits,
        )
        .is_err()
    );

    let mut duplicate = ledgers.clone();
    duplicate[2] = duplicate[0].clone();
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &duplicate),
            limits,
        )
        .is_err()
    );

    let mut reordered = ledgers.clone();
    reordered.swap(0, 1);
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &reordered),
            limits,
        )
        .is_err()
    );

    let mut changed_alias = ledgers.clone();
    let graph_ledger = changed_alias
        .iter_mut()
        .find(|ledger| find_subslice(ledger, b"arithmetic_kernels").is_some())
        .expect("root ledger retains dependency alias");
    let alias_offset =
        find_subslice(graph_ledger, b"arithmetic_kernels").expect("root ledger alias offset");
    graph_ledger[alias_offset] = b'b';
    let error = CanonicalPackageReconstructionQuestion::recover(
        &join_question(version, &source, &changed_alias),
        limits,
    )
    .expect_err("source-inconsistent dependency alias must reject");
    assert_eq!(
        error.message(),
        "obligation ledger dependency edges do not match the source subject"
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn recovery_rejects_unknown_version_trailing_bytes_and_resource_violations() {
    let (temporary, _closure, _reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();

    let mut unknown_version = question.canonical_bytes().to_vec();
    unknown_version[QUESTION_MAGIC.len()..QUESTION_MAGIC.len() + 2]
        .copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(CanonicalPackageReconstructionQuestion::recover(&unknown_version, limits).is_err());

    let mut trailing = question.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(CanonicalPackageReconstructionQuestion::recover(&trailing, limits).is_err());

    let record_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_record_bytes: question.canonical_bytes().len() - 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), record_bound,)
            .is_err()
    );

    let package_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_packages: question.entries().len() - 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), package_bound,)
            .is_err()
    );

    let ledger_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_ledger_bytes: 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), ledger_bound,)
            .is_err()
    );

    let aggregate_ledger_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_total_ledger_bytes: 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            question.canonical_bytes(),
            aggregate_ledger_bound,
        )
        .is_err()
    );

    remove_temporary_tree(&temporary);
}
