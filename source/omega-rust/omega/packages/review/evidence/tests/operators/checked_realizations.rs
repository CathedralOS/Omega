use crate::support::*;

#[test]
fn review_projects_exact_public_callable_conformances_and_static_machine_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
