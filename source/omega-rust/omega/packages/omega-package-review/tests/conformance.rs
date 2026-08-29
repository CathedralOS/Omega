mod support;

use support::*;

#[test]
fn review_projects_trait_requirement_identity_machine_parameter() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "pub trait LocalSlot<machine Requirement> { }\n");
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
    let build = r#"target windows_x64 { }
target linux_x64 { }
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
    let build = r#"target windows_x64 { }
target linux_x64 { }
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
    let build = r#"target windows_x64 { }
target linux_x64 { }
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
        r#"target windows_x64 { }
target linux_x64 { }
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

#[test]
fn review_projects_composed_relation_laws_with_forwarded_proposition_family() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Reflexive<Carrier, proposition Relation>
where proposition Relation(left: Carrier, right: Carrier);
{
    machine reflexive(value: Carrier)
    ensures Relation(value, value);
}

pub trait Symmetric<Carrier, proposition Relation>
where proposition Relation(left: Carrier, right: Carrier);
{
    machine symmetric(left: Carrier, right: Carrier)
    requires Relation(left, right)
    ensures Relation(right, left);
}

pub trait Equivalence<Carrier, proposition Relation>:
    Reflexive<Carrier, Relation>
    + Symmetric<Carrier, Relation>
where proposition Relation(left: Carrier, right: Carrier);
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
    .expect("composed relation laws should check");
    let review = project_checked_package_review(&checked)
        .expect("composed relation laws should have exact package-review rows");
    let equivalence = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Equivalence")
        .expect("Equivalence trait row");
    assert_eq!(equivalence.parents().len(), 2);
    for parent in equivalence.parents() {
        let [carrier, relation] = parent.arguments() else {
            panic!("forwarded carrier and proposition-family arguments")
        };
        assert!(carrier.canonical().contains("type-parameter:0"));
        assert!(relation.canonical().contains("type-parameter:1"));
    }
}

#[test]
fn review_rejects_generic_proposition_endpoint_and_value_symbol_spoofs() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait RelationLaw<Carrier, proposition Relation>
where proposition Relation(value: Carrier);
{
    machine use(value: Carrier)
    ensures Relation(value);
}

trait OtherLaw<Carrier, proposition OtherRelation>
where proposition OtherRelation(value: Carrier);
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
    .expect("generic proposition spoof fixture should check before mutation");
    project_checked_package_review(&checked).expect("unmodified generic proposition review");

    let law = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "RelationLaw")
        .expect("RelationLaw definition");
    let [signature] = checked.trait_machine_signatures(law) else {
        panic!("one RelationLaw requirement")
    };
    let [contract] = checked.state_signature_contracts(signature) else {
        panic!("one RelationLaw contract")
    };
    let fact_handle = contract.facts.start();
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        checked.proof_facts.get(fact_handle)
    else {
        panic!("generic proposition fact")
    };
    let [argument_handle] = checked
        .expression_table
        .expression_handles(application.arguments)
    else {
        panic!("one generic proposition argument")
    };
    let argument_handle = *argument_handle;

    let other = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "OtherLaw")
        .expect("OtherLaw definition");
    let [_, other_relation] = checked.trait_type_parameters(other) else {
        panic!("OtherLaw carrier and proposition parameters")
    };
    let other_relation_symbol = other_relation.symbol;
    let psi_typed_trees::data::TypeParameterKind::Proposition { contract } = &other_relation.kind
    else {
        panic!("OtherRelation signature")
    };
    let [other_value] = checked.state_parameters.span_or_empty(contract.parameters) else {
        panic!("one OtherRelation value parameter")
    };
    let other_value_symbol = other_value.symbol;

    let mut endpoint_spoof = checked.clone();
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        endpoint_spoof.typed.proof_facts.get_mut(fact_handle)
    else {
        panic!("generic proposition fact")
    };
    application.proposition = other_relation_symbol;
    let diagnostics = project_checked_package_review(&endpoint_spoof)
        .expect_err("a foreign generic proposition binder must not rejoin by category");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("generic proposition endpoint rejoins 0 callable static binders")
    }));

    let mut value_spoof = checked;
    let psi_typed_trees::expression::ExpressionNode::Name(path) = value_spoof
        .typed
        .expression_table
        .expression_mut(argument_handle)
    else {
        panic!("generic proposition name argument")
    };
    path.head_symbol = other_value_symbol;
    path.symbol = other_value_symbol;
    let diagnostics = project_checked_package_review(&value_spoof)
        .expect_err("a same-spelled foreign value symbol must not rejoin a callable parameter");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contract parameter spelling does not match its exact resolved symbol")
    }));
}

