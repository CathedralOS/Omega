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
    assert!(provider.grants().is_empty());
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
    assert!(
        review.boundary_application_realizations().is_empty(),
        "selecting a provider without an actual use must not invent an application realization"
    );
}

#[test]
fn actual_empty_applications_rejoin_and_deduplicate_checked_body_review() {
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

data FirstUse {}
machine FirstUse::exercise(&mut self) {
    let result: i32 = CheckedMath::offset_zero(70);
}

data SecondUse {}
machine SecondUse::exercise(&mut self) {
    let result: i32 = CheckedMath::offset_zero(71);
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
    .expect("actual monomorphic boundary applications should check");
    let review = project_checked_package_review(&checked)
        .expect("actual boundary applications should rejoin their checked realization");

    let [realization] = review.boundary_application_realizations() else {
        panic!("equal empty applications should deduplicate to one realization row")
    };
    assert_eq!(
        realization.application(),
        PackageReviewBoundaryApplication::Empty
    );
    assert_eq!(
        realization.role(),
        PackageReviewBoundaryApplicationRealizationRole::NongenericCheckedBody
    );
    assert_eq!(
        realization.operator_declaration().path(),
        "CheckedMath::offset_zero"
    );
    let PackageReviewBoundaryApplicationRealization::NongenericCheckedBody {
        realization_machine,
        realization_state,
        realization_contract_commitment,
    } = realization.realization()
    else {
        panic!("checked adapter must retain the checked-body payload")
    };
    assert_eq!(
        realization_machine.path(),
        "CheckedMathProvider::offset_zero_impl"
    );
    assert!(
        realization_state
            .path()
            .starts_with("CheckedMathProvider::offset_zero_impl::")
    );
    assert_ne!(realization.selected_plan_digest(), &[0; 32]);
    assert_ne!(realization_contract_commitment, &[0; 32]);

    let rows = review
        .canonical_rows()
        .expect("application realization should encode canonically");
    let row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::BoundaryApplicationRealization)
        .expect("one separate application-realization canonical row");
    assert_eq!(row.risk(), PackageReviewCanonicalRowRisk::Blocking);
    let locations = row
        .source()
        .authored_locations()
        .expect("actual applications should retain authored locations");
    assert_eq!(locations.len(), 2);
    assert!(locations.iter().all(|location| {
        location.role() == PackageReviewSourceLocationRole::BoundaryApplicationUse
    }));
    let recovered = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(row)
            .expect("application realization recovery envelope should encode"),
    )
    .expect("application realization recovery envelope should decode");
    assert_eq!(
        recovered.kind(),
        PackageReviewCanonicalRowKind::BoundaryApplicationRealization
    );
    assert!(
        recovered
            .source()
            .authored_locations()
            .expect("recovered actual application locations")
            .iter()
            .all(|location| {
                location.role() == PackageReviewSourceLocationRole::BoundaryApplicationUse
            })
    );
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
        r#"target windows_x86_64 { }
target linux_x86_64 { }
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
        omega_package_evidence::record::PackageReviewProviderSelectionAuthority::BuildOverride
    );
    assert_eq!(
        family.coverage(),
        omega_package_evidence::record::PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily
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

#[test]
fn review_selects_alpha_renamed_generic_checked_provider() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data GenericMath {}
pub boundary operator GenericMath::identity<Element>(value: Element) -> Element;
pub data GenericProvider {}
pub machine GenericProvider::identity<Value>(value: Value) -> Value
satisfies GenericMath::identity
{ value }
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
    .expect("alpha-renamed generic operator provider should select");
    let review = project_checked_package_review(&checked)
        .expect("generic provider declaration should project without universal coverage");
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "GenericMath::identity")
        .expect("generic boundary operator declaration");
    assert_eq!(declaration.type_parameters().len(), 1);
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "GenericProvider::identity")
        .expect("generic checked provider callable");
    assert_eq!(callable.type_parameters().len(), 1);
    assert_eq!(declaration.type_parameters(), callable.type_parameters());
    assert_eq!(
        declaration.parameters()[0].type_identity(),
        callable.parameters()[0].type_identity()
    );
    let [realization] = callable.operator_realizations() else {
        panic!("one exact generic operator realization")
    };
    assert_eq!(realization.coordinate(), declaration.coordinate());
    let [provider] = review.selected_providers() else {
        panic!("one selected generic provider")
    };
    assert_eq!(
        provider.schema_declaration().path(),
        "GenericMath::identity"
    );
    let [row] = provider.row_declarations() else {
        panic!("one selected generic provider row")
    };
    assert_eq!(row.realization().path(), "GenericProvider::identity");
    assert!(review.boundary_application_realizations().is_empty());
}

