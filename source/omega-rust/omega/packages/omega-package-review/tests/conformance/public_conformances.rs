use crate::support::*;

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
fn public_conformance_target_lifetimes_are_complete_alpha_normalized_identity() {
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
    let source = |left: &str, right: &str, selected: &str, body: &str| {
        format!(
            r#"pub trait Parent<'parent> {{
    machine absorb(value: &'parent i32);
}}
pub trait Child<'child>: Parent<'child> {{ }}
pub data Buffer {{ }}
pub BufferReads<'{left}, '{right}>:
    Buffer satisfies Child<'{selected}>
{{
    machine absorb(value: &'{selected} i32) {{ {body} }}
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
        .expect("explicit target-trait lifetime application should check");
        project_checked_package_review(&checked)
            .expect("checked target-trait lifetime application should project")
    };

    let first = project(source("left", "right", "left", ""));
    let [shape] = first.public_conformances() else {
        panic!("one target-lifetime conformance")
    };
    assert_eq!(shape.interface().lifetime_arguments(), &[0]);
    let [requirement] = shape.interface().requirements() else {
        panic!("one inherited parent requirement")
    };
    assert_eq!(requirement.declaring_trait().path(), "Parent");
    assert_eq!(requirement.declaring_trait_lifetime_arguments(), &[0]);
    let first_row = first
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("target-lifetime conformance row");

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
        .expect("renamed target-lifetime conformance row");
    assert_eq!(first_row.canonical_bytes(), renamed_row.canonical_bytes());

    let changed = project(source("left", "right", "right", ""));
    let changed_row = changed
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("changed target-lifetime conformance row");
    assert_ne!(first_row.canonical_bytes(), changed_row.canonical_bytes());
}

#[test]
fn public_conformance_target_lifetimes_fail_closed_before_review() {
    for (source, expected) in [
        (
            "pub trait Reads<'view> { }\npub data Buffer { }\npub Broken<'scope>: Buffer satisfies Reads { }\n",
            "expects 1 target-trait lifetime argument(s), got 0",
        ),
        (
            "pub trait Reads<'view> { }\npub data Buffer { }\npub Broken<'scope>: Buffer satisfies Reads<'foreign> { }\n",
            "does not name an in-scope conformance lifetime binder",
        ),
    ] {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect_err("incomplete target-trait lifetime application must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    }
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
