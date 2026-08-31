use crate::support::*;

#[test]
fn review_projects_exact_concrete_type_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected_type: &str| {
        format!(
            r#"pub machine tag<Value>() -> u64 {{ 0 }}
boundary machine trusted_zero() -> u64
ensures result == tag<{selected_type}>();
"#,
        )
    };
    package.write("main.omg", &source("u64"));
    changed.write("main.omg", &source("i64"));
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free static type contract call should check");
        project_checked_package_review(&checked)
            .expect("a direct concrete type argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_zero = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_zero")
        .expect("trusted boundary callable");
    let [contract] = trusted_zero.contracts() else {
        panic!("one trusted-zero contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-zero equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [PackageReviewContractStaticArgument::Type(identity)] = static_arguments.as_slice() else {
        panic!("one exact concrete type argument")
    };
    assert!(identity.canonical().contains("u64"));
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("u64 static-type contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("i64 static-type contract encoding"),
        "changing an exact concrete type selection must change package-review identity",
    );
}

#[test]
fn review_projects_canonical_integer_const_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected_value: &str| {
        format!(
            r#"pub machine constant<const Value: u64>() -> u64 {{ 7 }}
boundary machine trusted_constant() -> u64
ensures result == constant<{selected_value}>();
"#,
        )
    };
    package.write("main.omg", &source("0x07"));
    changed.write("main.omg", &source("0x08"));
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free const-generic contract call should check");
        project_checked_package_review(&checked)
            .expect("a direct integer const argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_constant = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_constant")
        .expect("trusted const boundary callable");
    let [contract] = trusted_constant.contracts() else {
        panic!("one trusted-constant contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-constant equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("const-generic call")
    };
    assert_eq!(
        static_arguments,
        &[PackageReviewContractStaticArgument::ConstInteger(
            "0x7".to_owned()
        )]
    );
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("0x7 static-const contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("0x8 static-const contract encoding"),
        "changing an exact const selection must change package-review identity",
    );
}

