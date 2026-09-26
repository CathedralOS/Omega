use crate::support::*;
use compiler::CheckedCompileRequest;

#[test]
fn review_alpha_normalizes_forwarded_type_and_const_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed_type = TempPackage::new();
    let changed_const = TempPackage::new();
    let source = |first: &str,
                  second: &str,
                  left: &str,
                  right: &str,
                  selected_type: &str,
                  selected_const: &str| {
        format!(
            r#"pub machine tag<Value>() -> u64 {{ 0 }}
pub machine constant<const Value: u64>() -> u64 {{ 0 }}
pub machine generic_type<{first}, {second}>() -> u64
requires tag<{selected_type}>() == tag<{selected_type}>()
{{
    0
}}
pub machine generic_const<const {left}: u64, const {right}: u64>() -> u64
requires constant<{selected_const}>() == constant<{selected_const}>()
{{
    0
}}
"#,
        )
    };
    original.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "First", "Left"),
    );
    renamed.write(
        "main.omg",
        &source(
            "Primary",
            "Secondary",
            "Minimum",
            "Maximum",
            "Primary",
            "Minimum",
        ),
    );
    changed_type.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "Second", "Left"),
    );
    changed_const.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "First", "Right"),
    );
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    for package in [&original, &renamed, &changed_type, &changed_const] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("forwarded type and const contract arguments should check");
        project_checked_package_review(&checked)
            .expect("forwarded type and const binders have canonical review rows")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed_type = project(&changed_type);
    let changed_const = project(&changed_const);
    let static_arguments = |name: &str| {
        let callable = original
            .callables()
            .iter()
            .find(|callable| callable.identity().path() == name)
            .expect("generic callable");
        let [contract] = callable.contracts() else {
            panic!("one generic callable contract")
        };
        let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            right,
            ..
        }) = contract.fact()
        else {
            panic!("generic callable equality contract")
        };
        let PackageReviewContractExpression::Call {
            static_arguments, ..
        } = right.as_ref()
        else {
            panic!("generic callable contract call")
        };
        static_arguments.clone()
    };
    assert_eq!(
        static_arguments("generic_type"),
        [PackageReviewContractStaticArgument::GenericTypeBinder(0)]
    );
    assert_eq!(
        static_arguments("generic_const"),
        [PackageReviewContractStaticArgument::GenericConstBinder(0)]
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming forwarded type and const binders must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_type.canonical_review_bytes().unwrap(),
        "selecting a different forwarded type binder must change review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_const.canonical_review_bytes().unwrap(),
        "selecting a different forwarded const binder must change review identity",
    );
}

#[test]
fn review_projects_recursive_generic_data_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |nested_type: &str| {
        format!(
            r#"pub data Wrapper<Value> {{ value: Value; }}
pub machine tag<Value>() -> u64 {{ 0 }}
boundary machine trusted_tag() -> u64
ensures result == tag<Wrapper<{nested_type}>>();
"#,
        )
    };
    package.write("main.omg", &source("u64"));
    changed.write("main.omg", &source("i64"));
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("nested static type contract call should check");
        project_checked_package_review(&checked)
            .expect("a recursive generic data argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_tag = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_tag")
        .expect("trusted tag boundary callable");
    let [contract] = trusted_tag.contracts() else {
        panic!("one trusted-tag contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-tag equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::GenericType {
            base,
            lifetime_arguments,
            arguments,
        },
    ] = static_arguments.as_slice()
    else {
        panic!("one generic data static argument")
    };
    assert!(base.canonical().contains("Wrapper"));
    assert!(lifetime_arguments.is_empty());
    let [PackageReviewContractStaticArgument::Type(nested)] = arguments.as_slice() else {
        panic!("one nested concrete type argument")
    };
    assert!(nested.canonical().contains("u64"));
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("Wrapper<u64> contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("Wrapper<i64> contract encoding"),
        "changing a nested concrete type must change package-review identity",
    );
}