#[test]
fn review_static_machine_contracts_are_recursive_alpha_stable_and_shape_sensitive() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    let original_source = r#"pub machine register<machine Schema>()
where machine Schema<machine Inner>(value: u64) -> u64
where machine Inner(value: u64) -> u64
requires value == value;
{ }
"#;
    original.write("main.omg", original_source);
    renamed.write(
        "main.omg",
        r#"pub machine register<machine Operation>()
where machine Operation<machine Callback>(value: u64) -> u64
where machine Callback(value: u64) -> u64
requires value == value;
{ }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub machine register<machine Operation>()
where machine Operation<machine Callback>(value: u64) -> u64
where machine Callback(value: i64) -> u64
requires value == value;
{ }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("higher-order static-machine fixture should check");
        project_checked_package_review(&checked)
            .expect("higher-order static-machine contract should project")
    };
    let original = review(&original);
    let register = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("register"))
        .expect("register callable row");
    let [schema] = register.type_parameters() else {
        panic!("one outer static-machine parameter")
    };
    let PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
        signature,
    )) = schema.kind()
    else {
        panic!("outer structural contract")
    };
    let [inner] = signature.type_parameters() else {
        panic!("one nested static-machine parameter")
    };
    assert!(matches!(
        inner.kind(),
        PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
            _
        ))
    ));
    let register_row = original
        .canonical_rows()
        .expect("static-machine canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("register".len())
                    .any(|window| window == b"register")
        })
        .expect("register callable canonical row");
    assert!(
        register_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                location.role() == PackageReviewSourceLocationRole::ContractClause
                    && &original_source[start..end] == "requires"
            }))
    );

    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original static-machine encoding"),
        review(&renamed)
            .canonical_review_bytes()
            .expect("renamed static-machine encoding"),
        "renaming nested static-machine binders must not alter canonical review evidence",
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original static-machine encoding"),
        review(&changed)
            .canonical_review_bytes()
            .expect("changed static-machine encoding"),
        "changing a nested static-machine contract must alter canonical review evidence",
    );
}

#[test]
fn review_static_machine_nominal_contracts_require_exact_public_requirements() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Handler {
    machine call(value: i32) -> i32;
}
pub machine register<machine Selected>()
where machine Selected satisfies Handler::call;
{ }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("nominal static-machine fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public nominal static-machine contract should project");
    let register = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("register"))
        .expect("register callable row");
    let [selected] = register.type_parameters() else {
        panic!("one nominal static-machine parameter")
    };
    let PackageReviewTypeParameterKind::Machine(contract) = selected.kind() else {
        panic!("nominal static-machine parameter")
    };
    let Some((trait_identity, requirement_identity)) = contract.nominal() else {
        panic!("exact nominal requirement contract")
    };
    assert_eq!(trait_identity.path(), "Handler");
    assert!(requirement_identity.path().contains("Handler::call"));

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"trait Hidden {
    machine call(value: i32) -> i32;
}
pub machine register<machine Selected>()
where machine Selected satisfies Hidden::call;
{ }
"#,
    );
    hidden.write("build.omg", build);
    let diagnostics = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect_err("public authored selection should reject a private nominal requirement");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("public interface selects private trait `Hidden")
    }));
}

