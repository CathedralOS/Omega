mod support;

use support::*;

#[test]
fn review_projects_exact_public_callable_conformances_and_static_machine_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let satisfying = TempPackage::new();
    satisfying.write(
        "main.omg",
        r#"pub trait Handler<Element> { machine handle(value: Element) -> Element; }
pub machine handle(value: u32) -> u32 satisfies Handler<u32>::handle { value }
"#,
    );
    satisfying.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &satisfying.0.join("main.omg"),
        Some(target),
        package_inputs(&satisfying.0),
    )
    .expect("public satisfier fixture should check");
    let review = project_checked_package_review(&checked).expect("public conformance review");
    let handle = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("handle"))
        .expect("public handle row");
    let [conformance] = handle.conformances() else {
        panic!("one exact public callable conformance")
    };
    assert_eq!(conformance.trait_identity().path(), "Handler");
    assert!(conformance.requirement_identity().path().contains("handle"));
    assert_eq!(conformance.arguments().len(), 1);
    assert_eq!(conformance.alias(), None);

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"trait Hidden { machine handle(); }
pub machine handle() satisfies Hidden::handle { }
"#,
    );
    hidden.write("build.omg", build);
    let diagnostics = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect_err("compiler admission must reject a public satisfier of a private trait");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("private trait `Hidden`") })
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("private trait `Hidden::handle`")
    }));

    let generic = TempPackage::new();
    generic.write(
        "main.omg",
        r#"pub machine register<machine Selected>()
where machine Selected(value: bool) -> bool
requires value
crashes Abort
    value;
{ }
"#,
    );
    generic.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("public static-machine fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public static-machine contract should project exactly");
    let register = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("register"))
        .expect("public register row");
    let [parameter] = register.type_parameters() else {
        panic!("one static-machine parameter")
    };
    let PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
        signature,
    )) = parameter.kind()
    else {
        panic!("register must retain its structural static-machine contract")
    };
    assert!(signature.type_parameters().is_empty());
    assert_eq!(signature.parameters().len(), 1);
    assert_eq!(signature.contracts().len(), 1);
    assert_eq!(signature.published_crash().len(), 1);
}

#[test]
fn review_projects_exact_checked_operator_realization() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;

pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{
    transition { _ -> input }
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
    .expect("checked operator realization fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("checked operator realization should project exactly");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("public provider callable row");
    assert!(callable.conformances().is_empty());
    let [realization] = callable.operator_realizations() else {
        panic!("one exact checked operator realization")
    };
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "CheckedMath::identity")
        .expect("public operator declaration row");
    assert_eq!(realization.coordinate(), declaration.coordinate());
    assert_eq!(realization.alias(), None);
}

#[test]
fn review_projects_and_encodes_aliased_checked_operator_realization() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let project = |alias: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data CheckedMath {{}}
pub operator CheckedMath::identity(value: i32) -> i32;

pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity as {alias}
{{
    input
}}
"#,
            ),
        );
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("aliased checked operator realization fixture should check");
        project_checked_package_review(&checked)
            .expect("aliased checked operator realization should project exactly")
    };

    let selected = project("Selected");
    let alternate = project("Alternate");
    let selected_callable = selected
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("selected provider callable row");
    let alternate_callable = alternate
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("alternate provider callable row");
    let [selected_realization] = selected_callable.operator_realizations() else {
        panic!("one selected operator realization")
    };
    let [alternate_realization] = alternate_callable.operator_realizations() else {
        panic!("one alternate operator realization")
    };
    assert_eq!(selected_realization.alias(), Some("Selected"));
    assert_eq!(alternate_realization.alias(), Some("Alternate"));
    assert_eq!(
        selected_realization.coordinate(),
        alternate_realization.coordinate()
    );

    let callable_rows = |review: &CheckedPackageReviewProjection| {
        review
            .canonical_rows()
            .expect("aliased operator realization rows")
            .into_iter()
            .filter(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
            .collect::<Vec<_>>()
    };
    let selected_rows = callable_rows(&selected);
    let alternate_rows = callable_rows(&alternate);
    assert_eq!(selected_rows.len(), alternate_rows.len());
    assert!(
        selected_rows
            .iter()
            .zip(&alternate_rows)
            .all(|(left, right)| left.key_bytes() == right.key_bytes())
    );
    assert_eq!(
        selected_rows
            .iter()
            .zip(&alternate_rows)
            .filter(|(left, right)| left.canonical_bytes() != right.canonical_bytes())
            .count(),
        1,
        "changing only the alias must change only its callable value"
    );
}

