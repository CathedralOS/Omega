use crate::support::*;

#[test]
fn review_projects_binder_free_conformance_requirements_without_fabricating_evidence() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Ranked { }
pub trait Constraint<Element>
where Element satisfies Ranked
{ }
pub machine identity<Element>(value: Element) -> Element
where Element satisfies Ranked
{
    value
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
    .expect("unbound conformance-requirement fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("binder-free conformance requirement must project exactly");
    let identity = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("public identity row");
    let [bound] = identity.conformance_bounds() else {
        panic!("one exact binder-free conformance requirement")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(bound.trait_identity().path(), "Ranked");
    let constraint = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Constraint")
        .expect("public Constraint row");
    let [trait_bound] = constraint.conformance_bounds() else {
        panic!("one exact trait binder-free conformance requirement")
    };
    assert_eq!(trait_bound.binder_ordinal(), None);
    assert_eq!(trait_bound.subject_parameter(), 0);
    assert_eq!(trait_bound.trait_identity().path(), "Ranked");
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn review_projects_exact_selected_conformance_carrier_trait_and_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Marker<Tag> { }
pub data Tag { }
pub data Good { }
pub Primary: Good satisfies Marker<Tag> { }
pub machine accept<Element>(value: &Element)
where Element satisfies Good::Primary
{ }
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
    .expect("selected-conformance fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("exact non-generic selected conformance should project");
    let [conformance] = review.public_conformances() else {
        panic!("one package-owned public conformance row")
    };
    assert_eq!(conformance.identity().path(), "Primary");
    assert_eq!(conformance.lifetime_parameter_count(), 0);
    assert!(conformance.type_parameters().is_empty());
    let PackageReviewConformanceSubject::Nominal(subject) = conformance.subject() else {
        panic!("the public conformance has one nominal carrier")
    };
    assert_eq!(subject.path(), "Good");
    assert_eq!(conformance.interface().trait_identity().path(), "Marker");
    let [argument] = conformance.interface().arguments() else {
        panic!("one exact trait argument")
    };
    assert!(argument.canonical().contains("Tag"));
    assert!(conformance.interface().requirements().is_empty());
    assert!(review.canonical_rows().unwrap().iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicConformance
            && row.risk() == PackageReviewCanonicalRowRisk::Blocking
    }));
    let accept = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("accept"))
        .expect("public accept row");
    let [bound] = accept.conformance_bounds() else {
        panic!("one exact selected conformance requirement")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(
        bound
            .selected_conformance()
            .expect("selected conformance")
            .path(),
        "Primary"
    );
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("selected conformance has one exact nominal subject")
    };
    assert!(subject.canonical().contains("Good"));
    assert!(bound.selected_lifetime_arguments().is_empty());
    assert!(bound.selected_arguments().is_empty());
    assert_eq!(bound.trait_identity().path(), "Marker");
    assert_eq!(bound.arguments().len(), 1);
    assert!(bound.arguments()[0].canonical().contains("Tag"));
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn review_projects_complete_selected_generic_conformance_application() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Encodes<Output> { }
pub data Card { }
pub data Message { }
pub FullEncoding<'scope, Element, Output, const Rank: u64>:
    Element satisfies Encodes<Output>
{ }
pub machine inspect<'view, Element>(value: &'view Element)
where Element satisfies Card::FullEncoding<'view, Card, Message, 7>
{ }
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
    .expect("selected generic conformance fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("the complete selected conformance application must project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one exact selected generic conformance requirement")
    };
    assert_eq!(
        bound
            .selected_conformance()
            .expect("selected conformance declaration")
            .path(),
        "FullEncoding"
    );
    assert_eq!(bound.selected_lifetime_arguments(), [0]);
    let [
        PackageReviewContractStaticArgument::Type(card),
        PackageReviewContractStaticArgument::Type(message),
        PackageReviewContractStaticArgument::ConstInteger(rank),
    ] = bound.selected_arguments()
    else {
        panic!("selected application retains its exact categorized telescope")
    };
    assert!(card.canonical().contains("Card"));
    assert!(message.canonical().contains("Message"));
    assert_eq!(rank, "7");
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("selected application retains its instantiated subject")
    };
    assert!(subject.canonical().contains("Card"));
    assert_eq!(bound.trait_identity().path(), "Encodes");
    let [trait_argument] = bound.arguments() else {
        panic!("selected application retains its instantiated trait argument")
    };
    assert!(trait_argument.canonical().contains("Message"));
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn selected_generic_conformance_rows_alpha_normalize_and_detect_application_changes() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |lifetime: &str, output: &str| {
        format!(
            r#"pub trait Encodes<Output> {{ }}
pub data Card {{ }}
pub data First {{ }}
pub data Second {{ }}
pub Scoped<'scope, Element, Output>:
    Element satisfies Encodes<Output>
{{ }}
pub machine inspect<'{lifetime}, Element>(value: &'{lifetime} Element)
where Element satisfies Card::Scoped<'{lifetime}, Card, {output}>
{{ }}
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
        .expect("selected generic conformance comparison fixture should check");
        project_checked_package_review(&checked)
            .expect("selected generic conformance comparison fixture should project")
            .canonical_review_bytes()
            .expect("selected generic conformance comparison bytes")
    };

    let first = project(source("view", "First"));
    let renamed = project(source("borrow", "First"));
    let changed = project(source("view", "Second"));
    assert_eq!(first, renamed);
    assert_ne!(first, changed);
}

