//! Publication replay for
//! `SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization`.
//!
//! The selected-lowering catalog row resolves through the physical-selection
//! gate, the fold stage executes to its validated fixed point inside a real
//! compilation, and the retained native artifact replays the exact physical
//! result independently. This file exists separately from `optimizer_opt_in`
//! so the saturating-divide zero-dividend family keeps its own witnessed
//! coverage.

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static PROJECT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn project(label: &str, build: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-saturating-divide-zero-dividend-zero-materialization-{label}-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)
        .expect("create saturating-divide-zero-dividend-zero-materialization project");
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: u8; }\nmachine Main::main(&mut self) { }\n",
    )
    .expect("write saturating-divide-zero-dividend-zero-materialization main");
    std::fs::write(root.join("build.omg"), build)
        .expect("write saturating-divide-zero-dividend-zero-materialization build");
    root
}

#[test]
fn return_only_saturating_divide_zero_dividend_zero_materialization_rejoins_native_artifact_production()
 {
    let root = project(
        "rejoin",
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer_saturating_divide_zero_dividend_zero_materialization_rejoin");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization);
}
"#,
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect(
        "the exact return-only saturating-divide-zero-dividend-zero-materialization selection \
         should reach native custody",
    );
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    assert!(
        artifact
            .physical_evidence()
            .expect("empty D32 coverage remains exact")
            .children()
            .is_empty()
    );
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}
