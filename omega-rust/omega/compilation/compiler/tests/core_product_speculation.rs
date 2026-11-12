//! The core Build's Product sources are typed on a fork of its preparation
//! while the core Build is checked. Completion adopts the fork only when
//! nothing was admitted to the preparation meanwhile; otherwise it discards
//! the fork and types the sources itself. Both paths must publish the same
//! bytes, or a compile's output would depend on which path it took.
//!
//! No bundled core Build admits a source while it runs, so this witness forces
//! the refusal through the compiler's test seam and compares the Terminal
//! artifact and its Product diagnostics with an adopting compile.
//!
//! The seam and counters are process-global, so this target holds one test.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};

// A package with a selected program entry, so it reaches a Terminal artifact.
const FIXTURE: &str =
    "tests/omega/pass/providers/checked_boundary_requirement_dispatch_exit/main.omg";

fn terminal_bytes(run: &str) -> Vec<u8> {
    let target_name = "linux_x86_64";
    let main = repo_root().join(FIXTURE);
    let prepared = reviewed_repository_fixture_package_inputs(&main, Some(target_name))
        .unwrap_or_else(|diagnostics| panic!("{FIXTURE} package inputs: {diagnostics:#?}"));
    let build_dir = std::env::temp_dir().join(format!(
        "omega-core-product-speculation-{}-{run}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&build_dir);
    let mut request = CompileRequest::new(CompileOptions {
        root_path: prepared.root_path,
        build_dir: Some(build_dir),
        target_name: Some(target_name.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = prepared.package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    orchestration::compile_sources(&mut compiler::CompilationSession::default(), request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{FIXTURE} {run}: {diagnostics:#?}"))
        .into_retained_terminal_artifact()
        .unwrap_or_else(|| panic!("{FIXTURE} {run} retains its Terminal artifact"))
        .artifact()
        .to_bytes()
}

#[test]
fn refused_core_product_speculation_publishes_the_adopted_bytes() {
    let before = compiler::core_product_speculations();
    let adopted = terminal_bytes("adopted");
    let after_adopted = compiler::core_product_speculations();
    assert!(
        after_adopted.0 > before.0 && after_adopted.1 == before.1,
        "an ordinary compile adopts its core speculation: {before:?} -> {after_adopted:?}"
    );

    compiler::refuse_core_product_speculation(true);
    let refused = terminal_bytes("refused");
    compiler::refuse_core_product_speculation(false);
    let after_refused = compiler::core_product_speculations();
    assert!(
        after_refused.1 > after_adopted.1 && after_refused.0 == after_adopted.0,
        "the seam forces the refused path: {after_adopted:?} -> {after_refused:?}"
    );

    assert!(
        adopted == refused,
        "the refused speculation changed the Terminal artifact ({} vs {} bytes)",
        adopted.len(),
        refused.len()
    );
    assert_eq!(
        adopted,
        terminal_bytes("adopted-again"),
        "repeat compiles differ"
    );
}

#[test]
fn source_compile_has_no_core_provider_defaults_activation() {
    let before = artifacts::compile_timings::counters::provider_defaults_execution_count();
    let bytes = terminal_bytes("no-core-defaults");
    assert!(!bytes.is_empty(), "the program publishes its Psi artifact");
    assert_eq!(
        artifacts::compile_timings::counters::provider_defaults_execution_count() - before,
        0,
        "canonical float realizations need no core provider-default Build activation"
    );
}
