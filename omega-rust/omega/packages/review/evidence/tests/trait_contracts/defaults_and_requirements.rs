use crate::support::*;

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
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &default_package.0.join("main.omg"),
        Some("windows_x86_64"),
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
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let abstract_checked = compile_to_checked_with_packages(
        &abstract_package.0.join("main.omg"),
        Some("windows_x86_64"),
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
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &precondition_package.0.join("main.omg"),
        Some("windows_x86_64"),
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
            r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
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
            r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
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
