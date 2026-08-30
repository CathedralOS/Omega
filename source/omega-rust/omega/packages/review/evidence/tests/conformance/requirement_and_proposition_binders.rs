use crate::support::*;

#[test]
fn review_projects_trait_requirement_identity_machine_parameter() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "pub trait LocalSlot<machine Requirement> { }\n");
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
    .expect("public requirement-identity fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("closed requirement-identity parameter should project");
    let local_slot = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path().contains("LocalSlot"))
        .expect("LocalSlot review row");
    let [parameter] = local_slot.type_parameters() else {
        panic!("one requirement-identity machine parameter")
    };
    assert!(matches!(
        parameter.kind(),
        PackageReviewTypeParameterKind::Machine(
            PackageReviewMachineParameterContract::RequirementIdentity
        )
    ));
    assert_eq!(
        review.canonical_review_bytes().unwrap(),
        project_checked_package_review(&checked)
            .unwrap()
            .canonical_review_bytes()
            .unwrap(),
    );
    assert_eq!(
        review.canonical_rows().unwrap(),
        project_checked_package_review(&checked)
            .unwrap()
            .canonical_rows()
            .unwrap(),
    );
}

#[test]
fn review_projects_alpha_normalized_public_conformance_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let original = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Ranked<u32>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Ranked<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    original.write("build.omg", build);
    let renamed = TempPackage::new();
    renamed.write(
        "main.omg",
        r#"pub trait Ranked<Measure> { }
pub trait Alternate<Measure> { }
pub trait Ordering<Value, Unused, OrderingEvidence: Value satisfies Ranked<u32>> { }
pub machine identity<Value, Unused, IdentityEvidence: Value satisfies Ranked<u32>>(value: Value) -> Value {
    value
}
"#,
    );
    renamed.write("build.omg", build);
    let changed = TempPackage::new();
    changed.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Alternate<u32>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Alternate<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    changed.write("build.omg", build);
    let changed_subject = TempPackage::new();
    changed_subject.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Other satisfies Ranked<u32>> { }
pub machine identity<Element, Other, Evidence: Other satisfies Ranked<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    changed_subject.write("build.omg", build);
    let changed_argument = TempPackage::new();
    changed_argument.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Ranked<u64>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Ranked<u64>>(value: Element) -> Element {
    value
}
"#,
    );
    changed_argument.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic conformance-binder fixture should check");
        project_checked_package_review(&checked)
            .expect("generic conformance-binder review should close")
    };
    let original = review(&original);
    let renamed = review(&renamed);
    let changed = review(&changed);
    let changed_subject = review(&changed_subject);
    let changed_argument = review(&changed_argument);

    let ordering = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Ordering")
        .expect("public Ordering row");
    let [trait_bound] = ordering.conformance_bounds() else {
        panic!("one exact trait conformance binder")
    };
    assert_eq!(trait_bound.binder_ordinal(), Some(0));
    assert_eq!(trait_bound.subject_parameter(), 0);
    assert_eq!(trait_bound.trait_identity().path(), "Ranked");
    assert_eq!(trait_bound.arguments().len(), 1);

    let identity = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("public identity row");
    let [callable_bound] = identity.conformance_bounds() else {
        panic!("one exact callable conformance binder")
    };
    assert_eq!(callable_bound.binder_ordinal(), Some(0));
    assert_eq!(callable_bound.subject_parameter(), 0);
    assert_eq!(callable_bound.trait_identity().path(), "Ranked");

    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming conformance, type, and lifetime-free evidence binders must not change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing the exact conformance trait must change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_subject.canonical_review_bytes().unwrap(),
        "changing the conformance subject parameter must change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_argument.canonical_review_bytes().unwrap(),
        "changing a conformance trait argument must change review identity"
    );
}

