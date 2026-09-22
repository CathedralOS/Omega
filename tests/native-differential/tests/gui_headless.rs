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

use build_declarations::{BuildDeclaration, extract_build_declaration};
use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use compiler::CheckedCompileRequest;
use compiler::{CheckedCompilation, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};

// The compiler test target owns the exact Console provider acceptance this
// harness must replay; including it keeps both targets on one derivation.
#[path = "../../../omega-rust/omega/compiler/compiler/tests/support/console_acceptance.rs"]
mod console_acceptance;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("native differential tests live under tests/native-differential")
        .to_path_buf()
}

fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("window_demo fixture package identity is nonzero")
}

/// Package inputs for `samples/gui/window_demo`: the root package bound to the
/// sample directory plus the ordinary std dependency its `build.omg` declares.
fn window_demo_package_inputs(root_path: &Path) -> PackageCompilationInputs {
    let project_root = root_path
        .parent()
        .expect("window_demo source has a project root");
    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("window_demo {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => {
            panic!(
                "window_demo {} cannot be a workspace root",
                project_root.display()
            )
        }
    };
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    let packages = vec![
        PackageSourceBinding::new(
            root_identity,
            root_name.into_string(),
            project_root.to_path_buf(),
        ),
        PackageSourceBinding::new(
            standard_library_identity,
            "omega-language-std",
            repo_root().join("source/library/std"),
        ),
    ];
    let dependencies = vec![PackageDependencyBinding::new(
        root_identity,
        "omega_language_std",
        standard_library_identity,
    )];

    PackageCompilationInputs::new(root_identity, root_role, packages, dependencies)
        .unwrap_or_else(|errors| panic!("window_demo {}: {errors:#?}", project_root.display()))
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