#[test]
fn review_projects_fixed_token_checked_operator_realization_by_declaration_coordinate() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub operator - CheckedMath::subtract(left: i32, right: i32) -> i32;

pub machine provide_subtract(left: i32, right: i32) -> i32
satisfies CheckedMath::subtract
{
    transition { _ -> left }
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
    .expect("fixed-token checked operator realization fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("fixed-token checked operator realization should project exactly");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_subtract")
        .expect("public provider callable row");
    let [realization] = callable.operator_realizations() else {
        panic!("one exact fixed-token operator realization")
    };
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "CheckedMath::subtract")
        .expect("public fixed-token operator declaration row");
    assert_eq!(
        declaration.spelling(),
        Some(psi_language_core::OperatorSpelling::Subtract)
    );
    assert_eq!(realization.coordinate(), declaration.coordinate());
    assert_eq!(realization.alias(), None);
}

#[test]
fn review_joins_boundary_operator_realization_to_selected_provider() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub boundary operator CheckedMath::offset_zero(value: i32) -> i32;

pub data CheckedMathProvider {}
pub machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
satisfies CheckedMath::offset_zero
{
    transition { _ -> input }
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
    .expect("boundary operator provider fixture should check and select uniquely");
    let review = project_checked_package_review(&checked)
        .expect("selected boundary operator provider should project exactly");
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "CheckedMath::offset_zero")
        .expect("public boundary operator declaration row");
    assert!(declaration.is_boundary());
    assert_eq!(declaration.spelling(), None);
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "CheckedMathProvider::offset_zero_impl")
        .expect("public checked adapter callable row");
    let [realization] = callable.operator_realizations() else {
        panic!("one exact boundary operator realization")
    };
    assert_eq!(realization.coordinate(), declaration.coordinate());
    assert_eq!(realization.alias(), None);

    let [provider] = review.selected_providers() else {
        panic!("one selected boundary operator provider")
    };
    assert_eq!(
        provider.schema_declaration(),
        declaration.coordinate().identity()
    );
    let [provider_row] = provider.row_declarations() else {
        panic!("one selected boundary operator provider row")
    };
    assert_eq!(provider_row.realization(), callable.identity());
    assert_eq!(
        provider_row.requirement().owner(),
        declaration.coordinate().identity().owner()
    );
    assert_eq!(
        provider_row.requirement().path(),
        provider.schema().methods[0].requirement_identity
    );
    assert!(matches!(
        provider.rows()[0].binding,
        omega_effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
    ));
}

#[test]
fn review_keeps_named_boundary_operator_overloads_and_private_supply_exact() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub boundary operator CheckedMath::convert(value: i32) -> i32;
pub boundary operator CheckedMath::convert(value: u64) -> u64;

pub data I32Provider {}
pub machine I32Provider::convert(input: i32) -> i32
satisfies CheckedMath::convert
{ input }

data U64Provider {}
machine U64Provider::convert(input: u64) -> u64
satisfies CheckedMath::convert
{ 0 }
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
    .expect("boundary operator overload providers should select independently");
    let review = project_checked_package_review(&checked)
        .expect("boundary operator overload providers should project exactly");
    let overloads = review
        .public_operators()
        .iter()
        .filter(|operator| operator.coordinate().identity().path() == "CheckedMath::convert")
        .collect::<Vec<_>>();
    assert_eq!(overloads.len(), 2);
    assert_ne!(
        overloads[0].coordinate().parameter_dispatch(),
        overloads[1].coordinate().parameter_dispatch()
    );
    assert!(
        overloads
            .iter()
            .all(|operator| operator.coordinate().result_dispatch().is_empty())
    );

    let public_adapter = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "I32Provider::convert")
        .expect("public i32 adapter callable");
    let [public_realization] = public_adapter.operator_realizations() else {
        panic!("one exact public boundary operator realization")
    };
    assert!(overloads.iter().any(|operator| {
        operator.coordinate() == public_realization.coordinate()
            && operator.coordinate().parameter_dispatch().contains("i32")
    }));
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "U64Provider::convert"),
        "private selected adapters must not become public callable rows"
    );

    assert_eq!(review.selected_providers().len(), 2);
    assert!(
        review.selected_provider_families().is_empty(),
        "independently selected overloads must not be invented into one atomic family"
    );
    let selected_realizations = review
        .selected_providers()
        .iter()
        .flat_map(|provider| provider.row_declarations())
        .map(|row| row.realization().path())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        selected_realizations,
        std::collections::BTreeSet::from(["I32Provider::convert", "U64Provider::convert"])
    );
}

