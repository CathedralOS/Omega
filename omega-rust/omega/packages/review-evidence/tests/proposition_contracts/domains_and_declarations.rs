use crate::support::*;
use compiler::CheckedCompileRequest;

#[test]
fn review_projects_structural_propositions_and_alpha_normalizes_their_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub proposition equivalent<Element>(left: Element, right: Element);
pub machine compare<Value>(left: Value, right: Value)
requires equivalent<Value>(left, right)
{ }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub proposition equivalent<Item>(left: Item, right: Item);
pub machine compare<Compared>(left: Compared, right: Compared)
requires equivalent<Compared>(left, right)
{ }
"#,
    );
    let build = r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);

    let project = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("generic proposition fixture should check");
        project_checked_package_review(&checked).expect("generic proposition review")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let compare = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("compare"))
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one proposition contract")
    };
    assert_eq!(contract.evidence_lane_position(), None);
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("exact proposition application")
    };
    assert_eq!(application.declaration().path(), "equivalent");
    let [binder] = application.binders() else {
        panic!("one proposition binder")
    };
    assert_eq!(binder.kind(), &PackageReviewPropositionBinderKind::Type);
    let [argument] = application.binder_arguments() else {
        panic!("one proposition binder argument")
    };
    assert_eq!(
        argument.value(),
        &PackageReviewPropositionBinderValue::GenericBinder(0)
    );
    assert_eq!(application.parameter_types().len(), 2);
    assert_eq!(
        application.arguments(),
        [
            PackageReviewContractExpression::Parameter(0),
            PackageReviewContractExpression::Parameter(1),
        ]
    );
    assert_eq!(
        application.evidence(),
        &PackageReviewPropositionEvidence::FactOnly
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming callable and proposition binders must not alter package evidence",
    );
}

#[test]
fn review_projects_unused_public_consts_with_exact_type_and_value_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }
"#;
    let project = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("public const declaration should check");
        project_checked_package_review(&checked).expect("public const review")
    };

    let original_source = "pub const LIMIT: u64 = 4;\nconst HIDDEN_LIMIT: u64 = 2;\n";
    let original = project(original_source);
    let changed_value = project("pub const LIMIT: u64 = 5;\n");
    let changed_type = project("pub const LIMIT: u32 = 4;\n");
    let relocated = project("\n\npub const LIMIT: u64 = 4;\n");

    let [limit] = original.public_consts() else {
        panic!("private consts must stay out of public compatibility rows");
    };
    assert_eq!(limit.identity().path(), "LIMIT");
    assert!(limit.declared_type().canonical().contains("u64"));
    assert!(!limit.canonical_value_encoding().is_empty());
    let rows = original
        .canonical_rows()
        .expect("canonical public const rows");
    let const_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConst)
        .collect::<Vec<_>>();
    assert_eq!(const_rows.len(), 1);
    assert_eq!(
        const_rows[0].risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    let locations = const_rows[0]
        .source()
        .authored_locations()
        .expect("public const source locations");
    assert_eq!(locations.len(), 2);
    let source_for_role = |role| {
        let location = locations
            .iter()
            .find(|location| location.role() == role)
            .expect("exact public const source role");
        let start = usize::try_from(location.start_byte()).unwrap();
        let end = usize::try_from(location.end_byte()).unwrap();
        &original_source[start..end]
    };
    assert_eq!(
        source_for_role(PackageReviewSourceLocationRole::Declaration),
        "LIMIT"
    );
    assert_eq!(
        source_for_role(PackageReviewSourceLocationRole::ConstInitializer),
        "4"
    );
    let original_initializer_start = locations
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::ConstInitializer)
        .unwrap()
        .start_byte();
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_value.canonical_review_bytes().unwrap(),
        "changing a public const value must change package compatibility",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_type.canonical_review_bytes().unwrap(),
        "changing a public const declared type must change package compatibility",
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        relocated.canonical_review_bytes().unwrap(),
        "relocating identical const semantics must not change canonical review identity",
    );
    let relocated_row = relocated
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConst)
        .expect("relocated public const row");
    let relocated_initializer = relocated_row
        .source()
        .authored_locations()
        .unwrap()
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::ConstInitializer)
        .expect("relocated initializer location");
    assert_eq!(
        relocated_initializer.start_byte(),
        original_initializer_start + 2
    );
}
