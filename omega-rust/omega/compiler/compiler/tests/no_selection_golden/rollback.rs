use compiler::{CompileOptions, CompileRequest, OptimizationRollback, RequestedCompileProduct};
use optimization_core::Optimization;
use std::sync::atomic::{AtomicU64, Ordering};

use super::support::{
    HOSTED_NATIVE_TARGETS, compile_retained_native, repo_root, retained_native_snapshot,
};

static ROLLBACK_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn selected_canary() -> std::path::PathBuf {
    repo_root()
        .join("tests/omega/pass")
        .join(super::fixture_roster::ROLLBACK_TO_NO_SELECTION_EMPTY_ENTRY)
}

fn build_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "omega-optimizer-rollback-{label}-{}-{}",
        std::process::id(),
        ROLLBACK_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn request(target: &str, output_dir: std::path::PathBuf) -> CompileRequest {
    request_for(
        selected_canary().join("main.omg"),
        target,
        output_dir,
        [
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ],
    )
}

fn request_for(
    root_path: std::path::PathBuf,
    target: &str,
    output_dir: std::path::PathBuf,
    disabled: impl IntoIterator<Item = Optimization>,
) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path,
        build_dir: Some(output_dir),
        target_name: Some(target.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::NativeArtifact)
    .with_optimization_rollback(
        OptimizationRollback::new(disabled).expect("the rollback request is duplicate-free"),
    )
}

fn dead_scalar_project(
    project_dir: std::path::PathBuf,
    select_dead_scalar: bool,
) -> std::path::PathBuf {
    std::fs::create_dir_all(&project_dir).expect("create the dead-scalar project directory");
    std::fs::write(
        project_dir.join("main.omg"),
        "data Main { }\nmachine Main::main(&mut self) {\n    let dead_const: i32 = 7;\n}\n",
    )
    .expect("write the dead-scalar project root");
    let mut build_source = concat!(
        "machine build(builder: &mut Build) {\n",
        "    builder.application(\"dead-scalar-rollback\");\n",
        "    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);\n",
        "    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n",
        "    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);\n",
        "    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);\n",
    )
    .to_owned();
    if select_dead_scalar {
        build_source.push_str(
            "    builder.optimizations.enable(Optimization::DeadPureScalarElimination);\n",
        );
    }
    build_source.push_str("}\n");
    std::fs::write(project_dir.join("build.omg"), build_source)
        .expect("write the dead-scalar build file");
    project_dir.join("main.omg")
}

fn compile_ordinary_retained_native(
    root_path: std::path::PathBuf,
    target: &str,
) -> compiler::RetainedNativeArtifact {
    let output_dir = build_dir(target);
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path,
            build_dir: Some(output_dir.clone()),
            target_name: Some(target.to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!("ordinary compilation for {target} failed: {diagnostics:#?}")
    });
    let artifact = report
        .into_retained_native_artifact()
        .expect("native compilation must retain its artifact");
    artifact
        .validate()
        .expect("the retained ordinary artifact must replay");
    let _ = std::fs::remove_dir_all(output_dir);
    artifact
}

#[test]
fn rollback_to_empty_selection_rejoins_exact_ordinary_path_on_every_target() {
    for target in HOSTED_NATIVE_TARGETS {
        let output_dir = build_dir(target);
        let report = compiler::compile(request(target, output_dir.clone()))
            .and_then(compiler::CompileOutcomes::into_single_report)
            .unwrap_or_else(|diagnostics| panic!("rollback compilation failed: {diagnostics:#?}"));
        let receipt = report
            .optimization_rollback_receipt()
            .expect("a nonempty rollback request must leave custody");
        assert_eq!(
            receipt.build_selected().as_slice(),
            &[Optimization::ControlFlowCleanup],
            "{target}"
        );
        assert_eq!(
            receipt.requested_disabled().as_slice(),
            &[
                Optimization::ControlFlowCleanup,
                Optimization::CopyPropagation,
            ],
            "{target}"
        );
        assert_eq!(
            receipt.actually_disabled().as_slice(),
            &[Optimization::ControlFlowCleanup],
            "{target}"
        );
        assert!(receipt.effective().is_empty(), "{target}");

        let rolled_back = report
            .into_retained_native_artifact()
            .expect("native compilation must retain its artifact");
        let ordinary = compile_retained_native(target);
        assert_eq!(
            retained_native_snapshot(target, &rolled_back),
            retained_native_snapshot(target, &ordinary),
            "{target}"
        );
        assert_eq!(
            rolled_back.semantic_bytes(),
            ordinary.semantic_bytes(),
            "{target}"
        );
        assert_eq!(
            rolled_back.proof_bytes(),
            ordinary.proof_bytes(),
            "{target}"
        );
        assert_eq!(
            rolled_back.object().text_bytes(),
            ordinary.object().text_bytes(),
            "{target}"
        );
        assert_eq!(
            rolled_back.image().output().bytes,
            ordinary.image().output().bytes,
            "{target}"
        );
        let _ = std::fs::remove_dir_all(output_dir);
    }
}

