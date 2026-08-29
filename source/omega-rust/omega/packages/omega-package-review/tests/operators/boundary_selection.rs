use crate::support::*;

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
        omega_package_review::evidence::PackageReviewProviderSelectionAuthority::BuildOverride
    );
    assert_eq!(
        family.coverage(),
        omega_package_review::evidence::PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily
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
        .map(|provider| provider.plan_report_fingerprint())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        coordinates
            .iter()
            .map(|coordinate| coordinate.plan_report_fingerprint())
            .collect::<std::collections::BTreeSet<_>>(),
        selected_fingerprints
    );

    let canonical = review
        .canonical_review_bytes()
        .expect("family mapping should have canonical review encoding");
    assert!(!canonical.is_empty());
}
