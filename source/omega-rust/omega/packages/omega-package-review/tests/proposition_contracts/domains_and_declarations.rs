use crate::support::*;

#[test]
fn review_projects_exact_public_domain_membership_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain u64::Trusted;

pub machine consume(value: u64)
requires value in u64::Trusted
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
    .expect("checked public-domain membership requirement should check");
    let review = project_checked_package_review(&checked).expect("membership contract review");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public callable row");
    let [contract] = callable.contracts() else {
        panic!("one exact membership contract")
    };
    let PackageReviewContractFact::Membership { value, domain } = contract.fact() else {
        panic!("exact membership row")
    };
    assert_eq!(*value, PackageReviewContractExpression::Parameter(0));
    assert_eq!(domain.path(), "u64::Trusted");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let membership_row = review
        .canonical_rows()
        .expect("membership canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("consume".len())
                    .any(|window| window == b"consume")
        })
        .expect("public consume callable row");
    assert!(
        membership_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ContractClause
            }))
    );

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"domain u64::Hidden;
pub machine consume(value: u64)
requires value in u64::Hidden
{ }
"#,
    );
    hidden.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect_err("ordinary visibility must reject a private domain in a public contract");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `u64::Hidden`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

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
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);

    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
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
fn review_projects_unused_public_proposition_declarations_without_granting_facts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let source = r#"pub proposition ready();
pub proposition reflexive(value: i32) = value == value;
proposition hidden();
"#;
    package.write("main.omg", source);
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
    .expect("public proposition declarations should check");
    assert!(
        checked
            .facts
            .proof
            .proposition_vocabulary
            .applications
            .is_empty(),
        "publishing a bodyless proposition declaration must not manufacture an application fact"
    );

    let review = project_checked_package_review(&checked).expect("public proposition review");
    assert_eq!(review.public_propositions().len(), 2);
    let ready = review
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "ready")
        .expect("unused public primitive proposition row");
    assert_eq!(ready.body(), &PackageReviewPublicPropositionBody::Primitive);
    let reflexive = review
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "reflexive")
        .expect("public transparent proposition row");
    assert!(matches!(
        reflexive.body(),
        PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
            PackageReviewContractExpression::Binary { .. }
        ))
    ));
    let rows = review
        .canonical_rows()
        .expect("canonical public proposition rows");
    let proposition_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicProposition)
        .count();
    assert_eq!(
        proposition_rows, 2,
        "private propositions stay out of public API rows"
    );
    let reflexive_row = rows
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicProposition
                && row
                    .key_bytes()
                    .windows("reflexive".len())
                    .any(|window| window == b"reflexive")
        })
        .expect("transparent proposition row");
    let locations = reflexive_row
        .source()
        .authored_locations()
        .expect("transparent proposition source custody");
    let formula = locations
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::PropositionFormula)
        .expect("transparent proposition formula location");
    let start = usize::try_from(formula.start_byte()).unwrap();
    let end = usize::try_from(formula.end_byte()).unwrap();
    assert_eq!(&source[start..end], "value == value");
    let ready_row = rows
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicProposition
                && row
                    .key_bytes()
                    .windows("ready".len())
                    .any(|window| window == b"ready")
        })
        .expect("primitive proposition row");
    assert!(
        ready_row
            .source()
            .authored_locations()
            .unwrap()
            .iter()
            .all(|location| location.role() != PackageReviewSourceLocationRole::PropositionFormula)
    );
}

#[test]
fn review_projects_unused_public_consts_with_exact_type_and_value_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let project = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
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
