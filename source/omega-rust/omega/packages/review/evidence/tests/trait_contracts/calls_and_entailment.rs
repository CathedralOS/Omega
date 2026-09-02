use crate::support::*;

#[test]
fn public_contract_call_projection_requires_one_exact_checked_certificate() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Pair [copy] { left: u64; right: u64; }
pub machine make_pair(left: u64, right: u64) -> Pair terminates; {
    transition { _ -> (Pair { left: left, right: right }) }
}
pub proposition projected_left(pair: Pair, expected: u64) = pair.left == expected;
pub trait Worker {
    machine observe(left: u64, right: u64) -> u64
    ensures projected_left(make_pair(left, right), left);
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("public fact-call projection fixture should check");
    project_checked_package_review(&checked)
        .expect("one exact fact-call projection certificate should rejoin");

    let mut missing = checked.clone();
    missing.facts.fact_call_projections.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing fact-call projection certificate must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("fact-call projection rejoins 0 exact eligibility certificates")
    }));

    let mut duplicate = checked;
    let certificate = duplicate.facts.fact_call_projections[0].clone();
    duplicate.facts.fact_call_projections.push(certificate);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate fact-call projection certificates must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("fact-call projection rejoins 2 exact eligibility certificates")
    }));
}

#[test]
fn public_trait_contract_calls_use_the_same_checked_projection() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub machine computed_zero() -> u64 { 0 }
pub trait Worker {
    machine wait() -> u64
    ensures result == computed_zero();
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("public trait contract call fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait contract calls use the checked call row");
    let [trait_shape] = review.public_traits() else {
        panic!("one public trait")
    };
    let [requirement] = trait_shape.requirements() else {
        panic!("one trait requirement")
    };
    let [contract] = requirement.contracts() else {
        panic!("one trait requirement contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("binary trait guarantee")
    };
    assert!(matches!(
        right.as_ref(),
        PackageReviewContractExpression::Call { target, .. }
            if target.nominal().is_some_and(|target| target.path() == "computed_zero::entry")
    ));
}

#[test]
fn review_retains_contract_entailment_stand_downs_as_open_later_discharge_obligations() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine unchecked_claim(a: u64, b: u64)
requires
    min(a, b) >= 1
ensures
    a >= 1
{
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("ordinary checking should retain the out-of-language stand-down");
    let [stand_down] = checked.contract_entailment_stand_downs() else {
        panic!("one exact contract-entailment stand-down")
    };
    assert_eq!(stand_down.contract_index, 1);
    assert_eq!(stand_down.fact_index, 0);
    assert_eq!(
        stand_down.reason,
        psi_validation::ContractEntailmentStandDownReason::OutsideEntailmentLanguage
    );

    let projection = project_checked_package_review(&checked)
        .expect("package review retains an unresolved stand-down as an open obligation");
    let [obligation] = projection.contract_entailment_open_obligations() else {
        panic!("one exact open contract-entailment obligation")
    };
    assert_eq!(obligation.callable().path(), "unchecked_claim");
    assert_eq!(obligation.contract_position(), 1);
    assert_eq!(obligation.fact_position(), 0);
    assert_ne!(obligation.machine_contract_commitment(), [0; 32]);
    assert_eq!(obligation.goal().kind(), PackageReviewContractKind::Ensures);
    assert_eq!(
        obligation.reason(),
        PackageReviewContractEntailmentOpenReason::OutsideEntailmentLanguage
    );
    assert!(matches!(
        obligation.goal().fact(),
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            operator: PackageReviewContractBinaryOperator::GreaterOrEqual,
            ..
        })
    ));
    let rows = projection
        .canonical_rows()
        .expect("open obligation has one canonical blocking row");
    let matching_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation)
        .collect::<Vec<_>>();
    let [row] = matching_rows.as_slice() else {
        panic!("one canonical open contract-entailment row")
    };
    assert_eq!(row.risk(), PackageReviewCanonicalRowRisk::Blocking);
    let locations = row
        .source()
        .authored_locations()
        .expect("open obligation retains authored source custody");
    for role in [
        PackageReviewSourceLocationRole::Declaration,
        PackageReviewSourceLocationRole::ContractClause,
        PackageReviewSourceLocationRole::ProofFact,
    ] {
        assert!(
            locations.iter().any(|location| location.role() == role),
            "open obligation source custody is missing {role:?}"
        );
    }
    let recovered = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(row).expect("encode open-obligation row"),
    )
    .expect("recover open-obligation row");
    assert_eq!(
        recovered.kind(),
        PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation
    );
    assert_eq!(recovered.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(recovered.key_bytes(), row.key_bytes());
    assert_eq!(recovered.canonical_bytes(), row.canonical_bytes());

    let results = reconstruct_ordinary_package_obligation_results(&checked)
        .expect("ordinary reconstruction retains the exact open obligation");
    let [result] = results.open_contract_entailment_obligations() else {
        panic!("one reconstructed open contract-entailment result")
    };
    assert_eq!(
        result.status(),
        OrdinaryPackageObligationStatus::OpenLaterDischarge
    );
    assert_eq!(result.obligation(), obligation);
    assert_eq!(result.row().canonical_bytes(), row.canonical_bytes());
}