#[test]
fn review_retains_atomic_boundary_operator_family_selection() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub boundary operator CheckedMath::convert(value: u64) -> u64;
pub boundary operator CheckedMath::convert(value: i32) -> i32;

pub data ConvertProvider {}
pub machine ConvertProvider::convert_u64(input: u64) -> u64
satisfies CheckedMath::convert
{ input }
pub machine ConvertProvider::convert_i32(input: i32) -> i32
satisfies CheckedMath::convert
{ input }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_provider<CheckedMath::convert, ConvertProvider>();
}
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("one provider should cover the complete overloaded operator family");
    let review = project_checked_package_review(&checked)
        .expect("package review should retain the exact atomic family mapping");

    let [family] = review.selected_provider_families() else {
        panic!("one explicit selected provider family")
    };
    assert_eq!(family.family_identity().path(), "CheckedMath::convert");
    assert_eq!(family.provider_type_declaration().path(), "ConvertProvider");
    assert_eq!(family.target().target_name(), target);
    assert_eq!(
        family.authority(),
        omega_package_review::PackageReviewProviderSelectionAuthority::BuildOverride
    );
    assert_eq!(
        family.coverage(),
        omega_package_review::PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily
    );
    let coordinates = family.coordinates();
    assert_eq!(coordinates.len(), 2);
    assert!(coordinates[0].requirement_identity().contains("i32"));
    assert!(coordinates[1].requirement_identity().contains("u64"));
    assert!(
        coordinates
            .windows(2)
            .all(|pair| pair[0].requirement_identity() < pair[1].requirement_identity())
    );
    let selected_fingerprints = review
        .selected_providers()
        .iter()
        .map(|provider| provider.plan_fingerprint())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        coordinates
            .iter()
            .map(|coordinate| coordinate.plan_fingerprint())
            .collect::<std::collections::BTreeSet<_>>(),
        selected_fingerprints
    );

    let canonical = review
        .canonical_review_bytes()
        .expect("family mapping should have canonical review encoding");
    assert!(!canonical.is_empty());
}

#[test]
fn changing_checked_operator_realization_changes_only_the_callable_value() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |selected: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data FirstMath {{}}
pub data OtherMath {{}}
pub operator FirstMath::identity(value: i32) -> i32;
pub operator OtherMath::identity(value: i32) -> i32;

pub machine provide_identity(input: i32) -> i32
satisfies {selected}::identity
{{
    input
}}
"#,
            ),
        );
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("operator selection fixture should check");
        project_checked_package_review(&checked).expect("operator selection should project")
    };

    let first = compile("FirstMath");
    let other = compile("OtherMath");
    assert_eq!(first.public_operators(), other.public_operators());
    let first_callable = first
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("first provider callable");
    let other_callable = other
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("other provider callable");
    assert_ne!(
        first_callable.operator_realizations(),
        other_callable.operator_realizations()
    );

    let first_rows = first
        .canonical_rows()
        .expect("first operator realization rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .collect::<Vec<_>>();
    let other_rows = other
        .canonical_rows()
        .expect("other operator realization rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .collect::<Vec<_>>();
    assert_eq!(first_rows.len(), other_rows.len());
    assert!(
        first_rows
            .iter()
            .zip(&other_rows)
            .all(|(left, right)| left.key_bytes() == right.key_bytes())
    );
    assert_eq!(
        first_rows
            .iter()
            .zip(&other_rows)
            .filter(|(left, right)| left.canonical_bytes() != right.canonical_bytes())
            .count(),
        1,
        "only the provider callable value should change"
    );
}

#[test]
fn operator_realization_rejects_post_check_reselection() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data FirstMath {}
pub data StrongerMath {}
pub operator FirstMath::identity(value: i32) -> i32
ensures result == value;
pub operator StrongerMath::identity(value: i32) -> i32
ensures result == 0;

pub machine provide_identity(input: i32) -> i32
satisfies FirstMath::identity
ensures result == input
{
    transition { _ -> input }
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
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("weaker operator realization control fixture should check");
    let stronger = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("stronger operator declaration");
    let stronger_namespace = checked.typed.operator_path_members(stronger.name)[0].clone();
    let satisfies = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine")
        .satisfies;
    checked
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .name = stronger_namespace;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check redirection to a stronger operator must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));
}

