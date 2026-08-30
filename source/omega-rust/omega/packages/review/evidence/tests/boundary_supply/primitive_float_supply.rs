use crate::support::*;

#[test]
fn review_closes_named_float_negation_without_replacing_authored_realizations() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data F32 {}
pub boundary operator F32::negate(value: f32) -> f32;
pub data F64 {}
pub boundary operator F64::negate(value: f64) -> f64;

pub data FloatProvider {}
pub machine FloatProvider::negate_f32(value: f32) -> f32
    satisfies F32::negate
    via Binding::CompilerIntrinsic;
pub machine FloatProvider::negate_f64(value: f64) -> f64
    satisfies F64::negate
    via Binding::CompilerIntrinsic;
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
    .expect("named-float negation fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("named-float negation has a closed package-review identity");

    for (requirement, realization, format) in [
        (
            "F32::negate",
            "FloatProvider::negate_f32",
            psi_numerics::literals::FloatFormat::F32,
        ),
        (
            "F64::negate",
            "FloatProvider::negate_f64",
            psi_numerics::literals::FloatFormat::F64,
        ),
    ] {
        let selected = review
            .selected_providers()
            .iter()
            .find(|provider| provider.schema_declaration().path() == requirement)
            .unwrap_or_else(|| panic!("missing selected provider for {requirement}"));
        let [row] = selected.row_declarations() else {
            panic!("one selected provider row for {requirement}")
        };
        assert_eq!(
            row.compiler_intrinsic_execution(),
            Some(PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(
                format
            )),
        );
        assert_eq!(row.compiler_intrinsic_builtin(), None);
        assert_eq!(
            row.realization().path(),
            realization,
            "the authored realization nominal remains independent of compiler execution identity",
        );
    }

    let selected_rows = review
        .canonical_rows()
        .expect("canonical negation provider rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .collect::<Vec<_>>();
    assert_eq!(selected_rows.len(), 1);
    for row in selected_rows {
        let encoded = encode_package_review_canonical_row(&row)
            .expect("selected negation-provider recovery envelope should encode");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("selected negation-provider recovery envelope should decode");
        assert_eq!(decoded.canonical_bytes(), row.canonical_bytes());
    }
}

#[test]
fn review_closes_named_float_conversion_with_exact_types_and_domain() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data F32 {}
pub boundary operator F32::from_f64(value: f64) -> f32;
pub data FloatProvider {}
pub machine FloatProvider::from_f64(value: f64) -> f32
    satisfies F32::from_f64
    via Binding::CompilerIntrinsic;
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
    .expect("named-float conversion fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("named-float conversion has a closed package-review identity");
    let selected = review
        .selected_providers()
        .iter()
        .find(|provider| provider.schema_declaration().path() == "F32::from_f64")
        .expect("selected F32::from_f64 provider");
    let [row] = selected.row_declarations() else {
        panic!("one selected provider row for F32::from_f64")
    };
    assert_eq!(
        row.compiler_intrinsic_execution(),
        Some(
            PackageReviewCompilerIntrinsicExecution::NamedFloatConversion {
                source: omega_provider_planning::plans::CompilerNumericType::F64,
                target: omega_provider_planning::plans::CompilerNumericType::F32,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            }
        ),
    );
    assert_eq!(row.compiler_intrinsic_builtin(), None);
    assert_eq!(row.realization().path(), "FloatProvider::from_f64");

    let selected_provider_row = review
        .canonical_rows()
        .expect("canonical conversion provider rows")
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .expect("selected-provider canonical row");
    let encoded = encode_package_review_canonical_row(&selected_provider_row)
        .expect("selected conversion-provider recovery envelope should encode");
    let decoded = decode_package_review_canonical_row(&encoded)
        .expect("selected conversion-provider recovery envelope should decode");
    assert_eq!(
        decoded.canonical_bytes(),
        selected_provider_row.canonical_bytes(),
        "canonical recovery must preserve exact conversion identity",
    );
}

#[test]
fn review_closes_primitive_float_binary_execution_by_operation_and_format() {
    let Some(target) = host_target_name() else {
        return;
    };

    use omega_provider_planning::plans::CompilerPrimitiveFloatBinaryOperation as Operation;
    use psi_numerics::literals::FloatFormat;

    let operations = [
        ("add", "+", Operation::Add, false),
        ("subtract", "-", Operation::Subtract, false),
        ("multiply", "*", Operation::Multiply, false),
        ("divide", "/", Operation::Divide, false),
        ("equal", "==", Operation::Equal, true),
        ("not_equal", "!=", Operation::NotEqual, true),
        ("less", "<", Operation::Less, true),
        ("less_or_equal", "<=", Operation::LessOrEqual, true),
        ("greater", ">", Operation::Greater, true),
        ("greater_or_equal", ">=", Operation::GreaterOrEqual, true),
    ];
    let formats = [("f32", FloatFormat::F32), ("f64", FloatFormat::F64)];
    let mut source = "pub data Float {}\npub data FloatProvider {}\n".to_owned();
    for (name, spelling, _, returns_boolean) in operations {
        for (primitive, _) in formats {
            let result = if returns_boolean { "bool" } else { primitive };
            source.push_str(&format!(
                "pub boundary operator {spelling} Float::{name}(left: {primitive}, right: {primitive}) -> {result};\n\
                 pub machine FloatProvider::{name}_{primitive}(left: {primitive}, right: {primitive}) -> {result}\n\
                     satisfies Float::{name}\n\
                     via Binding::CompilerIntrinsic;\n",
            ));
        }
    }

    let package = TempPackage::new();
    package.write("main.omg", &source);
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
    .expect("primitive float boundary-operator overloads should select independently");
    let review = project_checked_package_review(&checked)
        .expect("primitive float executions have closed package-review identities");
    let rows = review
        .selected_providers()
        .iter()
        .flat_map(|provider| provider.row_declarations())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), operations.len() * formats.len());
    for (name, _, operation, _) in operations {
        for (primitive, format) in formats {
            let realization = format!("FloatProvider::{name}_{primitive}");
            let row = rows
                .iter()
                .find(|row| row.realization().path() == realization)
                .unwrap_or_else(|| panic!("missing selected provider row for {realization}"));
            assert_eq!(
                row.compiler_intrinsic_execution(),
                Some(
                    PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary {
                        operation,
                        format,
                    }
                ),
            );
            assert_eq!(row.compiler_intrinsic_builtin(), None);
        }
    }

    let selected_provider_row = review
        .canonical_rows()
        .expect("canonical primitive-float provider rows")
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .expect("selected-provider canonical row");
    let encoded = encode_package_review_canonical_row(&selected_provider_row)
        .expect("primitive-float provider recovery envelope should encode");
    let decoded = decode_package_review_canonical_row(&encoded)
        .expect("primitive-float provider recovery envelope should decode");
    assert_eq!(
        decoded.canonical_bytes(),
        selected_provider_row.canonical_bytes()
    );
}

