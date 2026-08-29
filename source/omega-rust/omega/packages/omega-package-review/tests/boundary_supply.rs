mod support;

use support::*;

#[test]
fn empty_boundary_body_is_checked_callable_and_remains_directly_invocable() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::std::filesystem_host;

boundary machine adapter() reaches FilesystemHost { }

pub machine caller() reaches FilesystemHost {
    adapter();
}
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
    .expect("an explicit empty boundary body remains executable");
    let review =
        project_checked_package_review(&checked).expect("empty boundary body review should close");
    let adapter = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "adapter")
        .expect("adapter review row");
    assert!(matches!(
        adapter.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
    assert!(review.dangerous_authority_slack().iter().any(|slack| {
        slack.class() == PackageReviewDangerousAuthorityClass::Filesystem
            && slack.callable().path() == "adapter"
    }));
}

#[test]
fn package_review_rejects_impossible_supply_body_combinations() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write("main.omg", "pub machine api() { }\n");
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
    .expect("ordinary package should check");

    let mut missing_body = checked.clone();
    missing_body
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "api")
        .expect("api machine")
        .body_is_present = false;
    let diagnostics = project_checked_package_review(&missing_body)
        .expect_err("checked supply without a body must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("classified as checked supply but has no retained body")
    }));

    let mut bodyful_accepted = checked;
    let api = bodyful_accepted
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "api")
        .expect("api machine");
    api.supply_mode = psi_language_semantics::MachineSupplyMode::Accepted;
    let diagnostics = project_checked_package_review(&bodyful_accepted)
        .expect_err("bodyless supply with a body must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has bodyless supply but retains a body")
    }));
}

#[test]
fn review_projects_every_external_executable_supply_mechanism_as_opaque_blocking() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait ExternalSurface {
    machine imported() reaches ExternalSurface;
    machine syscalled() reaches ExternalSurface;
    machine intrinsic() reaches ExternalSurface;
    machine slot() reaches ExternalSurface;
    machine field() reaches ExternalSurface;
    machine table() reaches ExternalSurface;
}

pub data DispatchTable {
    dispatch: addr;
    invoke: addr;
}

pub machine import_leaf()
    satisfies ExternalSurface::imported
    via Binding::DllImport("libomega", "omega_entry");
pub machine syscall_leaf()
    satisfies ExternalSurface::syscalled
    via Binding::Syscall(61);
machine intrinsic_leaf()
    satisfies ExternalSurface::intrinsic
    via Binding::CompilerIntrinsic;
pub machine slot_leaf()
    satisfies ExternalSurface::slot
    via Binding::VtableSlot(7);
pub machine DispatchTable::field_leaf()
    satisfies ExternalSurface::field
    via Binding::VtableField(dispatch);
pub machine DispatchTable::table_leaf()
    satisfies ExternalSurface::table
    via Binding::TableFunction(invoke);
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
    .expect("external executable-supply fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("external executable-supply review should close");

    let expected = [
        (
            "import_leaf",
            PackageReviewExternalBinding::Import {
                library: "libomega".to_owned(),
                symbol: "omega_entry".to_owned(),
            },
        ),
        (
            "syscall_leaf",
            PackageReviewExternalBinding::Syscall { number: 61 },
        ),
        (
            "intrinsic_leaf",
            PackageReviewExternalBinding::CompilerIntrinsic,
        ),
        (
            "slot_leaf",
            PackageReviewExternalBinding::VtableSlot { index: 7 },
        ),
        (
            "DispatchTable::field_leaf",
            PackageReviewExternalBinding::VtableField {
                field: "dispatch".to_owned(),
            },
        ),
        (
            "DispatchTable::table_leaf",
            PackageReviewExternalBinding::TableFunction {
                field: "invoke".to_owned(),
            },
        ),
    ];
    let expected_count = expected.len();
    assert_eq!(review.external_executable_supply().len(), expected_count);
    for (callable, binding) in expected {
        let supply = review
            .external_executable_supply()
            .iter()
            .find(|supply| supply.callable().path() == callable)
            .unwrap_or_else(|| panic!("missing external supply for {callable}"));
        assert_eq!(supply.binding(), &binding);
        assert_eq!(
            supply
                .conformance()
                .expect("trait-bound external supply")
                .trait_identity()
                .path(),
            "ExternalSurface"
        );
        let callable_row = review
            .callables()
            .iter()
            .find(|candidate| candidate.identity() == supply.callable());
        if callable == "intrinsic_leaf" {
            assert!(
                callable_row.is_none(),
                "a private external leaf must not become public callable API"
            );
        } else {
            assert!(callable_row.is_some_and(|candidate| {
                candidate.supply() == PackageReviewCallableSupply::ExternalRealization
            }));
        }
    }

    let rows = review
        .canonical_rows()
        .expect("canonical external-supply rows");
    let supply_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .collect::<Vec<_>>();
    assert_eq!(supply_rows.len(), expected_count);
    assert!(supply_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::OpaqueBlocking
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && location.relative_path() == "main.omg"
                }) && locations
                    .iter()
                    .filter(|location| {
                        location.role() == PackageReviewSourceLocationRole::ExternalBinding
                            && location.relative_path() == "main.omg"
                            && location.end_byte() - location.start_byte() == 3
                    })
                    .count()
                    == 1
            })
    }));
    for row in supply_rows {
        let encoded = encode_package_review_canonical_row(row)
            .expect("external-supply recovery envelope should encode");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("external-supply recovery envelope should decode");
        assert_eq!(
            decoded.kind(),
            PackageReviewCanonicalRowKind::ExternalExecutableSupply
        );
        assert_eq!(
            decoded.risk(),
            PackageReviewCanonicalRowRisk::OpaqueBlocking
        );
        assert_eq!(decoded.key_bytes(), row.key_bytes());
    }
}