#[test]
fn review_projects_closed_generic_conformance_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected: &str| {
        format!(
            r#"pub trait Ranked {{}}
pub data Card {{}}
pub FieldOrder<Element>: Element satisfies Ranked {{}}
pub AlternateOrder<Element>: Element satisfies Ranked {{}}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 {{ 0 }}
boundary machine trusted() -> u64
ensures result == tag<Card, {selected}<Card>>();
"#,
        )
    };
    original.write("main.omg", &source("FieldOrder"));
    changed.write("main.omg", &source("AlternateOrder"));
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    original.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("closed generic conformance contract argument should check");
        assert_eq!(
            checked
                .facts
                .proof
                .contract_expression_static_conformance_applications
                .len(),
            1,
            "the exact proof-expression occurrence must own one closed application",
        );
        project_checked_package_review(&checked)
            .expect("the closed conformance application has a portable contract row")
    };
    let review = project(&original);
    let changed = project(&changed);
    let trusted = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted")
        .expect("trusted boundary callable");
    let [contract] = trusted.contracts() else {
        panic!("one trusted contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::Type(card),
        PackageReviewContractStaticArgument::ConformanceApplication {
            declaration,
            arguments,
            subject,
            trait_identity,
            trait_arguments,
        },
    ] = static_arguments.as_slice()
    else {
        panic!("type and closed-conformance static arguments")
    };
    assert!(card.canonical().contains("Card"));
    assert_eq!(declaration.path(), "FieldOrder");
    assert_eq!(trait_identity.path(), "Ranked");
    assert!(trait_arguments.is_empty());
    assert!(matches!(
        arguments.as_slice(),
        [PackageReviewContractStaticArgument::Type(argument)]
            if argument.canonical().contains("Card")
    ));
    assert!(matches!(
        subject.as_ref(),
        PackageReviewContractStaticArgument::Type(argument)
            if argument.canonical().contains("Card")
    ));
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "selecting another closed conformance application must change review identity",
    );
}

#[test]
fn review_projects_integer_const_conformance_arguments_without_aliasing_values() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let changed = TempPackage::new();
    let source = |rank: u64| {
        format!(
            r#"pub trait Ranked {{}}
pub data Card {{}}
pub FieldOrder<Element, const Rank: u64>: Element satisfies Ranked {{}}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 {{ 0 }}
boundary machine trusted() -> u64
ensures result == tag<Card, FieldOrder<Card, {rank}>>();
"#,
        )
    };
    original.write("main.omg", &source(7));
    changed.write("main.omg", &source(8));
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    original.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("integer-const conformance contract argument should check");
        project_checked_package_review(&checked)
            .expect("integer-const conformance contract argument should project")
    };

    let review = project(&original);
    let changed = project(&changed);
    let trusted = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted")
        .expect("trusted boundary callable");
    let [contract] = trusted.contracts() else {
        panic!("one trusted contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::Type(_),
        PackageReviewContractStaticArgument::ConformanceApplication {
            arguments, subject, ..
        },
    ] = static_arguments.as_slice()
    else {
        panic!("type and closed-conformance static arguments")
    };
    assert!(matches!(
        arguments.as_slice(),
        [
            PackageReviewContractStaticArgument::Type(argument),
            PackageReviewContractStaticArgument::ConstInteger(rank),
        ] if argument.canonical().contains("Card") && rank == "7"
    ));
    assert!(matches!(
        subject.as_ref(),
        PackageReviewContractStaticArgument::Type(argument)
            if argument.canonical().contains("Card")
    ));
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing an integer const in the selected conformance must change review identity",
    );
}