#[test]
fn compiler_assumption_certificate_locally_discharges_the_exact_open_obligation() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine retain(value: u64) -> u64
requires
    value >= 1
ensures
    value >= 1
{
    let retained: u64 = value;
    retained
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("assumption-discharge fixture should check");
    assert_eq!(
        checked
            .facts
            .proof
            .contract_entailment_assumption_discharges
            .len(),
        1
    );
    let projection = project_checked_package_review(&checked)
        .expect("review still exposes the structural-validator stand-down");
    assert_eq!(projection.contract_entailment_open_obligations().len(), 1);

    let results = reconstruct_ordinary_package_obligation_results(&checked)
        .expect("package evidence independently rechecks the compiler certificate");
    assert!(results.open_contract_entailment_obligations().is_empty());
    let [discharge] = results.contract_entailment_assumption_discharges() else {
        panic!("one exact assumption discharge")
    };
    assert_eq!(
        discharge.status(),
        OrdinaryPackageObligationStatus::Discharged
    );
    assert_eq!(
        discharge.obligation(),
        &projection.contract_entailment_open_obligations()[0]
    );
    assert_eq!(discharge.assumptions(), [discharge.goal().clone()]);
    assert_eq!(discharge.selected_assumption_position(), 0);

    let mut missing = checked.clone();
    missing
        .facts
        .proof
        .contract_entailment_assumption_discharges
        .clear();
    let missing_results = reconstruct_ordinary_package_obligation_results(&missing)
        .expect("a missing certificate must leave the obligation open");
    assert!(
        missing_results
            .contract_entailment_assumption_discharges()
            .is_empty()
    );
    assert_eq!(
        missing_results.open_contract_entailment_obligations().len(),
        1
    );

    let certificate = checked
        .facts
        .proof
        .contract_entailment_assumption_discharges[0]
        .clone();
    let mut tampered = checked.clone();
    let original = &tampered
        .facts
        .proof
        .contract_entailment_assumption_discharges[0];
    let changed = psi_checked_trees::CheckedContractEntailmentAssumptionDischarge::new(
        original.machine_symbol(),
        original.contract_position(),
        original.fact_position(),
        original.machine_contract_commitment(),
        original.assumptions().to_vec(),
        psi_core::Proposition::Falsehood,
        original.selected_assumption_position(),
    )
    .expect("well-formed tamper fixture");
    tampered
        .facts
        .proof
        .contract_entailment_assumption_discharges[0] = changed;
    let diagnostics = reconstruct_ordinary_package_obligation_results(&tampered)
        .expect_err("changed certificate must fail local reconstruction");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("certificate failed local recheck")
    }));

    let mut duplicate = checked;
    duplicate
        .facts
        .proof
        .contract_entailment_assumption_discharges = vec![certificate.clone(), certificate];
    let diagnostics = reconstruct_ordinary_package_obligation_results(&duplicate)
        .expect_err("duplicate certificate must fail the exact obligation join");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not rejoin exactly one open obligation")
    }));
}

#[test]
fn equal_contract_entailment_goals_retain_distinct_positions_and_complete_hypotheses() {
    let Some(target) = host_target_name() else {
        return;
    };
    let project = |minimum: u64| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"machine unchecked_claim(a: u64, b: u64)
requires
    min(a, b) >= {minimum}
ensures
    a >= 1
ensures
    a >= 1
{{
}}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("duplicate open-goal fixture should check");
        project_checked_package_review(&checked)
            .expect("duplicate equal goals retain exact coordinates")
    };

    let first = project(1);
    let obligations = first.contract_entailment_open_obligations();
    assert_eq!(obligations.len(), 2);
    assert_eq!(
        obligations
            .iter()
            .map(|obligation| (obligation.contract_position(), obligation.fact_position()))
            .collect::<Vec<_>>(),
        vec![(1, 0), (2, 0)]
    );
    assert_eq!(obligations[0].goal(), obligations[1].goal());
    let rows = first.canonical_rows().expect("encode duplicate open goals");
    let open_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation)
        .collect::<Vec<_>>();
    assert_eq!(open_rows.len(), 2);
    assert_ne!(open_rows[0].key_bytes(), open_rows[1].key_bytes());

    let changed_hypothesis = project(2);
    assert_ne!(
        first.contract_entailment_open_obligations()[0].machine_contract_commitment(),
        changed_hypothesis.contract_entailment_open_obligations()[0].machine_contract_commitment(),
        "the complete machine contract, not only the displayed goal, binds the open question"
    );
    assert_ne!(
        first
            .canonical_review_bytes()
            .expect("encode first open question"),
        changed_hypothesis
            .canonical_review_bytes()
            .expect("encode changed open question"),
        "whole-review identity must retain the open-obligation lane"
    );
}