#[test]
fn review_joins_external_boundary_operator_supply_without_implying_visibility() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
pub boundary operator F32::maximum(left: f32, right: f32) -> f32;
pub boundary operator F32::square_root(value: f32) -> f32;

pub data FloatProvider {}
pub machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
machine FloatProvider::maximum(left: f32, right: f32) -> f32
    satisfies F32::maximum
    via Binding::CompilerIntrinsic;
machine FloatProvider::square_root(value: f32) -> f32
    satisfies F32::square_root
    via Binding::CompilerIntrinsic;
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
    .expect("external boundary-operator fixture should check and select exact intrinsics");
    let review = project_checked_package_review(&checked)
        .expect("external boundary-operator supply should project exactly");

    assert_eq!(review.external_executable_supply().len(), 3);
    for (requirement, expected_builtin) in [
        ("minimum", psi_symbols::BuiltinFunction::Min),
        ("maximum", psi_symbols::BuiltinFunction::Max),
        ("square_root", psi_symbols::BuiltinFunction::Sqrt),
    ] {
        let callable_path = format!("FloatProvider::{requirement}");
        let declaration = review
            .public_operators()
            .iter()
            .find(|operator| {
                operator.coordinate().identity().path() == format!("F32::{requirement}")
            })
            .unwrap_or_else(|| panic!("missing public operator {requirement}"));
        let supply = review
            .external_executable_supply()
            .iter()
            .find(|supply| supply.callable().path() == callable_path)
            .unwrap_or_else(|| panic!("missing external supply for {callable_path}"));
        assert!(matches!(
            supply.requirement(),
            PackageReviewExternalRequirement::Operator(operator)
                if operator == declaration.coordinate()
        ));
        assert_eq!(supply.operator(), Some(declaration.coordinate()));
        assert_eq!(supply.conformance(), None);
        assert_eq!(
            supply.binding(),
            &PackageReviewExternalBinding::CompilerIntrinsic
        );

        let selected = review
            .selected_providers()
            .iter()
            .find(|provider| provider.schema_declaration() == declaration.coordinate().identity())
            .unwrap_or_else(|| panic!("missing selected provider for {requirement}"));
        let [selected_row] = selected.row_declarations() else {
            panic!("one selected realization for {requirement}")
        };
        assert_eq!(selected_row.realization(), supply.callable());
        assert_eq!(
            selected_row.requirement().owner(),
            declaration.coordinate().identity().owner()
        );
        assert_eq!(
            selected_row.compiler_intrinsic_builtin(),
            Some(expected_builtin),
            "selected provider review must retain the closed builtin child separately from the authored realization",
        );
        assert!(matches!(
            selected.rows()[0].binding,
            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
        ));
    }

    let public_callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "FloatProvider::minimum")
        .expect("public external operator leaf should remain public callable API");
    assert_eq!(
        public_callable.supply(),
        PackageReviewCallableSupply::ExternalRealization
    );
    assert_eq!(public_callable.operator_realizations().len(), 1);
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "FloatProvider::maximum"),
        "private external operator leaf must not become public callable API"
    );

    let rows = review
        .canonical_rows()
        .expect("canonical external operator-supply rows");
    let supply_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .collect::<Vec<_>>();
    assert_eq!(supply_rows.len(), 3);
    assert!(supply_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::OpaqueBlocking
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && location.relative_path() == "main.omg"
                }) && locations
                    .iter()
                    .filter(|location| {
                        location.role() == PackageReviewSourceLocationRole::ExternalBinding
                            && location.relative_path() == "main.omg"
                            && location.end_byte() - location.start_byte() == 3
                    })
                    .count()
                    == 1
            })
    }));
    for row in supply_rows {
        let encoded = encode_package_review_canonical_row(row)
            .expect("external operator-supply recovery envelope should encode");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("external operator-supply recovery envelope should decode");
        assert_eq!(
            decoded.kind(),
            PackageReviewCanonicalRowKind::ExternalExecutableSupply
        );
        assert_eq!(
            decoded.risk(),
            PackageReviewCanonicalRowRisk::OpaqueBlocking
        );
        assert_eq!(decoded.key_bytes(), row.key_bytes());
    }
    let selected_provider_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .expect("canonical selected-provider identity");
    let encoded = encode_package_review_canonical_row(selected_provider_row)
        .expect("selected-provider recovery envelope should encode");
    let decoded = decode_package_review_canonical_row(&encoded)
        .expect("selected-provider recovery envelope should decode");
    assert_eq!(
        decoded.canonical_bytes(),
        selected_provider_row.canonical_bytes(),
        "canonical recovery must preserve the builtin ordinals",
    );
}

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
    let build = r#"target windows_x64 { }
