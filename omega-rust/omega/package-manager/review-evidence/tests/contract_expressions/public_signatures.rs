use crate::support::*;
use compiler::CheckedCompileRequest;

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
        r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
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
        r#"pub data UnitIndex [copy] { scale: u64; exponent: i32; }
data UnitIndices {}
pub const UnitIndices::Meters: UnitIndex = UnitIndex { scale: 1, exponent: 0 };

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
        r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
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
