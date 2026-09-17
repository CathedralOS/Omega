use super::fixture_roster;
use super::{retained_float_differential_result_identity, selected_intrinsic_diagnostic_label};
use crate::{
    Command, compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host,
    compile_rooted_canary_for_target, executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn named_float_provider_calls_rewrite_to_selected_builtins() {
    #[derive(Debug, PartialEq, Eq)]
    struct FloatProviderContract {
        intrinsic: String,
        parameter_count: usize,
        parameter_type_identities: Vec<String>,
        has_result: bool,
        result_type_identity: Option<String>,
        service_reach: Vec<String>,
        synchronous_invocations: Vec<String>,
        may_suspend: bool,
        may_block: bool,
        terminates_guarantee: bool,
    }

    const DIFFERENTIAL_SUITE_ID: &str =
        "omega.float.hardware.macos_arm64.minimum-maximum-square-root.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 NaN operand order",
        "binary64 NaN operand order",
        "binary32 minimum signed-zero choice",
        "binary64 maximum signed-zero choice",
        "binary32 exact square root",
        "binary64 exact square root",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x0b72_09a4_4518_814d;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_MIN_MAX_SQRT_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("named float provider calls should compile to checked trees");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    let mut selected_contract_rows = std::collections::BTreeMap::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_report_fingerprint == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
            .expect("named operator evidence must resolve to its retained plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named float plan must contain exactly one row");
        };
        let [method] = plan.schema.methods.as_slice() else {
            panic!("named float plan must contain exactly one service method");
        };
        assert_eq!(plan.schema.trait_name, method.requirement_identity);
        assert_eq!(method.requirement_owner, method.requirement_identity);
        assert_eq!(method.name, "realize");
        assert_eq!(row.method, method.name);
        assert_eq!(row.requirement_identity, method.requirement_identity);
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding else {
            panic!("named float plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        let contract = FloatProviderContract {
            intrinsic: name.clone(),
            parameter_count: method.parameter_count,
            parameter_type_identities: method.parameter_type_identities.clone(),
            has_result: method.has_result,
            result_type_identity: method.result_type_identity.clone(),
            service_reach: method.service_reach.clone(),
            synchronous_invocations: method.synchronous_invocations.clone(),
            may_suspend: method.may_suspend,
            may_block: method.may_block,
            terminates_guarantee: method.terminates_guarantee,
        };
        match selected_contract_rows.entry(method.requirement_identity.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(contract);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                assert_eq!(entry.get(), &contract, "selected provider contract drifted");
            }
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());

        let typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("named operator use must remain a call expression");
        };
        assert!(
            !call.receiver.is_valid(),
            "builtin dispatch removes F32/F64"
        );
        let expected_builtin = if name.contains("::minimum.") {
            "min"
        } else if name.contains("::maximum.") {
            "max"
        } else if name.contains("::square_root.") {
            "sqrt"
        } else {
            panic!("unexpected migrated named intrinsic `{name}`");
        };
        assert_eq!(call.target.as_str(), expected_builtin);
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(match expected_builtin {
                    "min" => symbols::BuiltinFunction::Min,
                    "max" => symbols::BuiltinFunction::Max,
                    "sqrt" => symbols::BuiltinFunction::Sqrt,
                    _ => unreachable!(),
                })
        );
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::maximum.f32".to_owned(),
            "F32::minimum.f32".to_owned(),
            "F32::square_root.f32".to_owned(),
            "F64::maximum.f64".to_owned(),
            "F64::minimum.f64".to_owned(),
            "F64::square_root.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    let expected_contract =
        |intrinsic: &str, parameter_type_identities: &[&str], result_type_identity: &str| {
            FloatProviderContract {
                intrinsic: intrinsic.to_owned(),
                parameter_count: parameter_type_identities.len(),
                parameter_type_identities: parameter_type_identities
                    .iter()
                    .map(|identity| (*identity).to_owned())
                    .collect(),
                has_result: true,
                result_type_identity: Some(result_type_identity.to_owned()),
                service_reach: Vec::new(),
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
            }
        };
    let expected_contract_rows: std::collections::BTreeMap<_, _> = [
        (
            "operator::F32::maximum(named(name(f32)),named(name(f32)))->named(name(f32))",
            expected_contract(
                "F32::maximum.f32",
                &["named(name(f32))", "named(name(f32))"],
                "named(name(f32))",
            ),
        ),
        (
            "operator::F32::minimum(named(name(f32)),named(name(f32)))->named(name(f32))",
            expected_contract(
                "F32::minimum.f32",
                &["named(name(f32))", "named(name(f32))"],
                "named(name(f32))",
            ),
        ),
        (
            "operator::F32::square_root(named(name(f32)))->named(name(f32))",
            expected_contract(
                "F32::square_root.f32",
                &["named(name(f32))"],
                "named(name(f32))",
            ),
        ),
        (
            "operator::F64::maximum(named(name(f64)),named(name(f64)))->named(name(f64))",
            expected_contract(
                "F64::maximum.f64",
                &["named(name(f64))", "named(name(f64))"],
                "named(name(f64))",
            ),
        ),
        (
            "operator::F64::minimum(named(name(f64)),named(name(f64)))->named(name(f64))",
            expected_contract(
                "F64::minimum.f64",
                &["named(name(f64))", "named(name(f64))"],
                "named(name(f64))",
            ),
        ),
        (
            "operator::F64::square_root(named(name(f64)))->named(name(f64))",
            expected_contract(
                "F64::square_root.f64",
                &["named(name(f64))"],
                "named(name(f64))",
            ),
        ),
    ]
    .into_iter()
    .map(|(requirement, contract)| (requirement.to_owned(), contract))
    .collect();
    assert_eq!(selected_contract_rows, expected_contract_rows);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per operation/format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 70, "rewritten builtins must execute");

    let build_dir =
        std::env::temp_dir().join(format!("omega-named-float-provider-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named float provider calls should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named float provider canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "selected named float builtins must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-provider-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("named float provider calls should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_negate_and_is_nan_preserve_selected_roots_and_execute() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.negate-is-nan.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 signed-zero and infinity negation",
        "binary64 signed-zero and infinity negation",
        "binary32 NaN/infinity/finite predicate separation",
        "binary64 NaN/infinity/finite predicate separation",
        "selected-root unary evaluation shape",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x3c92_46b9_d29d_254c;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_NEGATE_IS_NAN_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("named negate/is_nan provider calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_report_fingerprint == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
            .expect("named operator evidence must resolve to its retained plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named float plan must contain exactly one row");
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding else {
            panic!("named float plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());

        if name.contains("::negate.") {
            let typed_trees::expression::ExpressionNode::Binary(binary) = checked
                .typed
                .expression_table
                .expression(operator_use.expression)
            else {
                panic!("`{name}` must preserve its selected root as a primitive binary expression");
            };
            assert_eq!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Multiply
            );
            let typed_trees::expression::ExpressionNode::Float(negative_one) =
                checked.typed.expression_table.expression(binary.right)
            else {
                panic!("`{name}` must multiply by a landed -1 literal");
            };
            assert_eq!(negative_one.text(), "-1.0");
            assert_eq!(
                negative_one.landing(),
                Some(if name.starts_with("F32::") {
                    numerics::literals::FloatFormat::F32
                } else {
                    numerics::literals::FloatFormat::F64
                })
            );
        } else if name.contains("::is_nan.") {
            let typed_trees::expression::ExpressionNode::Call(call) = checked
                .typed
                .expression_table
                .expression(operator_use.expression)
            else {
                panic!("`{name}` must preserve its selected root as a unary builtin call");
            };
            assert_eq!(
                call.target_symbol,
                checked
                    .typed
                    .symbols
                    .builtin_function_symbol(symbols::BuiltinFunction::FloatIsNan)
                    .expect("internal float is_nan builtin symbol")
            );
            assert_eq!(call.target.as_str(), "float#is_nan");
            assert!(!call.receiver.is_valid());
            assert_eq!(
                call.arguments.count(),
                1,
                "`{name}` must evaluate one argument"
            );
        } else {
            panic!("unexpected migrated named intrinsic `{name}`");
        }
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::is_nan.f32".to_owned(),
            "F32::negate.f32".to_owned(),
            "F64::is_nan.f64".to_owned(),
            "F64::negate.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        4,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per operation/format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "rewritten negate/is_nan expressions must execute in the interpreter"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-float-negate-is-nan-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "named negate/is_nan provider calls should compile from their authored native root",
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named negate/is_nan provider canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "selected named negate/is_nan expressions must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-negate-is-nan-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
            panic!(
                "named negate/is_nan provider calls should compile for {target}: {diagnostics:#?}"
            )
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_classification_predicates_select_and_execute() {
    const DIFFERENTIAL_SUITE_ID: &str =
        "omega.float.hardware.macos_arm64.classification-predicates.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32/binary64 finite versus infinity",
        "binary32/binary64 infinity versus NaN",
        "binary32/binary64 normal versus subnormal",
        "binary32/binary64 subnormal versus zero",
        "exactly-once unary evaluation shape",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xa6bf_7c01_3cb0_fd6a;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_CLASSIFICATION_PREDICATES_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("named float classification calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_report_fingerprint == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
            .expect("named classification evidence must retain its plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named classification plan must contain one row");
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding else {
            panic!("named classification plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());

        let (expected_builtin, expected_target) = if name.contains("::is_finite.") {
            (symbols::BuiltinFunction::FloatIsFinite, "float#is_finite")
        } else if name.contains("::is_infinite.") {
            (
                symbols::BuiltinFunction::FloatIsInfinite,
                "float#is_infinite",
            )
        } else if name.contains("::is_normal.") {
            (symbols::BuiltinFunction::FloatIsNormal, "float#is_normal")
        } else if name.contains("::is_subnormal.") {
            (
                symbols::BuiltinFunction::FloatIsSubnormal,
                "float#is_subnormal",
            )
        } else {
            panic!("unexpected classification intrinsic `{name}`");
        };
        let typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must remain a unary builtin call");
        };
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(expected_builtin)
        );
        assert_eq!(call.target.as_str(), expected_target);
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 1, "`{name}` evaluates one operand");
    }

    let expected_intrinsics = [
        "F32::is_finite.f32",
        "F32::is_infinite.f32",
        "F32::is_normal.f32",
        "F32::is_subnormal.f32",
        "F64::is_finite.f64",
        "F64::is_infinite.f64",
        "F64::is_normal.f64",
        "F64::is_subnormal.f64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_intrinsics, expected_intrinsics);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        8,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per predicate/format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "classification builtins must interpret"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-float-classification-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named float classification calls should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named float classification canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "classification predicates must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-classification-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("classification calls should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_classify_preserves_enum_layout_and_executes() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.classify-enum.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "FloatClass eight-byte layout and source-order tags",
        "FloatClass sign payload at byte four",
        "binary32 all class tags and signed payloads",
        "binary64 all class tags and signed payloads",
        "exactly-once unary evaluation shape",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x9a27_9424_1f02_d5fa;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_CLASSIFY_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("named float classify calls should compile to checked trees");
    let layouts = layout::build_layout_plan(&checked, target::NativeTarget::host(), &[])
        .expect("FloatClass layout should build");
    let float_class = layouts
        .data_layouts
        .iter()
        .find(|(_, layout)| layout.name.as_str() == "FloatClass")
        .map(|(_, layout)| layout)
        .expect("FloatClass layout");
    assert_eq!(float_class.layout.size, 8, "packed intrinsic store width");
    assert_eq!(float_class.layout.alignment, 4, "tag alignment");
    let layout::DataShape::Enum { variants, .. } = &float_class.shape else {
        panic!("FloatClass must remain an enum");
    };
    let variants = layouts.variants.span_or_empty(*variants);
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        ["NaN", "Infinity", "Normal", "Subnormal", "Zero"],
        "native tags are source-order ordinals"
    );
    for variant in &variants[1..] {
        let [negative] = layouts.fields.span_or_empty(variant.fields) else {
            panic!("{} must carry exactly one sign payload", variant.name);
        };
        assert_eq!(negative.name.as_str(), "negative");
        assert_eq!(negative.offset, 4, "sign payload follows the i32 tag");
        assert_eq!(negative.layout.size, 1, "sign payload is a bool");
    }

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_report_fingerprint == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
            .expect("named classify evidence must retain its plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named classify plan must contain one row");
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding else {
            panic!("named classify plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::classify.") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());
        let (expected_builtin, expected_target) = if name.starts_with("F32::") {
            (
                symbols::BuiltinFunction::FloatClassifyF32,
                "float#classify_f32",
            )
        } else {
            (
                symbols::BuiltinFunction::FloatClassifyF64,
                "float#classify_f64",
            )
        };
        let typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must remain a unary builtin call");
        };
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(expected_builtin)
        );
        assert_eq!(call.target.as_str(), expected_target);
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 1);
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::classify.f32".to_owned(),
            "F64::classify.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        2,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 70, "classify builtin must interpret");

    let build_dir =
        std::env::temp_dir().join(format!("omega-named-float-classify-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named float classify calls should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named float classify canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "classify must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-classify-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| panic!("classify calls should compile for {target}: {diagnostics:#?}"),
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_multiply_then_add_preserves_two_roundings_and_executes() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.multiply-then-add.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 cancellation edge",
        "binary64 cancellation edge",
        "two distinct roundings",
        "binary32 finite-overflow saturation",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x3469_73b6_84ba_8c5d;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_MULTIPLY_THEN_ADD_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("named multiply-then-add provider calls should compile to checked trees");

    let main_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main machine");
    let main_state = checked
        .typed
        .machine_states(main_machine)
        .first()
        .expect("Main::main entry state");
    let typed_trees::statement::StatementNode::Assignment(result32_assignment) = &checked
        .typed
        .statement_table
        .statements(main_state.statement_nodes)[2]
    else {
        panic!("Main::main statement 2 must assign result32");
    };
    let result32_origin = checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: main_machine.symbol,
        state_symbol: main_state.symbol,
        statement_index: 2,
        role: checked_trees::CheckedValueStatementRole::AssignmentValue,
    };
    let outer_add = checked
        .facts
        .operators
        .expression_use_in_origin(result32_assignment.value, result32_origin)
        .expect("the primitive add surrounding multiply-then-add must retain checked evidence");
    assert_eq!(
        outer_add.spelling,
        language_core::operator_spelling::OperatorSpelling::Add
    );
    assert_ne!(outer_add.provider_plan_report_fingerprint, 0);
    assert!(
        checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(outer_add.provider_plan_report_fingerprint)
            .is_some()
    );
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_report_fingerprint == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
            .expect("named operator evidence must resolve to its retained plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named float plan must contain exactly one row");
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding else {
            panic!("named float plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());

        let typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must preserve its selected root as a compiler call");
        };
        assert_eq!(call.arguments.count(), 3);
        assert!(!call.receiver.is_valid());
        let expected_builtin = match name.as_str() {
            "F32::multiply_then_add.f32" => symbols::BuiltinFunction::FloatMultiplyThenAddF32,
            "F64::multiply_then_add.f64" => symbols::BuiltinFunction::FloatMultiplyThenAddF64,
            _ => panic!("unexpected multiply-then-add intrinsic `{name}`"),
        };
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(expected_builtin)
        );
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::multiply_then_add.f32".to_owned(),
            "F64::multiply_then_add.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        2,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "selected multiply-then-add must keep its two-rounding semantics in the interpreter"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-float-multiply-then-add-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "named multiply-then-add provider calls should compile from their authored native root",
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named multiply-then-add provider canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "selected multiply-then-add must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-multiply-then-add-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
            panic!(
                "named multiply-then-add provider calls should compile for {target}: {diagnostics:#?}"
            )
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_fused_multiply_add_selects_aarch64_fmadd_and_executes() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.nearest-fma.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 cancellation edge",
        "binary64 cancellation edge",
        "single fused rounding",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xbb3f_d600_7ddf_03c0;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("named FMA provider calls should compile to checked trees on macOS AArch64");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    // Core spells `F32::fused_multiply_add`/`F64::fused_multiply_add` as
    // top-level boundary requirements, so the evidence is a named requirement
    // use rather than a named operator use.
    for (expression, provider_plan_report_fingerprint) in super::stamped_named_uses(&checked) {
        let plan = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
            .expect("named FMA evidence must resolve to its retained plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named FMA plan must contain exactly one row");
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding else {
            panic!("named FMA plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("fused_multiply_add") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());

        let typed_trees::expression::ExpressionNode::Call(call) =
            checked.typed.expression_table.expression(expression)
        else {
            panic!("`{name}` must preserve its selected root as a compiler call");
        };
        assert_eq!(call.arguments.count(), 3);
        assert!(!call.receiver.is_valid());
        let expected_builtin = match name.as_str() {
            "F32::fused_multiply_add.f32" => symbols::BuiltinFunction::FloatFusedMultiplyAddF32,
            "F64::fused_multiply_add.f64" => symbols::BuiltinFunction::FloatFusedMultiplyAddF64,
            _ => panic!("unexpected FMA intrinsic `{name}`"),
        };
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(expected_builtin)
        );
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::fused_multiply_add.f32".to_owned(),
            "F64::fused_multiply_add.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        2,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "selected FMA must preserve its single-rounding semantics in the interpreter"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-named-float-fma-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named FMA provider calls should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named FMA provider canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "selected FMA must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-named-float-fma-linux-arm64-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_target(&canary, scratch.join("out"), "linux_arm64").unwrap_or_else(
        |diagnostics| {
            panic!("named FMA provider calls should compile for linux_arm64: {diagnostics:#?}")
        },
    );
    let _ = fs::remove_dir_all(&scratch);

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_directed_fused_multiply_add_selects_aarch64_fmadd_and_executes() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-fma.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 half-ULP edge",
        "binary64 half-ULP edge",
        "toward zero",
        "toward positive",
        "toward negative",
        "single fused rounding",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x4b6d_5c3b_9fb5_54a6;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_DIRECTED_FUSED_MULTIPLY_ADD_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("directed-FMA provider calls should compile to checked trees on macOS AArch64");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding else {
            continue;
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::fused_multiply_add_toward_") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());

        let typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must rewrite to an unnameable compiler call");
        };
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 3);
        assert!(
            call.target
                .as_str()
                .starts_with("float#fused_multiply_add_toward_")
        );
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::fused_multiply_add_toward_negative.f32",
            "F32::fused_multiply_add_toward_positive.f32",
            "F32::fused_multiply_add_toward_zero.f32",
            "F64::fused_multiply_add_toward_negative.f64",
            "F64::fused_multiply_add_toward_positive.f64",
            "F64::fused_multiply_add_toward_zero.f64",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format/direction slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "directed-FMA interpreter semantics must distinguish half-ULP edges"
    );

    let build_dir = std::env::temp_dir().join(format!("omega-directed-fma-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("directed-FMA providers should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-FMA provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed FMA must restore nearest-even before ordinary FMA; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let scratch = std::env::temp_dir().join(format!(
        "omega-directed-fma-linux-arm64-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_target(&canary, scratch.join("out"), "linux_arm64").unwrap_or_else(
        |diagnostics| {
            panic!("directed-FMA providers should compile for linux_arm64: {diagnostics:#?}")
        },
    );
    let _ = fs::remove_dir_all(&scratch);

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}