target linux_x64 { }
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

#[test]
fn unsupported_external_boundary_operator_neighbors_remain_fail_closed() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let cases = [
        (
            "private-operator",
            r#"data F32 {}
boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
"#,
            "realizes non-public operator",
        ),
        (
            "aliased",
            r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum as Selected
    via Binding::CompilerIntrinsic;
"#,
            "through an alias not yet represented",
        ),
        (
            "generic-machine",
            r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum<T>(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
"#,
            "generic or lifetime-parameterized boundary operator",
        ),
    ];

    for (label, source, expected) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .unwrap_or_else(|diagnostics| panic!("{label} fixture should check: {diagnostics:?}"));
        let diagnostics = project_checked_package_review(&checked)
            .expect_err("unsupported external operator realization must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{label}: {diagnostics:?}"
        );
    }
}

#[test]
fn external_binding_changes_only_the_supply_row_for_a_stable_callable() {
    let Some(target) = host_target_name() else {
        return;
    };
    let project = |number: i64| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub boundary trait ExternalSurface {{
    machine invoke() reaches ExternalSurface;
}}
pub machine invoke_leaf()
    satisfies ExternalSurface::invoke
    via Binding::Syscall({number});
"#,
            ),
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
        .expect("external syscall fixture should check");
        project_checked_package_review(&checked)
            .expect("external syscall package review should close")
            .canonical_rows()
            .expect("external syscall canonical rows")
    };

    let old = project(60);
    let new = project(61);
    let old_callable = old
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("old callable row");
    let new_callable = new
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("new callable row");
    assert_eq!(old_callable.key_bytes(), new_callable.key_bytes());
    assert_eq!(
        old_callable.canonical_bytes(),
        new_callable.canonical_bytes()
    );

    let old_supply = old
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .expect("old external-supply row");
    let new_supply = new
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .expect("new external-supply row");
    assert_eq!(old_supply.key_bytes(), new_supply.key_bytes());
    assert_ne!(old_supply.canonical_bytes(), new_supply.canonical_bytes());
}

