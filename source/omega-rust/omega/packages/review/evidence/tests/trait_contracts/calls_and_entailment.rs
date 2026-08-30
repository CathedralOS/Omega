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
fn review_rejects_contract_entailment_stand_downs() {
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

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("package review must fail closed on an unresolved stand-down");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rejects unresolved contract-entailment stand-down")
    }));
}
