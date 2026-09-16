use super::fixture_roster;
use super::{
    retained_float_differential_result_identity,
    retained_float_policy_differential_result_identity, selected_intrinsic_diagnostic_label,
};
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_reviewed_repository_fixture,
    compile_rooted_canary_for_native_host, compile_rooted_canary_for_target, executable_name,
    fail_canary, fs, hosted_main_program_entry_build_for, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn named_float_directed_add_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-add.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 half-ULP tie",
        "binary64 half-ULP tie",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x7e9b_cd52_c66c_6510;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_DIRECTED_ADD_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("directed-add provider calls should compile to checked trees");

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
        if !name.contains("::add_toward_") {
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
        assert_eq!(call.arguments.count(), 2);
        assert!(call.target.as_str().starts_with("float#add_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::add_toward_negative.f32",
            "F32::add_toward_positive.f32",
            "F32::add_toward_zero.f32",
            "F64::add_toward_negative.f64",
            "F64::add_toward_positive.f64",
            "F64::add_toward_zero.f64",
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
        "directed-add interpreter semantics must distinguish half-ULP edges"
    );

    let build_dir = std::env::temp_dir().join(format!("omega-directed-add-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("directed-add providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-add provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed add must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-add-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("directed-add cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-add canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write directed-add build source");
        compile(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-add providers should compile for {target}: {diagnostics:#?}")
        });
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
fn named_float_directed_subtract_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-subtract.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 midpoint",
        "binary64 midpoint",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xb40d_f240_a7b2_6e47;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_DIRECTED_SUBTRACT_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("directed-subtract provider calls should compile to checked trees");

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
        if !name.contains("::subtract_toward_") {
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
        assert_eq!(call.arguments.count(), 2);
        assert!(call.target.as_str().starts_with("float#subtract_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::subtract_toward_negative.f32",
            "F32::subtract_toward_positive.f32",
            "F32::subtract_toward_zero.f32",
            "F64::subtract_toward_negative.f64",
            "F64::subtract_toward_positive.f64",
            "F64::subtract_toward_zero.f64",
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
        "directed-subtract interpreter semantics must distinguish midpoint edges"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-directed-subtract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("directed-subtract providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-subtract provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed subtract must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-subtract-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("directed-subtract cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-subtract canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write directed-subtract build source");
        compile(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-subtract providers should compile for {target}: {diagnostics:#?}")
        });
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
fn named_float_directed_multiply_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-multiply.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 exact-product edge",
        "binary64 exact-product edge",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x4411_b314_20a5_c04b;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_DIRECTED_MULTIPLY_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("directed-multiply provider calls should compile to checked trees");

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
        if !name.contains("::multiply_toward_") {
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
        assert_eq!(call.arguments.count(), 2);
        assert!(call.target.as_str().starts_with("float#multiply_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::multiply_toward_negative.f32",
            "F32::multiply_toward_positive.f32",
            "F32::multiply_toward_zero.f32",
            "F64::multiply_toward_negative.f64",
            "F64::multiply_toward_positive.f64",
            "F64::multiply_toward_zero.f64",
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
        "directed-multiply interpreter semantics must distinguish exact-product edges"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-directed-multiply-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("directed-multiply providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-multiply provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed multiply must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-multiply-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("directed-multiply cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-multiply canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write directed-multiply build source");
        compile(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-multiply providers should compile for {target}: {diagnostics:#?}")
        });
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
fn named_float_directed_divide_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-divide.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 exact-quotient edge",
        "binary64 exact-quotient edge",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x5e1f_542f_ee21_0fd9;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_DIRECTED_DIVIDE_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("directed-divide provider calls should compile to checked trees");

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
        if !name.contains("::divide_toward_") {
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
        assert_eq!(call.arguments.count(), 2);
        assert!(call.target.as_str().starts_with("float#divide_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::divide_toward_negative.f32",
            "F32::divide_toward_positive.f32",
            "F32::divide_toward_zero.f32",
            "F64::divide_toward_negative.f64",
            "F64::divide_toward_positive.f64",
            "F64::divide_toward_zero.f64",
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
        "directed-divide interpreter semantics must distinguish exact-quotient edges"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-directed-divide-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("directed-divide providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-divide provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed divide must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-divide-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("directed-divide cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-divide canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write directed-divide build source");
        compile(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-divide providers should compile for {target}: {diagnostics:#?}")
        });
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
fn named_float_directed_square_root_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-square-root.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 irrational-result edge",
        "binary64 irrational-result edge",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x5bfe_5610_aa74_88bf;

    let canary = pass_canary(fixture_roster::FLOAT_NAMED_PROVIDER_DIRECTED_SQUARE_ROOT_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("directed-square-root provider calls should compile to checked trees");

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
        if !name.contains("::square_root_toward_") {
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
        assert_eq!(call.arguments.count(), 1);
        assert!(call.target.as_str().starts_with("float#sqrt_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::square_root_toward_negative.f32",
            "F32::square_root_toward_positive.f32",
            "F32::square_root_toward_zero.f32",
            "F64::square_root_toward_negative.f64",
            "F64::square_root_toward_positive.f64",
            "F64::square_root_toward_zero.f64",
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
        "directed-square-root interpreter semantics must distinguish irrational-result edges"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-directed-square-root-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("directed-square-root providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-square-root provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed square root must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-square-root-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir)
            .expect("directed-square-root cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-square-root canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write directed-square-root build source");
        compile(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-square-root providers should compile for {target}: {diagnostics:#?}")
        });
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
fn float_policy_operator_uses_record_checked_result_adapters() {
    for &(canary_name, expected) in fixture_roster::POLICY_ADAPTER_PASS_CANARIES {
        let canary = pass_canary(canary_name);
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            None,
        ))
        .unwrap_or_else(|diagnostics| {
            panic!("{canary_name} should compile to checked policy evidence: {diagnostics:?}")
        });
        assert!(
            checked
                .facts
                .operators
                .resolved_uses()
                .any(|operator_use| operator_use.policy_adapter == expected),
            "{canary_name} should retain `{expected:?}` beside its selected float operator"
        );
    }
}

#[test]
fn nested_attached_float_policy_operators_retain_checked_selected_evidence() {
    let canary = pass_canary(fixture_roster::ARITHMETIC_FLOAT_SATURATING_OVERFLOW_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("nested attached-data float policy canary should check");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main machine");
    for (state_name, statement_index) in [("negative", 0), ("nested", 0), ("nested32", 1)] {
        let state = checked
            .typed
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == state_name)
            .unwrap_or_else(|| panic!("Main::main::{state_name} state"));
        let typed_trees::statement::StatementNode::Assignment(assignment) = &checked
            .typed
            .statement_table
            .statements(state.statement_nodes)[statement_index]
        else {
            panic!("Main::main::{state_name} statement {statement_index} must be an assignment");
        };
        let origin = checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
            role: checked_trees::CheckedValueStatementRole::AssignmentValue,
        };
        let operator_use = checked
            .facts
            .operators
            .expression_use_in_origin(assignment.value, origin)
            .unwrap_or_else(|| {
                panic!(
                    "Main::main::{state_name} statement {statement_index} must retain its outer operator"
                )
            });
        assert!(matches!(
            operator_use.policy_adapter,
            checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { .. }
        ));
        assert_ne!(operator_use.provider_plan_report_fingerprint, 0);
        assert!(
            checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
                .is_some()
        );
    }
}

#[test]
fn float_policy_adapters_retain_differential_results() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.policy-adapters.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32/binary64 finite results under both adapters",
        "binary32/binary64 finite-overflow saturation",
        "nested saturation and repeated clamping",
        "division by zero remains unclamped under Saturating",
        "Trapping finite overflow",
        "Trapping division by zero and invalid operation",
        "Trapping propagated NaN and infinity",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x0115_beff_3918_3c7f;

    let mut selected_evidence = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    let mut observations = Vec::new();
    let mut cross_builds = std::collections::BTreeSet::new();
    let case_filter = std::env::var("OMEGA_FLOAT_POLICY_CASE_FILTER").ok();
    let selected_cases = fixture_roster::POLICY_DIFFERENTIAL_PASS_CANARIES
        .iter()
        .filter(|(case_name, _, _)| {
            case_filter
                .as_deref()
                .is_none_or(|filter| case_name.contains(filter))
        })
        .collect::<Vec<_>>();
    assert!(
        !selected_cases.is_empty(),
        "OMEGA_FLOAT_POLICY_CASE_FILTER selected no differential cases"
    );
    for (case_name, expected_exit, expected_error) in selected_cases.iter().copied() {
        let canary = pass_canary(case_name);
        let main_path = canary.join("main.omg");
        let checked =
            compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
                .unwrap_or_else(|diagnostics| {
                    panic!(
                        "{case_name} should compile to checked policy evidence: {diagnostics:#?}"
                    )
                });
        for operator_use in checked.facts.operators.resolved_uses() {
            let adapter = match operator_use.policy_adapter {
                checked_trees::CheckedArithmeticPolicyAdapter::None => continue,
                checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                    format,
                } => {
                    if format == numerics::float_semantics::FloatFormat::BINARY32 {
                        "saturating-overflow-only.binary32"
                    } else {
                        assert_eq!(
                            format,
                            numerics::float_semantics::FloatFormat::BINARY64,
                            "policy evidence retained an unsupported float format"
                        );
                        "saturating-overflow-only.binary64"
                    }
                }
                checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                    format,
                } => {
                    if format == numerics::float_semantics::FloatFormat::BINARY32 {
                        "trapping-nonfinite.binary32"
                    } else {
                        assert_eq!(
                            format,
                            numerics::float_semantics::FloatFormat::BINARY64,
                            "policy evidence retained an unsupported float format"
                        );
                        "trapping-nonfinite.binary64"
                    }
                }
            };
            let plan = checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
                .expect("policy-adapted float evidence must retain its selected plan");
            let [row] = plan.rows.as_slice() else {
                panic!("policy-adapted float plan must retain one realization row");
            };
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
            else {
                panic!("policy-adapted float plan must select a compiler intrinsic");
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            selected_evidence.insert(format!("{name}|{adapter}"));
            selected_plan_identities.push(plan.report_fingerprint());
        }

        let outcome = interpret(&checked, &[]);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-float-policy-differential-{}-{}",
            case_name.replace('/', "-"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let native_compile = if matches!(
            *case_name,
            "arithmetic/float_trapping_overflow_traps"
                | "arithmetic/float_trapping_divzero_traps"
                | "arithmetic/float_trapping_invalid_traps"
                | "float/float_trapping_propagated_nan_traps"
                | "float/float_trapping_propagated_infinity_traps"
        ) {
            compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        } else {
            compile(CanaryCompileSpec {
                root_path: main_path,
                build_dir: Some(build_dir.clone()),
                target_name: None,
                product: CanaryCompileProduct::NativeArtifactAndPublish,
            })
        };
        native_compile.unwrap_or_else(|diagnostics| {
            panic!("{case_name} should compile natively: {diagnostics:#?}")
        });
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .unwrap_or_else(|error| panic!("{case_name} should run natively: {error}"));
        let _ = fs::remove_dir_all(&build_dir);

        if let Some(expected_exit) = expected_exit {
            assert_eq!(
                outcome.exit_code, *expected_exit,
                "{case_name} interpreter result changed: {:?}",
                outcome.error
            );
            assert_eq!(
                output.status.code(),
                Some(*expected_exit),
                "{case_name} native result changed; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            let error = outcome
                .error
                .as_deref()
                .unwrap_or_else(|| panic!("{case_name} interpreter should reject the result"));
            assert!(
                error.contains(expected_error.expect("trapping case error fragment")),
                "{case_name} interpreter error changed: {error}"
            );
            assert!(
                !output.status.success(),
                "{case_name} native execution reached its sailed-past sentinel"
            );
        }
        observations.push(((*case_name).to_owned(), outcome, output));

        for target in ["linux_x86_64", "linux_arm64"] {
            let scratch = std::env::temp_dir().join(format!(
                "omega-float-policy-differential-{}-{target}-{}",
                case_name.replace('/', "-"),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&scratch);
            let cross_compile = if matches!(
                *case_name,
                "float/float_trapping_propagated_nan_traps"
                    | "float/float_trapping_propagated_infinity_traps"
            ) {
                compile_rooted_canary_for_target(&canary, scratch.join("out"), target)
            } else {
                let source_dir = scratch.join("src");
                fs::create_dir_all(&source_dir)
                    .expect("float-policy cross-target source directory");
                fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
                    .expect("copy float-policy canary");
                fs::write(
                    source_dir.join("build.omg"),
                    hosted_main_program_entry_build_for(&canary, target),
                )
                .expect("write float-policy build source");
                compile(CanaryCompileSpec {
                    root_path: source_dir.join("main.omg"),
                    build_dir: Some(scratch.join("out")),
                    target_name: Some(target.to_owned()),
                    product: CanaryCompileProduct::NativeArtifactAndPublish,
                })
            };
            cross_compile.unwrap_or_else(|diagnostics| {
                panic!("{case_name} should compile for {target}: {diagnostics:#?}")
            });
            let _ = fs::remove_dir_all(&scratch);
            cross_builds.insert(format!("{case_name}@{target}"));
        }
    }

    if case_filter.is_some() {
        assert_eq!(cross_builds.len(), selected_cases.len() * 2);
        return;
    }

    let expected_evidence = [
        "Float::add.f32|saturating-overflow-only.binary32",
        "Float::add.f32|trapping-nonfinite.binary32",
        "Float::add.f64|saturating-overflow-only.binary64",
        "Float::add.f64|trapping-nonfinite.binary64",
        "Float::divide.f32|saturating-overflow-only.binary32",
        "Float::divide.f32|trapping-nonfinite.binary32",
        "Float::divide.f64|saturating-overflow-only.binary64",
        "Float::divide.f64|trapping-nonfinite.binary64",
        "Float::multiply.f32|saturating-overflow-only.binary32",
        "Float::multiply.f32|trapping-nonfinite.binary32",
        "Float::multiply.f64|saturating-overflow-only.binary64",
        "Float::multiply.f64|trapping-nonfinite.binary64",
        "Float::subtract.f32|saturating-overflow-only.binary32",
        "Float::subtract.f32|trapping-nonfinite.binary32",
        "Float::subtract.f64|saturating-overflow-only.binary64",
        "Float::subtract.f64|trapping-nonfinite.binary64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_evidence, expected_evidence);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        8,
        "{DIFFERENTIAL_SUITE_ID} must bind all four primitive plans in both formats"
    );
    assert_eq!(
        cross_builds.len(),
        fixture_roster::POLICY_DIFFERENTIAL_PASS_CANARIES.len() * 2
    );

    let observation_refs = observations
        .iter()
        .map(|(label, outcome, output)| (label.as_str(), outcome, output))
        .collect::<Vec<_>>();
    let result_identity = retained_float_policy_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_evidence,
        &selected_plan_identities,
        &observation_refs,
        &cross_builds,
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, adapter evidence, interpreter/native observations, and cross-target builds before refreshing the retained identity"
    );
}

// Without a declaration, mint, or signature selection, the domain meaning is
// inactive and the ordinary builtin operation stays selected.
// The evidence must say so explicitly (builtin fallback), not pretend the
// domain meaning won.

#[test]
fn domain_operator_selection_records_builtin_fallback_without_binding_selection() {
    let canary =
        pass_canary(fixture_roster::DOMAINS_DOMAIN_OPERATOR_UNPROVEN_KEEPS_BUILTIN_MEANING);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("unselected builtin fallback canary should compile to checked trees");

    let fallback_uses = checked
        .facts
        .operators
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback)
        .count();
    assert!(
        fallback_uses > 0,
        "expected the unselected `i32::Degrees` meaning to leave builtin `+` active \
         and record the use as a builtin fallback"
    );
    assert_eq!(
        checked
            .facts
            .operators
            .resolved_uses()
            .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
            .filter(|candidate| candidate.is_domain_owned())
            .count(),
        0,
        "no domain-owned meaning may be selected without a declaration, mint, or signature selection"
    );
}

// Same-carrier domain theories coexist until an operand binding actually
// selects them. An unrelated or inactive declaration cannot inject a meaning
// into an existing expression.

#[test]
fn domain_operator_inactive_same_carrier_meanings_coexist() {
    let canary =
        pass_canary(fixture_roster::DOMAINS_DOMAIN_OPERATOR_INACTIVE_SAME_CARRIER_COEXISTS);
    compile_reviewed_repository_fixture(CheckedCompileRequest::new(&canary.join("main.omg"), None))
        .expect("inactive same-carrier domain meanings should coexist");
}

// When one binding statically selects both domains, both meanings participate
// in this use and the checked resolution must reject the ambiguity.

#[test]
fn domain_operator_competing_binding_meanings_fail_at_use_site() {
    let canary = fail_canary(fixture_roster::DOMAINS_DOMAIN_OPERATOR_COMPETING_SPELLING_MEANINGS);
    let diagnostics = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect_err("competing selected domain meanings should fail");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("ambiguous operator spelling `+`")
                && diagnostic.message.contains("static operand-domain tuple")
        }),
        "expected use-site operator ambiguity diagnostic, got: {diagnostics:?}"
    );
}