#[test]
fn operator_realization_rejects_coordinated_typed_contract_tampering() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data FirstMath {}
pub data StrongerMath {}
pub operator FirstMath::identity(value: i32) -> i32
ensures result == value;
pub operator StrongerMath::identity(value: i32) -> i32
ensures result == 0;

pub machine provide_identity(input: i32) -> i32
satisfies FirstMath::identity
ensures result == input
{
    transition { _ -> input }
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
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("operator contract-custody fixture should check");
    let stronger = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("stronger operator declaration");
    let stronger_namespace = checked.typed.operator_path_members(stronger.name)[0].clone();
    let stronger_fact = checked.typed.operator_contracts(stronger)[0].facts.start();
    let psi_typed_trees::domain::ProofFact::Expression(stronger_expression) =
        checked.typed.proof_facts.get(stronger_fact)
    else {
        panic!("stronger operator expression contract")
    };
    let stronger_expression_node = checked
        .typed
        .expression_table
        .expression(*stronger_expression)
        .clone();
    let provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine");
    let provider_fact = checked.typed.machine_contracts(provider)[0].facts.start();
    let psi_typed_trees::domain::ProofFact::Expression(provider_expression) =
        checked.typed.proof_facts.get(provider_fact)
    else {
        panic!("provider expression contract")
    };
    let provider_expression = *provider_expression;
    let satisfies = provider.satisfies;

    checked
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .name = stronger_namespace;
    *checked
        .typed
        .expression_table
        .expression_mut(provider_expression) = stronger_expression_node;

    let mutated_provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("mutated provider machine");
    let mutated_operator = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("mutated stronger operator selection");
    psi_validation::validate_checked_operator_realization_contract(
        &checked.typed,
        mutated_provider,
        mutated_operator,
    )
    .expect("coordinated mutable typed state would pass contract revalidation alone");

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("checked custody must reject coordinated typed contract tampering");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));
}

