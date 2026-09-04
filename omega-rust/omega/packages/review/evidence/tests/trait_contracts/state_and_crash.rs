use crate::support::*;

#[test]
fn public_trait_member_contracts_join_exact_state_signature_places() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Threshold { minimum: u64; }
pub trait Bounds {
    machine accepts(threshold: Threshold, value: u64)
    requires value >= threshold.minimum;
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("public trait member contract should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait member contract should join checked places");
    let [bounds] = review.public_traits() else {
        panic!("one public trait")
    };
    let [accepts] = bounds.requirements() else {
        panic!("one public trait requirement")
    };
    let [contract] = accepts.contracts() else {
        panic!("one public trait contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("one exact binary contract expression")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = right.as_ref()
    else {
        panic!("right operand must retain the exact field place")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(member.path(), "Threshold::minimum");
    assert_eq!(
        member.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn public_trait_crash_ceilings_are_exact_canonical_checked_routes() {
    let project = |trap_guard: &str, stop_cause: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                "pub trait Worker {{\n    machine run(flag: bool)\n    crashes Trap {trap_guard};\n    machine stop() crashes {stop_cause};\n    machine idle();\n}}\n"
            ),
        );
        package.write(
            "build.omg",
            r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("public trait crash ceiling should check");
        project_checked_package_review(&checked)
            .expect("public trait crash ceiling should project from its exact checked capsule")
    };

    let first = project("flag", "Abort");
    let guard_changed = project("!flag", "Abort");
    let cause_changed = project("flag", "Trap");
    let worker = first
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let run = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path().contains("Worker::run"))
        .expect("run requirement row");
    let [trap] = run.published_crash() else {
        panic!("one guarded trap route")
    };
    assert_eq!(trap.cause(), PackageReviewCrashCause::Trap);
    let [PackageReviewCrashRouteGuard::Predicate(predicate)] = trap.alternative_guards() else {
        panic!("trap route must retain one canonical predicate guard")
    };
    assert!(!predicate.canonical_bytes().is_empty());
    let stop = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path().contains("Worker::stop"))
        .expect("stop requirement row");
    let [abort] = stop.published_crash() else {
        panic!("one unconditional abort route")
    };
    assert_eq!(abort.cause(), PackageReviewCrashCause::Abort);
    assert_eq!(
        abort.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let idle = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path().contains("Worker::idle"))
        .expect("idle requirement row");
    assert!(idle.published_crash().is_empty());
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        guard_changed.canonical_review_bytes().unwrap(),
        "changing a crash guard must change package comparison identity"
    );
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        cause_changed.canonical_review_bytes().unwrap(),
        "changing a crash cause must change package comparison identity"
    );
}

#[test]
fn public_trait_crash_projection_rejects_missing_or_duplicate_checked_capsules() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Worker {
    machine run() crashes Trap;
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("public trait crash fixture should check");

    let mut missing = checked.clone();
    missing.facts.contract_plans.crash_capsules.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing checked crash capsule must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no exact checked crash capsule")
    }));

    let mut duplicate = checked;
    let capsule = duplicate.facts.contract_plans.crash_capsules[0].clone();
    duplicate.facts.contract_plans.crash_capsules.push(capsule);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate checked crash capsules must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has duplicate checked crash capsules")
    }));
}