#[test]
fn review_projects_alpha_normalized_trait_proposition_parameter_signatures() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    let source = |carrier: &str, relation: &str, left: &str, right: &str, right_type: &str| {
        format!(
            r#"pub trait RelationShape<{carrier}, proposition {relation}>
where proposition {relation}({left}: {carrier}, {right}: {right_type});
{{}}
"#,
        )
    };
    original.write(
        "main.omg",
        &source("Carrier", "Relation", "left", "right", "Carrier"),
    );
    renamed.write(
        "main.omg",
        &source("Value", "Equivalent", "first", "second", "Value"),
    );
    changed.write(
        "main.omg",
        &source("Carrier", "Relation", "left", "right", "u64"),
    );
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("public proposition-parameter trait should check");
        project_checked_package_review(&checked)
            .expect("proposition-parameter signatures have canonical review rows")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed = project(&changed);
    let shape = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "RelationShape")
        .expect("RelationShape trait");
    let [_, relation] = shape.type_parameters() else {
        panic!("carrier and proposition parameters")
    };
    let PackageReviewTypeParameterKind::Proposition(signature) = relation.kind() else {
        panic!("proposition parameter signature")
    };
    let [left, right] = signature.parameters() else {
        panic!("two proposition value parameters")
    };
    assert!(
        left.type_identity()
            .canonical()
            .contains("type-parameter:0")
    );
    assert!(
        right
            .type_identity()
            .canonical()
            .contains("type-parameter:0")
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming trait, proposition, and proposition-value binders must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing a proposition parameter value type must change review identity",
    );
}

#[test]
fn review_rejects_uncertified_proposition_parameter_modes() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait RelationShape<Carrier, proposition Relation>
where proposition Relation(const value: Carrier);
{}
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
    .expect("non-default proposition parameter mode currently reaches checked IR");
    let diagnostics = project_checked_package_review(&checked).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("non-default value-parameter mode not yet certified")
    }));
}

#[test]
fn review_projects_generic_proposition_contract_endpoints_by_static_ordinal() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed_endpoint = TempPackage::new();
    let changed_arguments = TempPackage::new();
    let source = |carrier: &str,
                  relation: &str,
                  alternate: &str,
                  left: &str,
                  right: &str,
                  selected: &str,
                  first_argument: &str,
                  second_argument: &str| {
        format!(
            r#"pub trait RelationLaw<{carrier}, proposition {relation}, proposition {alternate}>
where proposition {relation}(first: {carrier}, second: {carrier});
where proposition {alternate}(first: {carrier}, second: {carrier});
{{
    machine reverse({left}: {carrier}, {right}: {carrier})
    ensures {selected}({first_argument}, {second_argument});
}}
"#,
        )
    };
    original.write(
        "main.omg",
        &source(
            "Carrier",
            "Relation",
            "Alternate",
            "left",
            "right",
            "Relation",
            "right",
            "left",
        ),
    );
    renamed.write(
        "main.omg",
        &source(
            "Value",
            "Equivalent",
            "Other",
            "left",
            "right",
            "Equivalent",
            "right",
            "left",
        ),
    );
    changed_endpoint.write(
        "main.omg",
        &source(
            "Carrier",
            "Relation",
            "Alternate",
            "left",
            "right",
            "Alternate",
            "right",
            "left",
        ),
    );
    changed_arguments.write(
        "main.omg",
        &source(
            "Carrier",
            "Relation",
            "Alternate",
            "left",
            "right",
            "Relation",
            "left",
            "right",
        ),
    );
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed_endpoint, &changed_arguments] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic proposition contract endpoint should check");
        project_checked_package_review(&checked)
            .expect("generic proposition contract endpoint should project exactly")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed_endpoint = project(&changed_endpoint);
    let changed_arguments = project(&changed_arguments);
    let law = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "RelationLaw")
        .expect("RelationLaw trait row");
    let [reverse] = law.requirements() else {
        panic!("one relation law")
    };
    let [contract] = reverse.contracts() else {
        panic!("one relation law contract")
    };
    let PackageReviewContractFact::PropositionParameter(application) = contract.fact() else {
        panic!("generic proposition-parameter application")
    };
    assert_eq!(application.binder_ordinal(), 1);
    assert_eq!(
        application.arguments(),
        [
            PackageReviewContractExpression::Parameter(1),
            PackageReviewContractExpression::Parameter(0),
        ]
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming trait and proposition-family binders must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_endpoint.canonical_review_bytes().unwrap(),
        "selecting a different proposition-family binder must change review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_arguments.canonical_review_bytes().unwrap(),
        "changing proposition value arguments must change review identity",
    );
}

#[test]
fn compiler_rejects_named_generic_proposition_evidence_before_package_review() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait RelationLaw<Carrier, proposition Relation>
where proposition Relation(left: Carrier, right: Carrier);
{
    machine use(value: Carrier)
    requires proof: Relation(value, value);
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
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect_err("named generic proposition evidence must fail before checked lowering");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not resolve to one nominal proposition endpoint")
    }));
}
