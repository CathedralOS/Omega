mod support;

use support::*;

#[test]
fn public_trait_operational_envelope_is_exact_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait Console { }
pub boundary trait Worker {
    machine wait(handler: &mut Console)
    reaches <= Console
    invokes handler;
    invokes Console;
    suspends;
    blocks;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait suspension fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait operational review should close");
    let worker = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let [wait] = worker.requirements() else {
        panic!("one worker requirement")
    };
    let [console] = wait.service_reach() else {
        panic!("one exact service-reach row")
    };
    assert_eq!(console.path(), "Console");
    assert_eq!(
        console.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(wait.service_reach_is_installation_bound());
    assert_eq!(wait.synchronous_invocations().len(), 2);
    assert_eq!(wait.synchronous_invocations()[0].parameter(), Some(0));
    assert_eq!(
        wait.synchronous_invocations()[1]
            .service()
            .expect("service invocation")
            .path(),
        "Console"
    );
    assert!(wait.suspends());
    assert!(wait.blocks());
}

#[test]
fn public_trait_termination_is_parameter_rooted_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait SchedulerRuntime {
    machine wait(&self, scheduler: SchedulerRuntime)
    requires self in WeakFair
    requires scheduler in WeakFair
    terminates;
}
pub domain SchedulerRuntime::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerRuntime) -> SchedulerRuntime in WeakFair;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait termination fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait termination review should close");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let wait = runtime
        .requirements()
        .iter()
        .find(|requirement| {
            requirement
                .identity()
                .path()
                .contains("SchedulerRuntime::wait")
        })
        .expect("wait requirement row");
    let premises = wait
        .termination()
        .premises()
        .expect("wait must promise termination");
    assert_eq!(premises.len(), 2);
    for premise in premises {
        assert_eq!(premise.profile().path(), "SchedulerRuntime::WeakFair");
        assert_eq!(
            premise.profile().owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert!(premise.projections().is_empty());
    }
    assert!(premises[0].subject().is_receiver());
    assert_eq!(premises[1].subject().parameter(), Some(0));
}

#[test]
fn public_trait_termination_rejects_a_non_public_progress_profile() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect_err("ordinary visibility must reject a private profile in a public trait contract");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `SchedulerHandle::WeakFair`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn review_projects_trait_defaults_and_unnamed_contracts() {
    let default_package = TempPackage::new();
    default_package.write(
        "main.omg",
        r#"pub trait Worker {
    machine wait() { }
}
"#,
    );
    default_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &default_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&default_package.0),
    )
    .expect("public trait default fixture should check");
    let default_review = project_checked_package_review(&checked)
        .expect("review should retain a public trait default realization");
    let default_requirement = &default_review.public_traits()[0].requirements()[0];
    assert!(default_requirement.has_default_realization());

    let abstract_package = TempPackage::new();
    abstract_package.write(
        "main.omg",
        r#"pub trait Worker {
    machine wait();
}
"#,
    );
    abstract_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let abstract_checked = compile_to_checked_with_packages(
        &abstract_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&abstract_package.0),
    )
    .expect("abstract public trait fixture should check");
    let abstract_review = project_checked_package_review(&abstract_checked)
        .expect("review should retain an abstract public trait requirement");
    assert!(!abstract_review.public_traits()[0].requirements()[0].has_default_realization());
    assert_ne!(
        default_review.canonical_review_bytes().unwrap(),
        abstract_review.canonical_review_bytes().unwrap(),
    );

    let precondition_package = TempPackage::new();
    precondition_package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair;
}
"#,
    );
    precondition_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &precondition_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&precondition_package.0),
    )
    .expect("public progress precondition fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("an unnamed trait precondition should project exactly");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let [wait] = runtime.requirements() else {
        panic!("one scheduler requirement")
    };
    let [contract] = wait.contracts() else {
        panic!("one exact trait contract")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Requires);
    assert_eq!(contract.binding(), None);
    let PackageReviewContractFact::Membership { value, domain } = contract.fact() else {
        panic!("trait precondition must retain exact membership")
    };
    assert_eq!(value, &PackageReviewContractExpression::Parameter(0));
    assert_eq!(domain.path(), "SchedulerHandle::WeakFair");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let runtime_row = review
        .canonical_rows()
        .expect("trait precondition canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicTrait
                && row
                    .key_bytes()
                    .windows("SchedulerRuntime".len())
                    .any(|window| window == b"SchedulerRuntime")
        })
        .expect("scheduler runtime trait row");
    assert!(
        runtime_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ContractClause
            }))
    );
}