#[test]
fn review_accepts_generic_checked_provider_with_weaker_property_demand() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data GenericMath {}
pub operator GenericMath::identity<Element [copy]>(value: Element) -> Element;
pub machine provide_identity<Value>(value: Value) -> Value
satisfies GenericMath::identity
{ value }
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
    .expect("a provider with weaker property demands should satisfy the operator");
    let review = project_checked_package_review(&checked)
        .expect("property-demand weakening should remain explicit in package review");
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "GenericMath::identity")
        .expect("bounded generic operator declaration");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("weaker generic provider callable");
    assert_ne!(declaration.type_parameters(), callable.type_parameters());
    let [realization] = callable.operator_realizations() else {
        panic!("one refined generic operator realization")
    };
    assert_eq!(realization.coordinate(), declaration.coordinate());
}

#[test]
fn review_selects_alpha_renamed_const_generic_checked_provider() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data ConstMath {}
pub boundary operator ConstMath::identity<const Count: u64>(
    value: [u8; Count]
) -> [u8; Count];
pub data ConstProvider {}
pub machine ConstProvider::identity<const Length: u64>(
    value: [u8; Length]
) -> [u8; Length]
satisfies ConstMath::identity
{ value }
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
    .expect("alpha-renamed const-generic operator provider should select");
    let review = project_checked_package_review(&checked)
        .expect("const-generic provider declaration should project exactly");
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "ConstMath::identity")
        .expect("const-generic boundary operator declaration");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "ConstProvider::identity")
        .expect("const-generic checked provider callable");
    assert_eq!(declaration.type_parameters().len(), 1);
    assert_eq!(callable.type_parameters().len(), 1);
    assert_eq!(declaration.type_parameters(), callable.type_parameters());
    assert_eq!(
        declaration.parameters()[0].type_identity(),
        callable.parameters()[0].type_identity()
    );
    for authored_binder in ["Count", "Length"] {
        assert!(
            !callable.parameters()[0]
                .type_identity()
                .canonical()
                .contains(authored_binder)
        );
    }
    let [realization] = callable.operator_realizations() else {
        panic!("one exact const-generic operator realization")
    };
    assert_eq!(realization.coordinate(), declaration.coordinate());
    assert!(review.boundary_application_realizations().is_empty());
}

#[test]
fn checked_operator_static_telescope_mismatches_reject_before_review() {
    let Some(target) = host_target_name() else {
        return;
    };
    let cases = [
        (
            "category",
            r#"pub data GenericMath {}
pub operator GenericMath::identity<Element>(value: i32) -> i32;
pub machine provide_identity<const Count: u64>(value: i32) -> i32
satisfies GenericMath::identity
{ value }
"#,
        ),
        (
            "const carrier",
            r#"pub data GenericMath {}
pub operator GenericMath::identity<const Count: u64>(value: i32) -> i32;
pub machine provide_identity<const Count: i64>(value: i32) -> i32
satisfies GenericMath::identity
{ value }
"#,
        ),
        (
            "property bounds",
            r#"pub data GenericMath {}
pub operator GenericMath::identity<Element>(value: Element) -> Element;
pub machine provide_identity<Value [copy]>(value: Value) -> Value
satisfies GenericMath::identity
{ value }
"#,
        ),
    ];
    for (label, source) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
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
        .expect_err("mismatched operator/provider static telescopes must not check");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("does not match one exact overload of operator requirement")),
            "{label}: {diagnostics:?}"
        );
    }
}