#[test]
fn external_executable_supply_projection_rejects_inconsistent_checked_state() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait ExternalSurface {
    machine invoke() reaches ExternalSurface;
}
pub machine invoke_leaf()
    satisfies ExternalSurface::invoke
    via Binding::Syscall(60);
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
    .expect("external tamper fixture should check");

    fn replace_external_binding(
        checked: &mut CheckedCompilation,
        identity: psi_language_semantics::ExternalBindingIdentity,
    ) {
        let mechanism = identity.mechanism();
        let binding = checked.typed.external_bindings.intern(identity);
        let leaf = checked
            .typed
            .machines_mut()
            .iter_mut()
            .find(|machine| machine.name.as_str() == "invoke_leaf")
            .expect("external leaf");
        let satisfies = leaf.satisfies;
        leaf.supply_mode =
            psi_language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism };
        checked
            .typed
            .machine_trait_conformances
            .span_mut_or_empty(satisfies)[0]
            .external_binding = Some(binding);
    }

    let mut mechanism_mismatch = checked.clone();
    let leaf = mechanism_mismatch
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf");
    let psi_language_semantics::MachineSupplyMode::ExternalRealization { binding, .. } =
        leaf.supply_mode
    else {
        panic!("external leaf supply")
    };
    leaf.supply_mode = psi_language_semantics::MachineSupplyMode::ExternalRealization {
        binding,
        mechanism: psi_language_semantics::ExternalBindingMechanism::Import,
    };
    let diagnostics = project_checked_package_review(&mechanism_mismatch)
        .expect_err("mechanism mismatch must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("supply mechanism inconsistent with its exact binding identity")
    }));

    let mut span_without_conformance_binding = checked.clone();
    let satisfies = span_without_conformance_binding
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    span_without_conformance_binding
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = None;
    let diagnostics = project_checked_package_review(&span_without_conformance_binding)
        .expect_err("authored custody without a binding must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retains authored `via` custody without an external binding")
    }));

    let mut binding_without_source_span = checked.clone();
    let satisfies = binding_without_source_span
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    binding_without_source_span
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding_source_span = None;
    let diagnostics = project_checked_package_review(&binding_without_source_span)
        .expect_err("external binding without authored custody must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no exact authored `via` custody")
    }));

    let mut invalid_source_span = checked.clone();
    let satisfies = invalid_source_span
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    invalid_source_span
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding_source_span = Some(Default::default());
    let diagnostics = project_checked_package_review(&invalid_source_span)
        .expect_err("source-free external binding custody must fail closed");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("source span"))
    );

    let mut missing_binding_identity = checked.clone();
    let invalid_binding = psi_language_semantics::ExternalBindingId(u32::MAX);
    let leaf = missing_binding_identity
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf");
    let satisfies = leaf.satisfies;
    leaf.supply_mode = psi_language_semantics::MachineSupplyMode::ExternalRealization {
        binding: invalid_binding,
        mechanism: psi_language_semantics::ExternalBindingMechanism::Syscall,
    };
    missing_binding_identity
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = Some(invalid_binding);
    let diagnostics = project_checked_package_review(&missing_binding_identity)
        .expect_err("missing binding-table identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no exact binding-table identity")
    }));

    let mut bodyful_external = checked.clone();
    bodyful_external
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .body_is_present = true;
    let diagnostics = project_checked_package_review(&bodyful_external)
        .expect_err("bodyful external supply must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retains an implementation body")
    }));

    let mut missing_conformance = checked.clone();
    missing_conformance
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies = Default::default();
    let diagnostics = project_checked_package_review(&missing_conformance)
        .expect_err("external supply without a conformance must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has 0 conformance applications; expected exactly one")
    }));

    let mut duplicate_conformance = checked.clone();
    let leaf_index = duplicate_conformance
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf index");
    let duplicate = duplicate_conformance
        .typed
        .machine_trait_conformances(&duplicate_conformance.typed.machines()[leaf_index])[0]
        .clone();
    let machine_roots = duplicate_conformance.typed.roots.machines;
    let tables = &mut duplicate_conformance.typed.tables;
    let leaf = &mut tables.machines.span_mut_or_empty(machine_roots)[leaf_index];
    tables
        .machine_trait_conformances
        .append_to_span(&mut leaf.satisfies, duplicate);
    let diagnostics = project_checked_package_review(&duplicate_conformance)
        .expect_err("multiple external conformances must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has 2 conformance applications; expected exactly one")
    }));

    let mut mismatched_conformance_binding = checked.clone();
    let different_binding = mismatched_conformance_binding
        .typed
        .external_bindings
        .intern(psi_language_semantics::ExternalBindingIdentity::Syscall { number: 61 });
    let satisfies = mismatched_conformance_binding
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    mismatched_conformance_binding
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = Some(different_binding);
    let diagnostics = project_checked_package_review(&mismatched_conformance_binding)
        .expect_err("different valid conformance binding must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance binding inconsistent with its supply mode")
    }));

    let mut nonexternal_supply = checked.clone();
    nonexternal_supply
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .supply_mode = psi_language_semantics::MachineSupplyMode::Boundary;
    let diagnostics = project_checked_package_review(&nonexternal_supply)
        .expect_err("external conformance binding on ordinary supply must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("external conformance binding without external supply")
    }));

    let malformed = [
        (
            psi_language_semantics::ExternalBindingIdentity::Import {
                library: String::new(),
                symbol: "entry".to_owned(),
            },
            "has no exact import-library identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::Import {
                library: "omega".to_owned(),
                symbol: String::new(),
            },
            "has no exact import-symbol identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::Syscall { number: -1 },
            "has a syscall number outside 0..=u32::MAX",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::VtableSlot { index: -1 },
            "has a negative vtable-slot index",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::VtableField {
                field: String::new(),
            },
            "has no exact table-field identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::TableFunction {
                field: "invoke".to_owned(),
            },
            "has table-field supply without one exact attached provider data declaration",
        ),
    ];
    for (identity, expected) in malformed {
        let mut tampered = checked.clone();
        replace_external_binding(&mut tampered, identity);
        let diagnostics = project_checked_package_review(&tampered)
            .expect_err("malformed external binding payload must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing diagnostic containing {expected:?}: {diagnostics:?}"
        );
    }
}