#[test]
fn review_static_machine_contracts_cover_public_proof_data() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Stream<machine Sample>
where machine Sample(index: u64) -> u64;
{
    case Empty;
    case More(tail: Stream<Sample>);
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
    .expect("public proof-data static-machine fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public proof-data static-machine contract should project");
    let [stream] = review.public_data() else {
        panic!("one public proof-data row")
    };
    let [sample] = stream.type_parameters() else {
        panic!("one proof-data static-machine parameter")
    };
    assert!(matches!(
        sample.kind(),
        PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
            _
        ))
    ));
}

#[test]
fn review_projects_binder_free_conformance_requirements_without_fabricating_evidence() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Ranked { }
pub trait Constraint<Element>
where Element satisfies Ranked
{ }
pub machine identity<Element>(value: Element) -> Element
where Element satisfies Ranked
{
    value
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
    .expect("unbound conformance-requirement fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("binder-free conformance requirement must project exactly");
    let identity = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("public identity row");
    let [bound] = identity.conformance_bounds() else {
        panic!("one exact binder-free conformance requirement")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(bound.trait_identity().path(), "Ranked");
    let constraint = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Constraint")
        .expect("public Constraint row");
    let [trait_bound] = constraint.conformance_bounds() else {
        panic!("one exact trait binder-free conformance requirement")
    };
    assert_eq!(trait_bound.binder_ordinal(), None);
    assert_eq!(trait_bound.subject_parameter(), 0);
    assert_eq!(trait_bound.trait_identity().path(), "Ranked");
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn review_projects_exact_selected_conformance_carrier_trait_and_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Marker<Tag> { }
pub data Tag { }
pub data Good { }
pub Primary: Good satisfies Marker<Tag> { }
pub machine accept<Element>(value: &Element)
where Element satisfies Good::Primary
{ }
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
    .expect("selected-conformance fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("exact non-generic selected conformance should project");
    let [conformance] = review.public_conformances() else {
        panic!("one package-owned public conformance row")
    };
    assert_eq!(conformance.identity().path(), "Primary");
    assert_eq!(conformance.lifetime_parameter_count(), 0);
    assert!(conformance.type_parameters().is_empty());
    let PackageReviewConformanceSubject::Nominal(subject) = conformance.subject() else {
        panic!("the public conformance has one nominal carrier")
    };
    assert_eq!(subject.path(), "Good");
    assert_eq!(conformance.interface().trait_identity().path(), "Marker");
    let [argument] = conformance.interface().arguments() else {
        panic!("one exact trait argument")
    };
    assert!(argument.canonical().contains("Tag"));
    assert!(conformance.interface().requirements().is_empty());
    assert!(review.canonical_rows().unwrap().iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicConformance
            && row.risk() == PackageReviewCanonicalRowRisk::Blocking
    }));
    let accept = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("accept"))
        .expect("public accept row");
    let [bound] = accept.conformance_bounds() else {
        panic!("one exact selected conformance requirement")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(
        bound
            .selected_conformance()
            .expect("selected conformance")
            .path(),
        "Primary"
    );
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("selected conformance has one exact nominal subject")
    };
    assert!(subject.canonical().contains("Good"));
    assert!(bound.selected_lifetime_arguments().is_empty());
    assert!(bound.selected_arguments().is_empty());
    assert_eq!(bound.trait_identity().path(), "Marker");
    assert_eq!(bound.arguments().len(), 1);
    assert!(bound.arguments()[0].canonical().contains("Tag"));
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn review_projects_complete_selected_generic_conformance_application() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Encodes<Output> { }
pub data Card { }
pub data Message { }
pub FullEncoding<'scope, Element, Output, const Rank: u64>:
    Element satisfies Encodes<Output>
{ }
pub machine inspect<'view, Element>(value: &'view Element)
where Element satisfies Card::FullEncoding<'view, Card, Message, 7>
{ }
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
    .expect("selected generic conformance fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("the complete selected conformance application must project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one exact selected generic conformance requirement")
    };
    assert_eq!(
        bound
            .selected_conformance()
            .expect("selected conformance declaration")
            .path(),
        "FullEncoding"
    );
    assert_eq!(bound.selected_lifetime_arguments(), [0]);
    let [
        PackageReviewContractStaticArgument::Type(card),
        PackageReviewContractStaticArgument::Type(message),
        PackageReviewContractStaticArgument::ConstInteger(rank),
    ] = bound.selected_arguments()
    else {
        panic!("selected application retains its exact categorized telescope")
    };
    assert!(card.canonical().contains("Card"));
    assert!(message.canonical().contains("Message"));
    assert_eq!(rank, "7");
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("selected application retains its instantiated subject")
    };
    assert!(subject.canonical().contains("Card"));
    assert_eq!(bound.trait_identity().path(), "Encodes");
    let [trait_argument] = bound.arguments() else {
        panic!("selected application retains its instantiated trait argument")
    };
    assert!(trait_argument.canonical().contains("Message"));
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn selected_generic_conformance_rows_alpha_normalize_and_detect_application_changes() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |lifetime: &str, output: &str| {
        format!(
            r#"pub trait Encodes<Output> {{ }}
pub data Card {{ }}
pub data First {{ }}
pub data Second {{ }}
pub Scoped<'scope, Element, Output>:
    Element satisfies Encodes<Output>
{{ }}
pub machine inspect<'{lifetime}, Element>(value: &'{lifetime} Element)
where Element satisfies Card::Scoped<'{lifetime}, Card, {output}>
{{ }}
"#
        )
    };
    let project = |source: String| {
        package.write("main.omg", &source);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("selected generic conformance comparison fixture should check");
        project_checked_package_review(&checked)
            .expect("selected generic conformance comparison fixture should project")
            .canonical_review_bytes()
            .expect("selected generic conformance comparison bytes")
    };

    let first = project(source("view", "First"));
    let renamed = project(source("borrow", "First"));
    let changed = project(source("view", "Second"));
    assert_eq!(first, renamed);
    assert_ne!(first, changed);
}