#[test]
fn dead_scalar_rollback_rejoins_exact_ordinary_path_on_every_target() {
    for target in HOSTED_NATIVE_TARGETS {
        let pair_dir = build_dir("dead-scalar-pair");
        let selected_root = dead_scalar_project(pair_dir.join("selected"), true);
        let ordinary_root = dead_scalar_project(pair_dir.join("ordinary"), false);
        let output_dir = build_dir(target);
        let report = compiler::compile(request_for(
            selected_root,
            target,
            output_dir.clone(),
            [Optimization::DeadPureScalarElimination],
        ))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("rollback compilation failed: {diagnostics:#?}"));
        let receipt = report
            .optimization_rollback_receipt()
            .expect("a nonempty rollback request must leave custody");
        assert_eq!(
            receipt.build_selected().as_slice(),
            &[Optimization::DeadPureScalarElimination],
            "{target}"
        );
        assert_eq!(
            receipt.requested_disabled().as_slice(),
            &[Optimization::DeadPureScalarElimination],
            "{target}"
        );
        assert_eq!(
            receipt.actually_disabled().as_slice(),
            &[Optimization::DeadPureScalarElimination],
            "{target}"
        );
        assert!(receipt.effective().is_empty(), "{target}");

        let rolled_back = report
            .into_retained_native_artifact()
            .expect("native compilation must retain its artifact");
        let ordinary = compile_ordinary_retained_native(ordinary_root, target);
        assert_eq!(
            retained_native_snapshot(target, &rolled_back),
            retained_native_snapshot(target, &ordinary),
            "{target}"
        );
        assert_eq!(
            rolled_back.semantic_bytes(),
            ordinary.semantic_bytes(),
            "{target}"
        );
        assert_eq!(
            rolled_back.proof_bytes(),
            ordinary.proof_bytes(),
            "{target}"
        );
        assert_eq!(
            rolled_back.object().text_bytes(),
            ordinary.object().text_bytes(),
            "{target}"
        );
        assert_eq!(
            rolled_back.image().output().bytes,
            ordinary.image().output().bytes,
            "{target}"
        );
        let _ = std::fs::remove_dir_all(output_dir);
        let _ = std::fs::remove_dir_all(pair_dir);
    }
}

#[test]
fn native_rollback_rejects_products_that_do_not_enter_native_realization() {
    for product in [
        RequestedCompileProduct::Check,
        RequestedCompileProduct::TerminalArtifact,
    ] {
        let root = build_dir("must-not-read-source").join("missing.omg");
        let diagnostics = compiler::compile(
            CompileRequest::new(CompileOptions {
                root_path: root,
                build_dir: None,
                target_name: None,
            })
            .with_requested_product(product)
            .with_optimization_rollback(
                OptimizationRollback::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                    .unwrap(),
            ),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect_err("rollback cannot appear honored without native realization");
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("`SelectedIncomingU12ExactAddImmediate`")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("names stages not executed by")
        );
        assert!(diagnostics[0].message.contains(&format!("{product:?}")));
        assert!(!diagnostics[0].message.contains("failed to read"));
    }
}

#[test]
fn empty_rollback_request_leaves_no_release_receipt() {
    let output_dir = build_dir("empty-request");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: super::support::native_canary().join("main.omg"),
            build_dir: Some(output_dir.clone()),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("ordinary native compilation must succeed");
    assert!(report.optimization_rollback_receipt().is_none());
    let _ = std::fs::remove_dir_all(output_dir);
}