#[test]
fn unsupported_checked_operator_realization_neighbors_remain_fail_closed() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"data CheckedMath {}
operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    private.write("build.omg", build);
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some(target),
        package_inputs(&private.0),
    )
    .expect_err("compiler admission must reject a public satisfier of a private operator");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("private operator `CheckedMath::identity`")
    }));

    let cases = [
        (
            "external",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
via Binding::Syscall(60);
"#,
            "one exact boundary operator",
        ),
        (
            "bodyless",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub boundary machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity;
"#,
            "without one checked implementation body",
        ),
        (
            "crash-contract",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32
crashes Trap;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
            "outcome-specific or crash contracts outside checked operator refinement",
        ),
        (
            "provider-crash",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
crashes Trap
{ input }
"#,
            "nonempty checked crash behavior outside checked operator refinement",
        ),
        (
            "fixed-token-boundary",
            r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::negate(value: i32) -> i32;
pub data CheckedMathProvider {}
pub machine CheckedMathProvider::negate_impl(input: i32) -> i32
satisfies CheckedMath::negate
{ input }
"#,
            "before checked-adapter token dispatch is represented",
        ),
    ];

    for (label, source, expected) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .unwrap_or_else(|diagnostics| panic!("{label} fixture should check: {diagnostics:?}"));
        let diagnostics = project_checked_package_review(&checked)
            .expect_err("unsupported operator realization must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{label}: {diagnostics:?}"
        );
    }

    let compile_admission_control = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("operator admission-drift control fixture should check")
    };

    let mut visibility_drift = compile_admission_control(
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    let operator_roots = visibility_drift.typed.roots.operators;
    visibility_drift
        .typed
        .tables
        .operators
        .span_mut_or_empty(operator_roots)[0]
        .is_public = false;
    let diagnostics = project_checked_package_review(&visibility_drift)
        .expect_err("post-check private-to-public operator drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut alias_drift = compile_admission_control(
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity as Selected
{ input }
"#,
    );
    let provider = alias_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("aliased provider machine");
    let satisfies = provider.satisfies;
    alias_drift
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .alias = None;
    let diagnostics = project_checked_package_review(&alias_drift)
        .expect_err("post-check removal of an operator alias must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut signature_drift = compile_admission_control(
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
machine u64_helper(value: u64) { }
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    let helper = signature_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "u64_helper")
        .expect("u64 helper machine");
    let helper_state = &signature_drift.typed.machine_states(helper)[0];
    let u64_type = signature_drift.typed.state_parameters(helper_state)[0].type_reference;
    let u64_node = signature_drift
        .typed
        .type_reference_table
        .type_reference(u64_type)
        .clone();
    let provider = signature_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("signature-drift provider");
    let provider_state = &signature_drift.typed.machine_states(provider)[0];
    let provider_type = signature_drift.typed.state_parameters(provider_state)[0].type_reference;
    let operator_type = signature_drift.typed.operator_parameters(
        signature_drift
            .typed
            .operators()
            .first()
            .expect("signature-drift operator"),
    )[0]
    .type_reference;
    signature_drift
        .typed
        .type_reference_table
        .substitute_node(provider_type, u64_node.clone());
    signature_drift
        .typed
        .type_reference_table
        .substitute_node(operator_type, u64_node);
    let diagnostics = project_checked_package_review(&signature_drift)
        .expect_err("coordinated post-check overload-shape drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut lifetime_drift = compile_admission_control(
        r#"pub data CheckedBorrow {}
pub operator CheckedBorrow::observe(first: &[u8], second: &[u8]);
pub machine provide_observe<'first, 'second>(
    first: &'first [u8],
    second: &'second [u8]
)
satisfies CheckedBorrow::observe
{ }
"#,
    );
    let provider = lifetime_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_observe")
        .expect("lifetime-drift provider");
    let state = &lifetime_drift.typed.machine_states(provider)[0];
    let parameters = lifetime_drift.typed.state_parameters(state);
    let first_type = parameters[0].type_reference;
    let second_type = parameters[1].type_reference;
    assert_ne!(
        first_type, second_type,
        "distinct lifetime-bearing type nodes"
    );
    let second_node = lifetime_drift
        .typed
        .type_reference_table
        .type_reference(second_type)
        .clone();
    lifetime_drift
        .typed
        .type_reference_table
        .substitute_node(first_type, second_node);
    let diagnostics = project_checked_package_review(&lifetime_drift)
        .expect_err("post-check lifetime-topology drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let generic = TempPackage::new();
    generic.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
machine generic<Element>() { }
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    generic.write("build.omg", build);
    let mut checked = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("generic-tamper control fixture should check");
    let type_parameters = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "generic")
        .expect("generic helper machine")
        .type_parameters;
    let mut forged_type_parameter = checked.clone();
    let operators = forged_type_parameter.typed.roots.operators;
    forged_type_parameter
        .typed
        .tables
        .operators
        .span_mut_or_empty(operators)[0]
        .type_parameters = type_parameters;
    forged_type_parameter
        .facts
        .operators
        .operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &forged_type_parameter.typed,
        );
    let diagnostics = project_checked_package_review(&forged_type_parameter)
        .expect_err("post-check generic operator realization must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("realizes generic or lifetime-parameterized operator")
    }));

    let operator = &checked.typed.operators()[0];
    let forged_lifetime = checked.typed.operator_path_members(operator.name)[0].clone();
    let operators = checked.typed.roots.operators;
    checked.typed.tables.operators.span_mut_or_empty(operators)[0]
        .lifetime_parameters
        .push(forged_lifetime);
    checked.facts.operators.operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &checked.typed,
        );
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check generic operator realization must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("realizes generic or lifetime-parameterized operator")
    }));

    let mut duplicate = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("duplicate-realization control fixture should check");
    let machine_index = duplicate
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine index");
    let machine_symbol = duplicate.typed.machines()[machine_index].symbol;
    let repeated = duplicate
        .typed
        .machine_trait_conformances(&duplicate.typed.machines()[machine_index])[0]
        .clone();
    let repeated_checked = duplicate
        .facts
        .operators
        .operator_realization_contracts
        .iter()
        .find(|row| row.machine_symbol() == machine_symbol)
        .expect("provider checked operator-realization contract")
        .clone();
    duplicate
        .facts
        .operators
        .operator_realization_contracts
        .push(repeated_checked);
    let machine_roots = duplicate.typed.roots.machines;
    let tables = &mut duplicate.typed.tables;
    let machine = &mut tables.machines.span_mut_or_empty(machine_roots)[machine_index];
    tables
        .machine_trait_conformances
        .append_to_span(&mut machine.satisfies, repeated);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate exact operator realizations must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("duplicate exact operator realization")
    }));
}