#[test]
fn selected_generic_conformance_rows_substitute_lifetimes_into_trait_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |first: &str, second: &str, selected: &str| {
        format!(
            r#"pub trait Borrows<Source> {{ }}
pub data Card {{ }}
pub data Borrow<'scope, Element> {{ value: &'scope Element; }}
pub Scoped<'scope, Element>:
    Element satisfies Borrows<Borrow<'scope, Element>>
{{ }}
pub machine inspect<'{first}, '{second}, Element>(
    value: &'{first} Element,
    other: &'{second} Element
)
where Element satisfies Card::Scoped<'{selected}, Card>
{{ }}
"#
        )
    };
    let project = |source: String| {
        package.write("main.omg", &source);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("lifetime-bearing selected conformance should check");
        project_checked_package_review(&checked)
            .expect("selected lifetime substitution should project")
    };

    let first = project(source("left", "right", "left"));
    let inspect = first
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one lifetime-bearing selected bound")
    };
    assert_eq!(bound.selected_lifetime_arguments(), [0]);
    let [trait_argument] = bound.arguments() else {
        panic!("one instantiated trait argument")
    };
    assert!(trait_argument.canonical().contains("Borrow"));
    let first_bytes = first.canonical_review_bytes().unwrap();

    let renamed = project(source("primary", "secondary", "primary"));
    assert_eq!(first_bytes, renamed.canonical_review_bytes().unwrap());
    let changed = project(source("left", "right", "right"));
    assert_ne!(first_bytes, changed.canonical_review_bytes().unwrap());
}

