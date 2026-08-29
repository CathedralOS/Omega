use crate::support::*;

#[test]
fn review_projects_exact_nominal_record_and_case_constructors() {
    let project = |point_fields: &str, case: &str, case_fields: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Point [copy] {{ x: i32; y: i32; }}
pub data Outcome [copy] {{
    code: u64;
    case Success(value: u64);
    case Failure(value: u64);
}}
pub proposition has_point(value: Point);
pub proposition has_outcome(value: Outcome);
pub machine consume()
requires has_point(Point {{ {point_fields} }})
requires has_outcome(Outcome::{case} {{ {case_fields} }})
{{ }}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("nominal constructor contract fixture should check");
        project_checked_package_review(&checked)
            .expect("nominal constructors should project by exact declaration identity")
    };

    let original = project("x: 1, y: 2", "Success", "code: 3u64, value: 4u64");
    let consume = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public constructor consumer");
    assert_eq!(consume.contracts().len(), 2);
    let point = consume
        .contracts()
        .iter()
        .find_map(|contract| match contract.fact() {
            PackageReviewContractFact::Proposition(application)
                if application.declaration().path() == "has_point" =>
            {
                application.arguments().first()
            }
            _ => None,
        })
        .expect("record constructor argument");
    let PackageReviewContractExpression::Constructor { data, case, fields } = point else {
        panic!("one exact record constructor")
    };
    assert_eq!(data.path(), "Point");
    assert_eq!(
        data.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(case.is_none());
    assert_eq!(fields.len(), 2);
    assert!(fields[0].field().path() < fields[1].field().path());

    let outcome = consume
        .contracts()
        .iter()
        .find_map(|contract| match contract.fact() {
            PackageReviewContractFact::Proposition(application)
                if application.declaration().path() == "has_outcome" =>
            {
                application.arguments().first()
            }
            _ => None,
        })
        .expect("case constructor argument");
    let PackageReviewContractExpression::Constructor {
        data,
        case: Some(case),
        fields,
    } = outcome
    else {
        panic!("one exact sum-case constructor")
    };
    assert_eq!(data.path(), "Outcome");
    assert!(case.path().contains("Success"));
    assert_eq!(fields.len(), 2, "record and selected-case payload fields");

    let reordered = project("y: 2, x: 1", "Success", "value: 4u64, code: 3u64");
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "constructor field spelling order must canonicalize by exact field identity",
    );
    let changed_case = project("x: 1, y: 2", "Failure", "code: 3u64, value: 4u64");
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_case.canonical_review_bytes().unwrap(),
        "changing the exact selected case must change review identity",
    );
    let changed_value = project("x: 1, y: 2", "Success", "code: 3u64, value: 5u64");
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_value.canonical_review_bytes().unwrap(),
        "changing a constructor field value must change review identity",
    );

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"data Hidden [copy] { value: u64; }
pub proposition hidden(value: Hidden);
pub machine consume()
requires hidden(Hidden { value: 1u64 })
{ }
"#,
    );
    private.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&private.0),
    )
    .expect_err("a public contract must reject a private constructor before review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("public interface selects private data `Hidden`")
    }));
}

#[test]
fn review_projects_checked_index_and_range_contract_expressions() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub proposition selected(value: i32);
pub proposition window(values: &[i32]);
pub machine inspect(values: [i32; 2])
requires
    selected(values[0]),
    window(values[0..1])
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("indexed public contract fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("checked index and range expressions should project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public indexed-contract callable");
    let [selected, window] = inspect.contracts() else {
        panic!("two indexed requirements")
    };

    let PackageReviewContractFact::Proposition(selected) = selected.fact() else {
        panic!("selected proposition application")
    };
    let [
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        },
    ] = selected.arguments()
    else {
        panic!("selected argument is one indexed expression")
    };
    assert_eq!(*meaning, PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(**collection, PackageReviewContractExpression::Parameter(0));
    assert_eq!(
        **index,
        PackageReviewContractExpression::Integer("0".to_owned())
    );

    let PackageReviewContractFact::Proposition(window) = window.fact() else {
        panic!("window proposition application")
    };
    let [
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        },
    ] = window.arguments()
    else {
        panic!("window argument is one indexed expression")
    };
    assert_eq!(*meaning, PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(**collection, PackageReviewContractExpression::Parameter(0));
    assert_eq!(
        **index,
        PackageReviewContractExpression::Range {
            start: Some(Box::new(PackageReviewContractExpression::Integer(
                "0".to_owned(),
            ))),
            end: Some(Box::new(PackageReviewContractExpression::Integer(
                "1".to_owned(),
            ))),
            end_inclusive: false,
        }
    );
    let baseline_bytes = review
        .canonical_review_bytes()
        .expect("indexed contract review must encode canonically");

    package.write(
        "main.omg",
        r#"pub proposition selected(value: i32);
pub proposition window(values: &[i32]);
pub machine inspect(values: [i32; 2])
requires
    selected(values[1]),
    window(values[0..=1])
{ }
"#,
    );
    let changed = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("changed indexed public contract fixture should check");
    let changed_bytes = project_checked_package_review(&changed)
        .expect("changed checked index and range expressions should project")
        .canonical_review_bytes()
        .expect("changed indexed contract review must encode canonically");
    assert_ne!(baseline_bytes, changed_bytes);
}

#[test]
fn review_projects_exact_zero_value_targets_in_public_contracts() {
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
    let source = |binder: &str, family: &str| {
        format!(
            r#"pub data Optional<Element> {{ case #0 None; }}
pub data Alternate<Element> {{ case #0 None; }}
pub proposition zero_is_none<{binder}>() =
    zero_value<{family}<{binder}>>() == zero_value<{family}<{binder}>>();
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
        .expect("zero-value public proposition should check");
        project_checked_package_review(&checked)
            .expect("zero-value public proposition should project exactly")
    };

    let first = project(source("Item", "Optional"));
    let proposition = first
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "zero_is_none")
        .expect("public zero-value proposition row");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { left, .. },
    )) = proposition.body()
    else {
        panic!("one transparent zero-value equality")
    };
    let PackageReviewContractExpression::ZeroValue(target_type) = left.as_ref() else {
        panic!("the proof-only observation retains its exact target type")
    };
    assert!(target_type.canonical().contains("Optional"));
    let first_bytes = first.canonical_review_bytes().unwrap();

    let renamed = project(source("Value", "Optional"));
    assert_eq!(first_bytes, renamed.canonical_review_bytes().unwrap());
    let changed = project(source("Item", "Alternate"));
    assert_ne!(first_bytes, changed.canonical_review_bytes().unwrap());

    package.write(
        "main.omg",
        r#"data Hidden {}
pub proposition hidden_zero() =
    zero_value<Hidden>() == zero_value<Hidden>();
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect_err("a public zero-value target must not expose a private data declaration");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("public interface selects private data `Hidden`")
    }));
}
