use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, OptimizationRollback,
    RequestedCompileProduct,
};
use omega_optimization_core::Optimization;
use std::sync::atomic::{AtomicU64, Ordering};

use super::support::{
    HOSTED_NATIVE_TARGETS, compile_retained_native, repo_root, retained_native_snapshot,
};

static ROLLBACK_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn selected_canary() -> std::path::PathBuf {
    repo_root().join("tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry")
}

fn build_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "omega-optimizer-rollback-{label}-{}-{}",
        std::process::id(),
        ROLLBACK_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn request(target: &str, output_dir: std::path::PathBuf) -> CompileRequest {
    request_for(selected_canary().join("main.omg"), target, output_dir)
}

fn request_for(
    root_path: std::path::PathBuf,
    target: &str,
    output_dir: std::path::PathBuf,
) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path,
        build_dir: Some(output_dir),
        target_name: Some(target.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::NativeArtifact)
    .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
    .with_optimization_rollback(
        OptimizationRollback::new([
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .expect("the rollback request is duplicate-free"),
    )
}

#[test]
fn rollback_to_empty_selection_rejoins_exact_ordinary_path_on_every_target() {
    for target in HOSTED_NATIVE_TARGETS {
        let output_dir = build_dir(target);
        let report = omega_compiler::compile(request(target, output_dir.clone()))
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
fn nonempty_rollback_rejects_products_that_do_not_enter_native_realization() {
    for product in [
        RequestedCompileProduct::Check,
        RequestedCompileProduct::TerminalArtifact,
    ] {
        let root = build_dir("must-not-read-source").join("missing.omg");
        let diagnostics = omega_compiler::compile(
            CompileRequest::new(CompileOptions {
                root_path: root,
                build_dir: None,
                target_name: None,
            })
            .with_requested_product(product)
            .with_optimization_rollback(
                OptimizationRollback::new([Optimization::ControlFlowCleanup]).unwrap(),
            ),
        )
        .expect_err("rollback cannot appear honored without native realization");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("`ControlFlowCleanup`"));
        assert!(
            diagnostics[0]
                .message
                .contains("requires NativeArtifact production")
        );
        assert!(diagnostics[0].message.contains(&format!("{product:?}")));
        assert!(!diagnostics[0].message.contains("failed to read"));
    }
}

#[test]
fn empty_rollback_request_leaves_no_release_receipt() {
    let output_dir = build_dir("empty-request");
    let report = omega_compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: super::support::native_canary().join("main.omg"),
            build_dir: Some(output_dir.clone()),
            target_name: Some("linux_x64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect("ordinary native compilation must succeed");
    assert!(report.optimization_rollback_receipt().is_none());
    let _ = std::fs::remove_dir_all(output_dir);
}