#[test]
fn review_alpha_normalizes_forwarded_selected_conformance_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Encodes<Output> { }
pub data Card { }
pub data Message { }
pub Encoding<Output, const Rank: u64>:
    Card satisfies Encodes<Output>
{ }
pub machine inspect<Output, const Rank: u64, Element>(value: &Element)
where Element satisfies Card::Encoding<Output, Rank>
{ }
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
    .expect("forwarded selected conformance arguments should check");
    let review = project_checked_package_review(&checked)
        .expect("forwarded selected conformance arguments should project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one forwarded selected conformance bound")
    };
    assert_eq!(
        bound.selected_arguments(),
        [
            PackageReviewContractStaticArgument::GenericTypeBinder(0),
            PackageReviewContractStaticArgument::GenericConstBinder(1),
        ]
    );
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("fixed selected subject is retained exactly")
    };
    assert!(subject.canonical().contains("Card"));
    assert_eq!(bound.arguments().len(), 1);
}

#[test]
fn review_projects_public_core_private_callback_slot_conformance() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::core::layout;

pub trait WindowProcedure {
    machine call();
}
pub data WndClassLayout { }

pub WndClassWindowProcedureSlot:
    WndClassLayout satisfies
        PrivateCallbackSlot<WindowProcedure::call>;
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
    .expect("public private-callback-slot fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("toolchain-owned requirement-identity conformance should project");
    let [conformance] = review.public_conformances() else {
        panic!("one public private-callback-slot conformance")
    };
    assert_eq!(conformance.identity().path(), "WndClassWindowProcedureSlot");
    let PackageReviewConformanceSubject::Nominal(subject) = conformance.subject() else {
        panic!("private callback slot must retain its nominal layout subject")
    };
    assert_eq!(subject.path(), "WndClassLayout");
    let interface = conformance.interface();
    assert_eq!(interface.trait_identity().path(), "PrivateCallbackSlot");
    assert!(matches!(
        interface.trait_identity().owner(),
        PackageReviewNominalOwner::ToolchainSource(_)
    ));
    let [argument] = interface.arguments() else {
        panic!("one exact callback requirement identity argument")
    };
    assert!(argument.canonical().contains("WindowProcedure"));
    assert!(argument.canonical().contains("call"));
    assert!(interface.requirements().is_empty());
    assert!(review.canonical_rows().unwrap().iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicConformance
            && row.risk() == PackageReviewCanonicalRowRisk::Blocking
    }));
}

#[test]
fn public_conformance_rows_are_alpha_normalized_and_exclude_private_realizations() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |binder: &str, value: i32| {
        format!(
            r#"pub trait Marker<Tag> {{
    machine Self::code(&self) -> i32;
}}
pub data Good {{ }}
pub Generic<{binder}>: {binder} satisfies Marker<{binder}> {{
    machine code(&self) -> i32 {{ {value} }}
}}
"#,
        )
    };
    package.write("main.omg", &source("Element", 1));
    let first = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("first generic public conformance should check");
    let first = project_checked_package_review(&first).expect("first row should project");
    let [shape] = first.public_conformances() else {
        panic!("one public generic conformance")
    };
    assert!(matches!(
        shape.subject(),
        PackageReviewConformanceSubject::TypeParameter(0)
    ));
    assert_eq!(shape.type_parameters().len(), 1);
    let [requirement] = shape.interface().requirements() else {
        panic!("one complete normalized requirement row")
    };
    assert!(requirement.requirement().path().contains("Marker::code"));
    let first_row = first
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("public conformance canonical row");

    package.write("main.omg", &source("Value", 2));
    let second = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("renamed telescope and changed private body should check");
    let second = project_checked_package_review(&second).expect("second row should project");
    let second_row = second
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("second public conformance canonical row");
    assert_eq!(first_row.key_bytes(), second_row.key_bytes());
    assert_eq!(first_row.canonical_bytes(), second_row.canonical_bytes());
}