#[test]
fn public_trait_requires_and_ensures_change_comparison_identity() {
    let project = |minimum: u8| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                "pub trait Bounds {{\n    machine clamp(value: u64) -> u64\n    requires value >= {minimum}\n    ensures result >= value;\n}}\n"
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public trait contract fixture should check");
        project_checked_package_review(&checked).expect("public trait contracts should project")
    };

    let first = project(1);
    let changed = project(2);
    let [bounds] = first.public_traits() else {
        panic!("one public trait")
    };
    let [clamp] = bounds.requirements() else {
        panic!("one public trait requirement")
    };
    assert_eq!(clamp.contracts().len(), 2);
    assert!(
        clamp
            .contracts()
            .iter()
            .any(|contract| contract.kind() == PackageReviewContractKind::Requires)
    );
    assert!(
        clamp
            .contracts()
            .iter()
            .any(|contract| contract.kind() == PackageReviewContractKind::Ensures)
    );
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing a public trait contract must change comparison identity"
    );
}

#[test]
fn public_trait_named_witness_contracts_retain_exact_lanes_and_selector_identity() {
    let project = |requires_binding: &str, ensures_binding: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub trait Evidence {{
    machine witness();
}}
pub proposition ready() evidence Evidence;
pub trait Worker {{
    machine relay(value: i32) -> i32
    requires {requires_binding}: ready()
    ensures {ensures_binding}: ready()
    {{
        {ensures_binding} = {requires_binding};
    }}
}}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("named public-trait witness fixture should check");
        project_checked_package_review(&checked)
            .expect("named public-trait witness contracts should project")
    };

    let original = project("input_proof", "output_proof");
    let renamed_requires = project("renamed_local_input", "output_proof");
    let renamed_ensures = project("input_proof", "renamed_public_output");
    let worker = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let [relay] = worker.requirements() else {
        panic!("one worker requirement")
    };
    assert_eq!(relay.contracts().len(), 2);
    for contract in relay.contracts() {
        assert!(contract.evidence_lane_position().is_some());
        let PackageReviewContractFact::Proposition(application) = contract.fact() else {
            panic!("named witness contract must retain proposition identity")
        };
        assert_eq!(application.declaration().path(), "ready");
        let PackageReviewPropositionEvidence::Witness(interface) = application.evidence() else {
            panic!("named witness contract must retain its evidence interface")
        };
        assert_eq!(interface.trait_identity().path(), "Evidence");
        assert_eq!(interface.requirements().len(), 1);
    }
    let requires = relay
        .contracts()
        .iter()
        .find(|contract| contract.kind() == PackageReviewContractKind::Requires)
        .expect("named requires row");
    let ensures = relay
        .contracts()
        .iter()
        .find(|contract| contract.kind() == PackageReviewContractKind::Ensures)
        .expect("named ensures row");
    assert_eq!(requires.binding(), None, "requires binding is local");
    assert_eq!(ensures.binding(), Some("output_proof"));
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed_requires.canonical_review_bytes().unwrap(),
        "renaming a local requires evidence alias must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        renamed_ensures.canonical_review_bytes().unwrap(),
        "renaming a public ensures selector must change review identity",
    );
}

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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
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
    assert_eq!(trap.cause(), psi_checked_trees::CrashCause::Trap);
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
    assert_eq!(abort.cause(), psi_checked_trees::CrashCause::Abort);
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
        r#"target windows_x64 { }
target linux_x64 { }
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