#[test]
fn primitive_float_binary_intrinsics_require_the_exact_token_and_shape() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let cases = [
        (
            "tokenless",
            "pub boundary operator Float::add(left: f32, right: f32) -> f32;",
            "pub machine FloatProvider::realize(left: f32, right: f32) -> f32",
        ),
        (
            "mismatched-token",
            "pub boundary operator - Float::add(left: f32, right: f32) -> f32;",
            "pub machine FloatProvider::realize(left: f32, right: f32) -> f32",
        ),
        (
            "integer-operands",
            "pub boundary operator + Float::add(left: i32, right: i32) -> i32;",
            "pub machine FloatProvider::realize(left: i32, right: i32) -> i32",
        ),
        (
            "mixed-formats",
            "pub boundary operator + Float::add(left: f32, right: f64) -> f32;",
            "pub machine FloatProvider::realize(left: f32, right: f64) -> f32",
        ),
        (
            "arithmetic-bool-result",
            "pub boundary operator + Float::add(left: f32, right: f32) -> bool;",
            "pub machine FloatProvider::realize(left: f32, right: f32) -> bool",
        ),
        (
            "comparison-float-result",
            "pub boundary operator == Float::equal(left: f32, right: f32) -> f32;",
            "pub machine FloatProvider::realize(left: f32, right: f32) -> f32",
        ),
    ];

    for (label, operator, machine) in cases {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                "pub data Float {{}}\npub data FloatProvider {{}}\n{operator}\n{machine}\n    satisfies Float::{}\n    via Binding::CompilerIntrinsic;\n",
                if label == "comparison-float-result" {
                    "equal"
                } else {
                    "add"
                },
            ),
        );
        package.write("build.omg", build);
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect_err("malformed primitive float intrinsic must fail before package review");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("no compiler-known migrated intrinsic")
                    || diagnostic
                        .message
                        .contains("no compiler-known intrinsic realization")
            }),
            "{label}: {diagnostics:?}",
        );
    }
}