#[test]
fn review_projects_named_const_static_arguments_by_value_with_exact_source_custody() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |value: u64| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub const LIMIT: u64 = {value};
pub const OTHER: u64 = 9;
pub machine constant<const First: u64, const Second: u64>() -> u64 {{ 0 }}
boundary machine trusted_constant() -> u64
ensures result == constant<LIMIT, OTHER>();
"#,
            ),
        );
        package.write("build.omg", build);
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("named const static contract argument should check")
    };

    let checked = compile(7);
    let const_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            matches!(
                selection.target(),
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if checked.symbols.get(target.selected_symbol()).kind
                        == psi_symbols::SymbolKind::Const
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        const_selections.len(),
        2,
        "each named const static argument must retain one declaration selection"
    );
    assert!(const_selections.iter().all(|selection| {
        selection.kind()
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument
            && selection.exposure()
                == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
    }));
    assert!(checked.authored_declaration_selections().all_finalized());

    let review = project_checked_package_review(&checked)
        .expect("named const static argument should use the existing canonical value row");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_constant")
        .expect("trusted const callable");
    let [contract] = callable.contracts() else {
        panic!("one trusted-constant contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-constant equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("named const call")
    };
    assert_eq!(
        static_arguments,
        &[
            PackageReviewContractStaticArgument::ConstInteger("7".to_owned()),
            PackageReviewContractStaticArgument::ConstInteger("9".to_owned()),
        ]
    );
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        project_checked_package_review(&compile(8))
            .unwrap()
            .canonical_review_bytes()
            .unwrap(),
        "changing the selected const declaration value must change review identity",
    );

    let mut tampered = compile(7);
    let other = tampered
        .const_declarations()
        .iter()
        .map(|declaration| declaration.symbol)
        .find(|symbol| tampered.symbols.name(*symbol) == "OTHER")
        .expect("OTHER const symbol");
    let call_expression = tampered
        .typed
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| match node {
            psi_typed_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "constant"
                    && call.machine_arguments.len() == 2
                    && call.machine_arguments[0].symbol.is_valid()
                    && tampered.symbols.get(call.machine_arguments[0].symbol).kind
                        == psi_symbols::SymbolKind::Const =>
            {
                Some(expression)
            }
            _ => None,
        })
        .expect("named const contract call expression");
    let psi_typed_trees::expression::ExpressionNode::Call(call) = tampered
        .typed
        .expression_table
        .expression_mut(call_expression)
    else {
        unreachable!("selected call expression changed variant")
    };
    call.machine_arguments[0].symbol = other;
    let diagnostics = project_checked_package_review(&tampered)
        .expect_err("post-check named const selection drift must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("do not match their exact authored static-argument selections")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn review_projects_named_boolean_consts_with_exact_carrier_and_canonical_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |value: bool| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub const ENABLED: bool = {value};
pub const OTHER: u64 = 1;
pub machine selected<const Value: bool>() -> bool {{ true }}
boundary machine trusted_enabled() -> bool
ensures result == selected<ENABLED>();
"#,
            ),
        );
        package.write("build.omg", build);
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("named Boolean const static contract argument should check")
    };
    let static_arguments = |review: &CheckedPackageReviewProjection| {
        let callable = review
            .callables()
            .iter()
            .find(|callable| callable.identity().path() == "trusted_enabled")
            .expect("trusted Boolean callable");
        let [contract] = callable.contracts() else {
            panic!("one trusted-Boolean contract")
        };
        let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            right,
            ..
        }) = contract.fact()
        else {
            panic!("trusted-Boolean equality contract")
        };
        let PackageReviewContractExpression::Call {
            static_arguments, ..
        } = right.as_ref()
        else {
            panic!("named Boolean const call")
        };
        static_arguments.clone()
    };

    let checked = compile(true);
    let review = project_checked_package_review(&checked)
        .expect("named Boolean const should have canonical package-review custody");
    assert_eq!(
        static_arguments(&review),
        vec![PackageReviewContractStaticArgument::ConstBoolean(true)]
    );
    let false_review = project_checked_package_review(&compile(false))
        .expect("false named Boolean const should have canonical package-review custody");
    assert_eq!(
        static_arguments(&false_review),
        vec![PackageReviewContractStaticArgument::ConstBoolean(false)]
    );
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        false_review.canonical_review_bytes().unwrap(),
        "changing the selected canonical Boolean must change review identity",
    );

    let mut malformed = checked.clone();
    let enabled = malformed
        .typed
        .tables
        .const_declarations
        .iter()
        .find_map(|(handle, declaration)| {
            (malformed.symbols.name(declaration.symbol) == "ENABLED").then_some(handle)
        })
        .expect("ENABLED declaration");
    malformed
        .typed
        .tables
        .const_declarations
        .get_mut(enabled)
        .canonical_value_encoding = Some("boolean4:true-tail".to_owned());
    let diagnostics = project_checked_package_review(&malformed)
        .expect_err("malformed post-check Boolean encoding must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not a supported canonical value for its exact declared carrier")),
        "unexpected diagnostics: {diagnostics:#?}",
    );

    let mut wrong_carrier = checked;
    let declarations = wrong_carrier
        .typed
        .tables
        .const_declarations
        .iter()
        .filter_map(|(handle, declaration)| {
            let name = wrong_carrier.symbols.name(declaration.symbol);
            matches!(name, "ENABLED" | "OTHER").then_some((name.to_owned(), handle))
        })
        .collect::<Vec<_>>();
    let enabled = declarations
        .iter()
        .find_map(|(name, handle)| (name == "ENABLED").then_some(*handle))
        .expect("ENABLED declaration");
    let other = declarations
        .iter()
        .find_map(|(name, handle)| (name == "OTHER").then_some(*handle))
        .expect("OTHER declaration");
    let other_type = wrong_carrier
        .typed
        .tables
        .const_declarations
        .get(other)
        .declared_type;
    wrong_carrier
        .typed
        .tables
        .const_declarations
        .get_mut(enabled)
        .declared_type = other_type;
    let diagnostics = project_checked_package_review(&wrong_carrier)
        .expect_err("Boolean encoding under a non-Boolean exact carrier must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not a supported canonical value for its exact declared carrier")),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}

#[test]
fn public_contract_rejects_private_named_const_static_argument() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"const LIMIT: u64 = 7;
pub machine constant<const Value: u64>() -> u64 { 0 }
boundary machine trusted_constant() -> u64
ensures result == constant<LIMIT>();
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
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect_err("a public contract must not expose a private named const");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public interface selects private const")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

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
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed_type, &changed_const] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
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
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
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
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
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
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let compile = || {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
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
        .declaration = psi_symbols::SymbolHandle::invalid();
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
    let psi_typed_trees::expression::ExpressionNode::Call(mut call) = selection_drift
        .expression_table
        .expression(expression)
        .clone()
    else {
        panic!("checked occurrence rejoins its contract call")
    };
    call.machine_arguments[1].symbol = alternate;
    let closed = psi_typed_trees_to_checked_trees::close_conformance_application(
        &selection_drift.typed,
        &call.machine_arguments[1],
    )
    .expect("the alternate application also closes");
    *selection_drift
        .typed
        .expression_table
        .expression_mut(expression) = psi_typed_trees::expression::ExpressionNode::Call(call);
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
