//! Source compilation publishes its Psi artifact without executing a separate
//! core supplier Build. Fixture review happens before the measured compile so
//! its own independent activations do not obscure this request's count.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};

// A package with a selected program entry, so it reaches a Terminal artifact.
const FIXTURE: &str =
    "tests/fixtures/pass/providers/checked_boundary_requirement_dispatch_exit/main.omg";

fn terminal_product(run: &str) -> (Vec<u8>, u64, u64) {
    let target_name = "linux_x86_64";
    let main = repo_root().join(FIXTURE);
    let prepared = reviewed_repository_fixture_package_inputs(&main, Some(target_name))
        .unwrap_or_else(|diagnostics| panic!("{FIXTURE} package inputs: {diagnostics:#?}"));
    let build_dir = std::env::temp_dir().join(format!(
        "omega-core-build-activation-{}-{run}",
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
    let builds_before = artifacts::compile_timings::counters::build_execution_count();
    let suppliers_before =
        artifacts::compile_timings::counters::provider_defaults_execution_count();
    let bytes =
        orchestration::compile_sources(&mut compiler::CompilationSession::default(), request)
            .and_then(compiler::CompileOutcomes::into_single_report)
            .unwrap_or_else(|diagnostics| panic!("{FIXTURE} {run}: {diagnostics:#?}"))
            .into_retained_terminal_artifact()
            .unwrap_or_else(|| panic!("{FIXTURE} {run} retains its Psi artifact"))
            .artifact()
            .to_bytes();
    (
        bytes,
        artifacts::compile_timings::counters::build_execution_count() - builds_before,
        artifacts::compile_timings::counters::provider_defaults_execution_count()
            - suppliers_before,
    )
}

#[test]
fn source_compile_has_no_core_provider_defaults_activation() {
    let (bytes, builds, suppliers) = terminal_product("no-core-defaults");
    assert!(!bytes.is_empty(), "the program publishes its Psi artifact");
    assert_eq!(
        suppliers, 0,
        "canonical float realizations need no core provider-default Build activation"
    );
    assert_eq!(builds, 1, "the compile executes only its root Build");
}