#[test]
fn selected_generic_conformance_rows_substitute_lifetimes_into_trait_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |first: &str, second: &str, selected: &str| {
        format!(
            r#"pub trait Borrows<Source> {{ }}
pub data Card {{ }}
pub data Borrow<'scope, Element> {{ value: &'scope Element; }}
pub Scoped<'scope, Element>:
    Element satisfies Borrows<Borrow<'scope, Element>>
{{ }}
pub machine inspect<'{first}, '{second}, Element>(
    value: &'{first} Element,
    other: &'{second} Element
)
where Element satisfies Card::Scoped<'{selected}, Card>
{{ }}
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
        .expect("lifetime-bearing selected conformance should check");
        project_checked_package_review(&checked)
            .expect("selected lifetime substitution should project")
    };

    let first = project(source("left", "right", "left"));
    let inspect = first
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one lifetime-bearing selected bound")
    };
    assert_eq!(bound.selected_lifetime_arguments(), [0]);
    let [trait_argument] = bound.arguments() else {
        panic!("one instantiated trait argument")
    };
    assert!(trait_argument.canonical().contains("Borrow"));
    let first_bytes = first.canonical_review_bytes().unwrap();

    let renamed = project(source("primary", "secondary", "primary"));
    assert_eq!(first_bytes, renamed.canonical_review_bytes().unwrap());
    let changed = project(source("left", "right", "right"));
    assert_ne!(first_bytes, changed.canonical_review_bytes().unwrap());
}

#[test]
fn review_alpha_normalizes_forwarded_selected_conformance_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Encodes<Output> { }
pub data Card { }
pub data Message { }
pub Encoding<Output, const Rank: u64>:
    Card satisfies Encodes<Output>
{ }
pub machine inspect<Output, const Rank: u64, Element>(value: &Element)
where Element satisfies Card::Encoding<Output, Rank>
{ }
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
    .expect("forwarded selected conformance arguments should check");
    let review = project_checked_package_review(&checked)
        .expect("forwarded selected conformance arguments should project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one forwarded selected conformance bound")
    };
    assert_eq!(
        bound.selected_arguments(),
        [
            PackageReviewContractStaticArgument::GenericTypeBinder(0),
            PackageReviewContractStaticArgument::GenericConstBinder(1),
        ]
    );
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("fixed selected subject is retained exactly")
    };
    assert!(subject.canonical().contains("Card"));
    assert_eq!(bound.arguments().len(), 1);
}

#[test]
fn package_review_closes_over_a_private_named_dynamic_selection_without_publishing_it() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Shape {
    machine code(&self) -> i32;
}
pub data Item { value: i32; }
Primary: Item satisfies Shape {
    machine code(&self) -> i32 { self.value }
}
pub data Main { item: Item; }
pub machine Main::inspect(&self) -> i32 {
    let erased: &dyn Shape = &self.item as &dyn Item::Primary;
    let result: i32 = erased.code();
    transition { _ -> result }
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
    .expect("private named dynamic selection should check in package-aware compilation");
    let [dynamic_selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
        panic!("one exact checked dynamic conformance selection expected")
    };
    let selected_conformance = dynamic_selection
        .conformance
        .expect("named conformance selection");
    let [dynamic_row] = dynamic_selection.rows.as_slice() else {
        panic!("one exact checked dynamic conformance row expected")
    };
    assert_ne!(dynamic_row.realization_machine, dynamic_row.requirement);

    let conformance_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind()
                == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
                && matches!(
                    selection.target(),
                    psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == selected_conformance
                )
        })
        .collect::<Vec<_>>();
    let [selection] = conformance_selections.as_slice() else {
        panic!("one authored named dynamic conformance selection expected")
    };
    assert_eq!(
        selection.exposure(),
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
    );

    let review = project_checked_package_review(&checked)
        .expect("ordinary package review should close over the supported dynamic body");
    assert!(
        review.public_conformances().is_empty(),
        "a body-local private conformance must not become public API"
    );
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    assert!(matches!(
        inspect.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody { realized, concrete }
            if realized.is_empty() && concrete.is_empty()
    ));
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}