#[test]
fn public_conformance_rows_alpha_normalize_lifetime_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |lifetime: &str| {
        format!(
            r#"pub trait Borrows<Source> {{ }}
pub data Borrow<'{lifetime}, Element> {{ value: &'{lifetime} Element; }}
pub Scoped<'{lifetime}, Element>:
    Element satisfies Borrows<Borrow<'{lifetime}, Element>>
{{ }}
"#
        )
    };

    package.write("main.omg", &source("scope"));
    let first = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("first lifetime-generic public conformance should check");
    let first = project_checked_package_review(&first)
        .expect("first lifetime-generic public conformance should project");
    let first_row = first
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("first lifetime-generic public conformance row");

    package.write("main.omg", &source("view"));
    let second = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("renamed lifetime-generic public conformance should check");
    let second = project_checked_package_review(&second)
        .expect("renamed lifetime-generic public conformance should project");
    let second_row = second
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("second lifetime-generic public conformance row");

    assert_eq!(first_row.key_bytes(), second_row.key_bytes());
    assert_eq!(first_row.canonical_bytes(), second_row.canonical_bytes());
}

#[test]
fn public_lifetime_conformances_project_inherited_requirement_substitutions() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |first: &str, second: &str, selected: &str, body: &str| {
        format!(
            r#"pub data Borrow<'{first}, Element> {{ value: &'{first} Element; }}
pub trait Parent<Source> {{
    machine absorb(value: Source);
}}
pub trait Child<Source>: Parent<Source> {{ }}
pub Scoped<'{first}, '{second}, Element>:
    Element satisfies Child<Borrow<'{selected}, Element>>
{{
    machine absorb(value: Borrow<'{selected}, Element>) {{ {body} }}
}}
"#
        )
    };
    let project = |source: String| {
        package.write("main.omg", &source);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("lifetime-generic inherited conformance should check");
        project_checked_package_review(&checked)
            .expect("inherited lifetime substitution should project exactly")
    };

    let first = project(source("left", "right", "left", ""));
    let [shape] = first.public_conformances() else {
        panic!("one inherited lifetime conformance")
    };
    let [requirement] = shape.interface().requirements() else {
        panic!("one inherited requirement")
    };
    assert_eq!(requirement.declaring_trait().path(), "Parent");
    assert_eq!(
        requirement.declaring_trait_arguments(),
        shape.interface().arguments()
    );
    let first_row = first
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("first inherited lifetime conformance row");

    let renamed = project(source(
        "primary",
        "secondary",
        "primary",
        "let private_value: i32 = 1;",
    ));
    let renamed_row = renamed
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("renamed inherited lifetime conformance row");
    assert_eq!(first_row.canonical_bytes(), renamed_row.canonical_bytes());

    let changed = project(source("left", "right", "right", ""));
    let changed_row = changed
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("changed inherited lifetime conformance row");
    assert_ne!(first_row.canonical_bytes(), changed_row.canonical_bytes());
}

#[test]
fn public_conformance_identity_is_independent_of_bodyless_or_closed_realization_form() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let source = |implementation: &str| {
        format!(
            r#"pub trait Marker {{ machine Self::touch(&self); }}
pub data Good {{ }}
{implementation}
"#
        )
    };
    package.write(
        "main.omg",
        &source("pub Primary: Good satisfies Marker;\nmachine Good::touch(&self) { }"),
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
    let bodyless = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("bodyless public conformance is valid static language input");
    let bodyless = project_checked_package_review(&bodyless)
        .expect("checked bodyless public conformance should project");
    let bodyless_row = bodyless
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("bodyless public conformance canonical row");

    package.write(
        "main.omg",
        &source("pub Primary: Good satisfies Marker { machine touch(&self) { } }"),
    );
    let closed = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("closed public conformance is valid static language input");
    let closed = project_checked_package_review(&closed)
        .expect("checked closed public conformance should project");
    let closed_row = closed
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("closed public conformance canonical row");

    assert_eq!(bodyless_row.key_bytes(), closed_row.key_bytes());
    assert_eq!(bodyless_row.canonical_bytes(), closed_row.canonical_bytes());
}
