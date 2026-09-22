//! Executed kernel canary: the theorem-certificate fixture interprets,
//! natively runs, and cross-compiles with its discharged call-site
//! requirement obligations riding the emitted call rows.
//!
//! `pass/proofs/kernel_theorem_equality_certificates` cites two theorem
//! machines (`equal_symmetric`, `equal_transitive`) whose `requires`
//! clauses allocate call-site `requirement_obligations` that the shipped
//! proof bundle discharges and the mathematical core re-decides. The
//! calls carry those obligation identities through the abstract plan into
//! the retained native call rows as proof metadata — this file pins the
//! end-to-end executed claim: the checked interpreter exits 70, the
//! emitted host executable exits 70, and the cross targets still compile.
//! The false twin stays refused before any kernel judgment.

use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;
use compiler::CheckedCompileRequest;
use compiler::{CompileOptions, compile, compile_to_checked};
use diagnostics::Diagnostic;
use fixture_package_inputs::{
    bundled_standard_library_root, candidate_program_entry_binding, console_acceptance,
    fixture_package_identity, repo_root, repository_fixture_package_inputs,
};
use package_compilation::PackageCompilationInputs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

const THEOREM_EQUALITY_CANARY: &str = "proofs/kernel_theorem_equality_certificates";
const THEOREM_EQUALITY_FALSE_TWIN: &str = "fail/proofs/kernel_theorem_equality_false_twin";

/// The fixture's authored `omega_language_std` package closure with the exact
/// entry schema for `target_name` accepted, then the one Console
/// `exit_process` provider the source reaches. Test acceptance only; it is
/// not evidence that a production audit occurred.
fn kernel_canary_package_inputs(
    project_root: &Path,
    target_name: &str,
) -> Result<PackageCompilationInputs, Vec<Diagnostic>> {
    let package_inputs = repository_fixture_package_inputs(&project_root.join("main.omg"))
        .expect("a kernel canary authors its std dependency");
    let standard_library_identity = fixture_package_identity(2);
    let entry = candidate_program_entry_binding(
        target_name,
        &bundled_standard_library_root(),
        standard_library_identity,
    )?
    .unwrap_or_else(|| panic!("kernel canary has no entry binding for {target_name}"));

    // The accepted entry binding authorizes checking the entry schema, so it
    // rides the preliminary build that resolves the exact Console provider
    // plan the fixture's `exit_process` reach accepts.
    let package_inputs = package_inputs
        .with_accepted_semantic_bindings(vec![entry])
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit kernel canary entry binding: {errors:?}"
            ))]
        })?;
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs.clone()),
        ..CheckedCompileRequest::new(&project_root.join("main.omg"), Some(target_name))
    })?;
    let console = console_acceptance::candidate_console_exit_binding(
        &preliminary,
        standard_library_identity,
        false,
        false,
    )?;
    let mut bindings: Vec<_> = package_inputs
        .accepted_semantic_bindings()
        .cloned()
        .collect();
    bindings.push(console);
    package_inputs
        .with_accepted_semantic_bindings(bindings)
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit kernel canary semantic bindings: {errors:?}"
            ))]
        })
}

fn checked_compile(
    project_root: &Path,
    target_name: &str,
) -> Result<compiler::CheckedCompilation, Vec<Diagnostic>> {
    let package_inputs = kernel_canary_package_inputs(project_root, target_name)?;
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&project_root.join("main.omg"), Some(target_name))
    })
}

fn compile_native(project_root: &Path, target_name: &str, build_dir: &Path) {
    let package_inputs =
        kernel_canary_package_inputs(project_root, target_name).unwrap_or_else(|diagnostics| {
            panic!(
                "{target_name} package inputs for {} should resolve:\n{diagnostics:#?}",
                project_root.display()
            )
        });
    let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
        package_inputs
            .accepted_semantic_bindings()
            .flat_map(|binding| binding.terminal_authority_permissions().iter().cloned())
            .collect(),
    )
    .unwrap_or_else(|error| {
        panic!("cannot construct kernel canary terminal-authority policy: {error:?}")
    });
    let report = compile(
        compiler::CompileRequest::new(CompileOptions {
            root_path: project_root.join("main.omg"),
            build_dir: Some(build_dir.to_path_buf()),
            target_name: Some(target_name.to_owned()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact)
        .with_terminal_authority_permission_policy(permission_policy)
        .with_package_inputs(package_inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!(
            "{target_name} compile of {} should succeed:\n{diagnostics:#?}",
            project_root.display()
        )
    });
    report
        .publish_retained_native_artifact(build_dir)
        .unwrap_or_else(|error| {
            panic!(
                "{target_name} artifact publication of {} should succeed: {error}",
                project_root.display()
            )
        });
}

struct TemporaryBuildDirectory(PathBuf);

impl TemporaryBuildDirectory {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryBuildDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unique_build_dir(tag: &str) -> TemporaryBuildDirectory {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should follow the Unix epoch")
        .as_nanos();
    TemporaryBuildDirectory(
        std::env::temp_dir().join(format!("omega-{tag}-{}-{nonce}", std::process::id())),
    )
}

#[test]
fn theorem_equality_certificates_interpret_run_natively_and_cross_compile() {
    let canary = repo_root()
        .join("tests/omega/pass")
        .join(THEOREM_EQUALITY_CANARY);
    let host = target::TargetProfile::host();

    let checked = checked_compile(&canary, host.target_name())
        .expect("theorem certificate canary should reach checked trees");
    let interpreted = interpret_entry(
        &checked,
        BuildMachineEntry::Name(
            checked
                .selected_program_entry_machine()
                .expect("theorem certificate canary selects an exact ProgramEntry"),
        ),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "interpreter must exit through both discharged theorem citations: {interpreted:?}"
    );

    let host_build = unique_build_dir("kernel-theorem-host");
    compile_native(&canary, host.target_name(), host_build.path());
    let executable = if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    };
    let output = Command::new(host_build.path().join(executable))
        .output()
        .expect("theorem certificate executable should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native theorem certificate canary failed; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let host_build_path = host_build.path().to_path_buf();
    drop(host_build);
    assert!(
        !host_build_path.exists(),
        "host build directory should be removed after execution"
    );

    // The kernel leg's native claim is the host run; cross-compile where the
    // std Console exit provider has a catalogued native intrinsic (Windows and
    // macOS console providers are not catalogued for native emission yet).
    let cross_target = "linux_arm64";
    let cross_build = unique_build_dir(&format!("kernel-theorem-{cross_target}"));
    compile_native(&canary, cross_target, cross_build.path());
    let cross_build_path = cross_build.path().to_path_buf();
    drop(cross_build);
    assert!(
        !cross_build_path.exists(),
        "{cross_target} build directory should be removed after compilation"
    );
}

#[test]
fn theorem_equality_false_twin_rejects_before_kernel_judgment() {
    let canary = repo_root()
        .join("tests/omega")
        .join(THEOREM_EQUALITY_FALSE_TWIN);
    let host = target::TargetProfile::host();
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some(host.target_name()),
    ))
    .expect_err("the false twin's ensures is not entailed by its premise");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(
            "cannot prove ensures contract proof fact `b == c` from the requires contract"
        ),
        "false twin should reject with the unprovable-ensures diagnostic, got:\n{rendered}"
    );
}