#[test]
fn review_projects_named_canonical_const_values_in_closed_contract_conformances() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let changed = TempPackage::new();
    let source = |rank: u64, marker: u8| {
        format!(
            r#"pub trait Ranked {{}}
pub data Card {{}}
pub data Marker [copy] {{ value: u8; }}
pub data Alternate [copy] {{ value: u64; }}
pub const RANK: u64 = {rank};
pub const MARKER: Marker = Marker {{ value: {marker} }};
pub const OTHER: Alternate = Alternate {{ value: 0 }};
pub FieldOrder<Element, const Rank: u64, const Mark: Marker>: Element satisfies Ranked {{}}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 {{ 0 }}
boundary machine trusted() -> u64
ensures result == tag<Card, FieldOrder<Card, RANK, MARKER>>();
"#,
        )
    };
    original.write("main.omg", &source(7, 1));
    changed.write("main.omg", &source(8, 2));
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    original.write("build.omg", build);
    changed.write("build.omg", build);
    let compile = |package: &TempPackage| {
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("named canonical const conformance arguments should check")
    };
    let checked = compile(&original);
    let changed_checked = compile(&changed);
    let retained = &checked
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .application;
    assert_eq!(retained.const_arguments.len(), 2);
    assert!(
        retained.const_arguments.iter().all(|argument| matches!(
            argument,
            typed_trees::typed_trees::ClosedConformanceConstArgument::Evaluated { .. }
        )),
        "checked closure must retain exact carriers and canonical values",
    );
    let review = project_checked_package_review(&checked)
        .expect("named canonical const conformance arguments should project");
    let changed_review = project_checked_package_review(&changed_checked)
        .expect("changed named canonical const conformance arguments should project");
    let trusted = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted")
        .expect("trusted boundary callable");
    let [contract] = trusted.contracts() else {
        panic!("one trusted contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::Type(_),
        PackageReviewContractStaticArgument::ConformanceApplication { arguments, .. },
    ] = static_arguments.as_slice()
    else {
        panic!("type and closed-conformance static arguments")
    };
    assert!(matches!(
        arguments.as_slice(),
        [
            PackageReviewContractStaticArgument::Type(_),
            PackageReviewContractStaticArgument::ConstInteger(rank),
            PackageReviewContractStaticArgument::ConstStructured {
                declared_type,
                canonical_value_encoding,
            },
        ] if rank == "7"
            && declared_type.canonical().contains("Marker")
            && canonical_value_encoding.starts_with("record")
    ));
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        changed_review.canonical_review_bytes().unwrap(),
        "changing named canonical values must change review identity",
    );

    let alternate_carrier = checked
        .const_declarations()
        .iter()
        .find(|declaration| checked.symbols.name(declaration.symbol) == "OTHER")
        .expect("OTHER declaration")
        .declared_type;
    let mut carrier_drift = checked.clone();
    let typed_trees::typed_trees::ClosedConformanceConstArgument::Evaluated {
        declared_carrier,
        ..
    } = &mut carrier_drift
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .application
        .const_arguments[1]
    else {
        panic!("named structured const must retain one evaluated argument")
    };
    *declared_carrier = alternate_carrier;
    let diagnostics = project_checked_package_review(&carrier_drift)
        .expect_err("post-check named const carrier drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retained checked occurrence disagrees with the authored application")
    }));

    let changed_rank_encoding = changed_checked
        .const_declarations()
        .iter()
        .find(|declaration| changed_checked.symbols.name(declaration.symbol) == "RANK")
        .and_then(|declaration| declaration.canonical_value_encoding.clone())
        .expect("changed RANK canonical value");
    let mut tampered = checked;
    let rank = tampered
        .typed
        .tables
        .const_declarations
        .iter()
        .find_map(|(handle, declaration)| {
            (tampered.symbols.name(declaration.symbol) == "RANK").then_some(handle)
        })
        .expect("RANK declaration");
    tampered
        .typed
        .tables
        .const_declarations
        .get_mut(rank)
        .canonical_value_encoding = Some(changed_rank_encoding);
    let diagnostics = project_checked_package_review(&tampered)
        .expect_err("post-check named const value drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retained checked occurrence disagrees with the authored application")
    }));
}

#[test]
fn review_alpha_normalizes_forwarded_const_binders_in_closed_contract_conformances() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    let source = |left: &str, right: &str, selected: &str| {
        format!(
            r#"pub trait Ranked {{}}
pub data Card {{}}
pub FieldOrder<Element, const Rank: u64>: Element satisfies Ranked {{}}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 {{ 0 }}
boundary machine trusted<const {left}: u64, const {right}: u64>() -> u64
ensures result == tag<Card, FieldOrder<Card, {selected}>>();
"#,
        )
    };
    original.write("main.omg", &source("Left", "Right", "Left"));
    renamed.write("main.omg", &source("First", "Second", "First"));
    changed.write("main.omg", &source("Left", "Right", "Right"));
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    for package in [&original, &renamed, &changed] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("forwarded-const conformance contract argument should check");
        project_checked_package_review(&checked)
            .expect("a forwarded const binder has an exact closed-conformance review row")
    };
    let review = project(&original);
    let renamed = project(&renamed);
    let changed = project(&changed);
    let trusted = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted")
        .expect("trusted boundary callable");
    let [contract] = trusted.contracts() else {
        panic!("one trusted contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::Type(_),
        PackageReviewContractStaticArgument::ConformanceApplication { arguments, .. },
    ] = static_arguments.as_slice()
    else {
        panic!("type and closed-conformance static arguments")
    };
    assert!(matches!(
        arguments.as_slice(),
        [
            PackageReviewContractStaticArgument::Type(_),
            PackageReviewContractStaticArgument::GenericConstBinder(0),
        ]
    ));
    assert_eq!(
        review.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming the forwarded const binder must preserve review identity",
    );
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "selecting another forwarded const binder must change review identity",
    );
}

