use crate::support::*;

#[test]
fn public_callable_signatures_are_exact_and_lifetime_alpha_normalized() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'source [u8] { source }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub machine borrow<'origin, 'scratch>(
    source: &'origin [u8],
    temporary: &'scratch [u8]
) -> &'origin [u8] { source }
pub machine identity<Value [copy]>(value: Value) -> Value { value }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'temporary [u8] { temporary }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
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
        .expect("public callable signature fixture should check");
        project_checked_package_review(&checked).expect("callable signature review should close")
    };
    let original = review(&original);
    let renamed = review(&renamed);
    let changed = review(&changed);
    let borrow = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("borrow"))
        .expect("borrow callable row");
    assert_eq!(borrow.lifetime_parameter_count(), 2);
    assert_eq!(borrow.parameters().len(), 2);
    assert!(borrow.type_parameters().is_empty());
    assert!(!borrow.return_type().canonical().is_empty());
    assert!(
        borrow.parameters()[0]
            .type_identity()
            .canonical()
            .contains("compiler-type"),
        "source-free builtin u8 must use a closed compiler atom: {}",
        borrow.parameters()[0].type_identity().canonical(),
    );
    assert!(
        !borrow.parameters()[0]
            .type_identity()
            .canonical()
            .contains("unresolved-owner"),
        "compiler builtins must not remain unresolved in package review",
    );
    let identity = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("generic identity callable row");
    assert_eq!(identity.type_parameters().len(), 1);
    assert_eq!(identity.parameters().len(), 1);

    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming lifetime and type binders must not alter canonical review evidence",
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        changed.canonical_review_bytes().expect("changed encoding"),
        "changing the result's borrow relationship must alter canonical review evidence",
    );
}

#[test]
fn public_signatures_encode_closed_compiler_domains_and_exact_layout_schema() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Save {
    #1 value: u32;
}

pub machine inspect(
    number: f64 in Finite,
    token: u64 in Carry::AnyCpu,
    bytes: &[u8] in OmegaLayout<Save>
) { }
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
    .expect("closed compiler-domain fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("closed compiler domains should project without textual fallback");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("inspect callable review row");
    let [number, token, bytes] = inspect.parameters() else {
        panic!("three inspect parameters")
    };
    assert!(
        number
            .type_identity()
            .canonical()
            .contains("compiler-domain")
    );
    assert!(number.type_identity().canonical().contains("finite"));
    assert!(
        !number
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
    assert!(
        token
            .type_identity()
            .canonical()
            .contains("compiler-domain")
    );
    assert!(token.type_identity().canonical().contains("any-cpu"));
    assert!(
        !token
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
    assert!(bytes.type_identity().canonical().contains("omega-layout"));
    assert!(bytes.type_identity().canonical().contains("derived"));
    assert!(bytes.type_identity().canonical().contains("Save"));
    assert!(bytes.type_identity().canonical().contains("package-owner"));
    assert!(
        !bytes
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
}

#[test]
fn public_signatures_encode_structured_const_values_without_transport_or_display_text() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data UnitIndex { scale: u64; exponent: i32; }
data UnitIndices {}
const UnitIndices::Meters: UnitIndex = UnitIndex { scale: 1, exponent: 0 };

pub domain<Carrier, const Index: UnitIndex> Carrier::Quantity<Index>;
pub domain<Carrier, const Count: u64> Carrier::Counted<Count>;

pub data Reading {
    value: i64 in Quantity<UnitIndices::Meters>;
    count: i64 in Counted<7>;
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
    .expect("structured const package fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("structured const value should project through closed identity");
    let reading = review
        .public_data()
        .iter()
        .find(|data| data.identity().path().contains("Reading"))
        .expect("Reading review row");
    let field = |name| {
        reading
            .members()
            .iter()
            .find_map(|member| match member {
                PackageReviewDataMember::Field(field) if field.name() == name => Some(field),
                PackageReviewDataMember::Field(_) | PackageReviewDataMember::Variant { .. } => None,
            })
            .unwrap_or_else(|| panic!("Reading field `{name}`"))
    };
    let identity = field("value").type_identity().canonical();

    assert!(identity.contains("canonical-const"), "{identity}");
    assert!(identity.contains("encoding"), "{identity}");
    assert!(!identity.contains("#omega-const"), "{identity}");
    assert!(!identity.contains("UnitIndex {"), "{identity}");
    assert!(!identity.contains("unresolved-owner"), "{identity}");
    let integer = field("count").type_identity().canonical();
    assert!(integer.contains("integer-const"), "{integer}");
    assert!(integer.contains('7'), "{integer}");
    assert!(!integer.contains("unresolved-owner"), "{integer}");
}
