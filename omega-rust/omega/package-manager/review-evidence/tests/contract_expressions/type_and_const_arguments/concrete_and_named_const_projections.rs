use crate::support::*;
use compiler::CheckedCompileRequest;

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
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
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
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
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
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
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
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("named const static contract argument should check")
    };

    let checked = compile(7);
    let const_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            matches!(
                selection.target(),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if checked.symbols.get(target.selected_symbol()).kind
                        == symbols::SymbolKind::Const
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
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument
            && selection.exposure()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
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
            typed_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "constant"
                    && call.machine_arguments.len() == 2
                    && call.machine_arguments[0].symbol.is_valid()
                    && tampered.symbols.get(call.machine_arguments[0].symbol).kind
                        == symbols::SymbolKind::Const =>
            {
                Some(expression)
            }
            _ => None,
        })
        .expect("named const contract call expression");
    let typed_trees::expression::ExpressionNode::Call(call) = tampered
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
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
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
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
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
            .contains("with a malformed canonical value encoding")),
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
            .contains("does not replay against its exact declared carrier")),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}

#[test]
fn review_projects_named_structured_consts_with_exact_carrier_replay() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    let compile = |left_x: u8| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
r#"pub data Point [copy] {{ x: u8; y: u8; }}
pub data Pair [copy] {{ left: Point; right: Point; flags: [bool; 2]; }}
pub data Mode [copy] {{ case Idle; case Count(value: u8); }}
pub data Alternate [copy] {{ value: u64; }}
pub const SELECTED: Pair = Pair {{ left: Point {{ x: {left_x}, y: 2 }}, right: Point {{ x: 3, y: 4 }}, flags: [true, false] }};
pub const ACTIVE: Mode = Mode::Count {{ value: 5 }};
pub const OTHER: Alternate = Alternate {{ value: 0 }};
pub machine selected<const Value: Pair, const State: Mode>() -> u64 {{ 0 }}
boundary machine trusted_pair() -> u64
ensures result == selected<SELECTED, ACTIVE>();
"#,
            ),
        );
        package.write("build.omg", build);
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("named structured const static contract argument should check")
    };
    let static_argument = |review: &CheckedPackageReviewProjection| {
        let callable = review
            .callables()
            .iter()
            .find(|callable| callable.identity().path() == "trusted_pair")
            .expect("trusted structured-const callable");
        let [contract] = callable.contracts() else {
            panic!("one trusted-pair contract")
        };
        let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            right,
            ..
        }) = contract.fact()
        else {
            panic!("trusted-pair equality contract")
        };
        let PackageReviewContractExpression::Call {
            static_arguments, ..
        } = right.as_ref()
        else {
            panic!("named structured-const call")
        };
        static_arguments.clone()
    };

    let checked = compile(1);
    let review = project_checked_package_review(&checked)
        .expect("structured const should replay through its exact checked carrier");
    let arguments = static_argument(&review);
    let [
        PackageReviewContractStaticArgument::ConstStructured {
            declared_type: pair_type,
            canonical_value_encoding: pair_encoding,
        },
        PackageReviewContractStaticArgument::ConstStructured {
            declared_type: mode_type,
            canonical_value_encoding: mode_encoding,
        },
    ] = arguments.as_slice()
    else {
        panic!("record and pure-sum structured const review values")
    };
    assert!(pair_type.canonical().contains("Pair"));
    assert!(pair_encoding.starts_with("record"));
    assert!(mode_type.canonical().contains("Mode"));
    assert!(mode_encoding.starts_with("variant"));
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        project_checked_package_review(&compile(9))
            .unwrap()
            .canonical_review_bytes()
            .unwrap(),
        "changing one nested scalar must change canonical package identity",
    );

    let mut field_spoof = checked.clone();
    let selected = field_spoof
        .typed
        .tables
        .const_declarations
        .iter()
        .find_map(|(handle, declaration)| {
            (field_spoof.symbols.name(declaration.symbol) == "SELECTED").then_some(handle)
        })
        .expect("SELECTED declaration");
    let encoding = field_spoof
        .typed
        .tables
        .const_declarations
        .get(selected)
        .canonical_value_encoding
        .clone()
        .expect("canonical structured value");
    field_spoof
        .typed
        .tables
        .const_declarations
        .get_mut(selected)
        .canonical_value_encoding = Some(encoding.replacen("left", "rift", 1));
    let diagnostics = project_checked_package_review(&field_spoof)
        .expect_err("a framed field-name spoof must fail exact carrier replay");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not replay against its exact declared carrier")),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let mut carrier_spoof = checked;
    let alternate_type = carrier_spoof
        .const_declarations()
        .iter()
        .find(|declaration| carrier_spoof.symbols.name(declaration.symbol) == "OTHER")
        .expect("OTHER declaration")
        .declared_type;
    carrier_spoof
        .typed
        .tables
        .const_declarations
        .get_mut(selected)
        .declared_type = alternate_type;
    let diagnostics = project_checked_package_review(&carrier_spoof)
        .expect_err("structured value under a substituted carrier must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not replay against its exact declared carrier")),
        "unexpected diagnostics: {diagnostics:#?}"
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
        r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
    );
    let diagnostics = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect_err("a public contract must not expose a private named const");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public interface selects private const")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}