#[test]
fn review_rejects_integer_const_conformance_occurrence_value_drift() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Ranked {}
pub data Card {}
pub FieldOrder<Element, const Rank: u64>: Element satisfies Ranked {}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 { 0 }
boundary machine trusted() -> u64
ensures result == tag<Card, FieldOrder<Card, 7>>();
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
    );
    let mut checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("integer-const conformance occurrence fixture should check");
    let retained = &mut checked
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .application
        .const_arguments[0];
    let typed_trees::typed_trees::ClosedConformanceConstArgument::Evaluated { value, .. } =
        retained
    else {
        panic!("integer literal must retain one evaluated const argument")
    };
    value.encoding = language_semantics::const_value::CanonicalConstIdentity::integer(
        value.type_name.clone(),
        8,
    )
    .encoding;
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("checked const-value drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retained checked occurrence disagrees with the authored application")
    }));
}

#[test]
fn review_rejects_type_erasure_of_machine_parameterized_conformance_targets() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait Callback { machine call(value: u64) -> u64; }
pub trait CallbackSlot<Element, machine Requirement> {}
pub data Card {}
pub Slot<Element>: Element satisfies CallbackSlot<Element, Callback::call> {}
pub machine tag<Element, Evidence: Element satisfies CallbackSlot<Element, Callback::call>>() -> u64 { 0 }
boundary machine trusted() -> u64
ensures result == tag<Card, Slot<Card>>();
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
    .expect("machine-parameterized conformance contract argument should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a machine target argument must not be mislabeled as a type");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("target trait is outside the type-only closed cohort")
    }));
}

#[test]
fn review_rejects_closed_contract_conformance_occurrence_drift() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Ranked {}
pub data Card {}
pub FieldOrder<Element>: Element satisfies Ranked {}
pub AlternateOrder<Element>: Element satisfies Ranked {}
pub machine tag<Element, Order: Element satisfies Ranked>() -> u64 { 0 }
boundary machine trusted() -> u64
ensures result == tag<Card, FieldOrder<Card>>();
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
    );
    let compile = || {
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("closed generic conformance occurrence fixture should check")
    };

    let mut missing = compile();
    missing
        .facts
        .proof
        .contract_expression_static_conformance_applications
        .clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("a missing checked occurrence row must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("with 0 exact checked occurrence rows; expected one")
    }));

    let mut duplicate = compile();
    let copied = duplicate
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .clone();
    duplicate
        .facts
        .proof
        .contract_expression_static_conformance_applications
        .push(copied);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("a duplicate checked occurrence row must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("with 2 exact checked occurrence rows; expected one")
    }));

    let mut redirected = compile();
    redirected
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .static_argument_position = 0;
    let diagnostics = project_checked_package_review(&redirected)
        .expect_err("a checked occurrence row redirected to another static slot must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("with 0 exact checked occurrence rows; expected one")
    }));

    let mut substituted = compile();
    substituted
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .application
        .declaration = symbols::SymbolHandle::invalid();
    let diagnostics = project_checked_package_review(&substituted)
        .expect_err("a substituted closed application must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retained checked occurrence disagrees with the authored application")
    }));

    let mut selection_drift = compile();
    let alternate = selection_drift
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "AlternateOrder")
        })
        .expect("alternate conformance declaration")
        .symbol;
    let expression = selection_drift
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .expression;
    let typed_trees::expression::ExpressionNode::Call(mut call) = selection_drift
        .expression_table
        .expression(expression)
        .clone()
    else {
        panic!("checked occurrence rejoins its contract call")
    };
    call.machine_arguments[1].symbol = alternate;
    let closed = typed_trees_to_checked_trees::close_conformance_application(
        &selection_drift.typed,
        &call.machine_arguments[1],
    )
    .expect("the alternate application also closes");
    *selection_drift
        .typed
        .expression_table
        .expression_mut(expression) = typed_trees::expression::ExpressionNode::Call(call);
    selection_drift
        .facts
        .proof
        .contract_expression_static_conformance_applications[0]
        .application = closed;
    let diagnostics = project_checked_package_review(&selection_drift)
        .expect_err("coordinated typed and checked drift must not bypass source selection custody");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("do not match their exact authored selections")
    }));
}
