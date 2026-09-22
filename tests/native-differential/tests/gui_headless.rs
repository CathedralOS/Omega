//! Headless GUI parity pin: `samples/gui/window_demo` runs interpreted to the
//! same exit 0 as the macOS-gated native build.
//! The virtual window system mints live handles, serves DCs/blits, reports
//! "no key / no event", and the sample's own 60-frame loop terminates -- no
//! real window opens, so this pin is platform-independent.
//!
//! The run needs ~40M interpreter steps (60 frames of software-rendered
//! pixels are genuine language-semantics work, not host ops), above the 10M
//! runaway default -- OMEGA_INTERP_STEP_BUDGET raises it for this process
//! (a dedicated test binary, so the env write races nothing).
//!
//! The sample declares its std dependency in `build.omg`, so it must be
//! compiled through the package route: the reconciled graph binds std to the
//! repository path, which is what `use omega_language_std::console` resolves
//! against -- the unmanaged route would instead look for an
//! `omega_language_std/` module below the sample root.

use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use compiler::CheckedCompileRequest;
use compiler::{CheckedCompilation, compile_to_checked};
use diagnostics::Diagnostic;
use fixture_package_inputs::{
    console_acceptance, fixture_package_identity, repo_root, standard_library_package_inputs,
};
use package_compilation::PackageCompilationInputs;
use std::path::Path;

// One harness owns the bundled standard library's location, the fixture
// package identities and the Console acceptance this target replays.
#[path = "common/fixture_package_inputs.rs"]
mod fixture_package_inputs;

/// Package inputs for `samples/gui/window_demo`: the root package bound to the
/// sample directory plus the ordinary std dependency its `build.omg` declares.
fn window_demo_package_inputs(root_path: &Path) -> PackageCompilationInputs {
    let project_root = root_path
        .parent()
        .expect("window_demo source has a project root");
    standard_library_package_inputs(project_root, 1, 2)
}

/// The package graph plus this harness's test acceptance of the exact std
/// Console plan the sample selects (`write_line` + `exit_process`; the
/// windowed app never reads stdin). The admitted row is derived from and
/// replayed against the preliminary checked graph; this is test-owned
/// acceptance, not evidence that an audit occurred.
fn reviewed_window_demo_inputs(
    root_path: &Path,
) -> Result<PackageCompilationInputs, Vec<Diagnostic>> {
    let package_inputs = window_demo_package_inputs(root_path);
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs.clone()),
        ..CheckedCompileRequest::new(root_path, None)
    })?;
    let binding = console_acceptance::candidate_console_exit_binding(
        &preliminary,
        fixture_package_identity(2),
        true,
        false,
    )?;
    package_inputs
        .with_accepted_semantic_bindings(vec![binding])
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit window_demo semantic binding: {errors:?}"
            ))]
        })
}

fn compile_window_demo(sample: &Path) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut request = CheckedCompileRequest::new(sample, None);
    request.package_inputs = Some(reviewed_window_demo_inputs(sample)?);
    compile_to_checked(request)
}

#[test]
fn window_demo_runs_headless_to_native_exit() {
    // SAFETY/order: this test file is its own binary and this is its only
    // test, so the process-global env write cannot race another test.
    unsafe { std::env::set_var("OMEGA_INTERP_STEP_BUDGET", "100000000") };

    let sample = repo_root().join("samples/gui/window_demo/main.omg");
    let checked = compile_window_demo(&sample).unwrap_or_else(|diagnostics| {
        panic!(
            "window_demo should compile for the interpreter:\n{}",
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(
        outcome.error, None,
        "the headless run must not decline: {:?}",
        outcome.error
    );
    assert_eq!(
        outcome.exit_code,
        0,
        "headless window_demo must exit 0 like the native run; stderr:\n{}",
        String::from_utf8_lossy(&outcome.stderr)
    );
}
