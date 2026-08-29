use crate::support::*;

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
