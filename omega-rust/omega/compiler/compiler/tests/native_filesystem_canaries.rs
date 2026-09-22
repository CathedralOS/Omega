//! Fixtures shared by the native filesystem canaries: compilation, exact
//! macOS entry builds, project copies, staged package inputs and run
//! assertions.

#![cfg(target_os = "macos")]

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "fixture_rosters/native_filesystem_canaries.rs"]
mod fixture_roster;
#[path = "native_filesystem_canaries/gui_and_sample_apps.rs"]
mod gui_and_sample_apps;
#[path = "native_filesystem_canaries/native_filesystem_passes.rs"]
mod native_filesystem_passes;
#[path = "native_filesystem_canaries/samples_floats_and_objc.rs"]
mod samples_floats_and_objc;

// Native regression coverage for the filesystem canaries — which had NO automated
// test before (they were compiled + run by hand per fire). Each obtains the
// `FilesystemHost` boundary from the single canonical std module
// (`use omega::language::std::filesystem_host;`) rather than an inline trait; this
// pins that they still COMPILE to a native mach-o and RUN correctly on real macOS,
// across the range: no-Path (close), Path+stat, multi-op CRUD, dirent walk, locking,
// dir ops. (These canaries signal success via "PASS: …" on stdout and exit with the
// final write's byte count, so the assertion is on stdout, not the exit code.)
// The sample projects under `samples/` instead import `omega_language_std` as an
// ordinary package dependency; `staged_std_package_inputs` supplies that graph.
use compiler::{CheckedCompileRequest, CompileOptions, compile_to_checked};
use diagnostics::Diagnostic;
use fixture_package_inputs::{
    bundled_standard_library_root, console_acceptance, copied_fixture_package_inputs,
    fixture_package_identity, macos_entry_acceptance, repo_root,
};
use package_compilation::{AcceptedSemanticBindingRole, PackageCompilationInputs};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ENTRY_STAGE: AtomicU64 = AtomicU64::new(1);

fn compile_program(
    options: CompileOptions,
    package_inputs: Option<PackageCompilationInputs>,
) -> Result<compiler::CompileReport, Vec<Diagnostic>> {
    let build_dir = options.build_dir();
    let mut request = compiler::CompileRequest::new(options)
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
    if let Some(package_inputs) = package_inputs {
        // The permission policy is derived from the bindings this harness
        // accepted, exactly as the canary suite and sample oracle do.
        let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
            package_inputs
                .accepted_semantic_bindings()
                .flat_map(|binding| binding.terminal_authority_permissions())
                .cloned()
                .collect(),
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot construct staged project terminal-authority policy: {error:?}"
            ))]
        })?;
        request = request
            .with_terminal_authority_permission_policy(permission_policy)
            .with_package_inputs(package_inputs);
    }
    let report =
        compiler::compile(request).and_then(compiler::CompileOutcomes::into_single_report)?;
    report
        .publish_retained_native_artifact(&build_dir)
        .map_err(|error| vec![Diagnostic::error(error)])
}

fn compile_exact_macos_entry(
    options: CompileOptions,
) -> Result<compiler::CompileReport, Vec<Diagnostic>> {
    let ordinal = NEXT_ENTRY_STAGE.fetch_add(1, Ordering::Relaxed);
    let stage_dir = std::env::temp_dir().join(format!(
        "omega-macos-entry-stage-{}-{ordinal}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&stage_dir);
    std::fs::create_dir_all(&stage_dir).expect("create exact-entry source stage");

    let source_dir = options
        .root_path
        .parent()
        .expect("native source has a project directory");
    copy_project_tree(source_dir, &stage_dir, options.build_dir.as_deref())
        .expect("stage native source project");
    write_exact_macos_build(&stage_dir);

    let root_path = stage_dir.join(
        options
            .root_path
            .file_name()
            .expect("native source has a file name"),
    );
    let result = staged_std_package_inputs(source_dir, &root_path).and_then(|package_inputs| {
        compile_program(
            CompileOptions {
                root_path,
                build_dir: options.build_dir,
                target_name: Some("macos_arm64".to_owned()),
            },
            package_inputs,
        )
    });
    let _ = std::fs::remove_dir_all(&stage_dir);
    result
}

/// Package inputs for a staged project that authors the ordinary std
/// dependency in its `build.omg`; `None` for the self-contained corpus
/// fixtures, which import the bundled `omega::language::std` modules and
/// compile exactly as before.
///
/// The stage is a copy of the project under a temporary directory. Without a
/// package graph the compiler resolves `omega_language_std::console` as a
/// module below the project root, which the copy cannot satisfy. The sample's
/// authored `Source::Path` row is relative to its checked-in location and
/// dangles from the copy, so the graph is projected from the authored project
/// and its root package bound to the stage directory; the sample itself stays
/// unedited. Acceptance follows the canary suite and sample oracle: the exact
/// macOS entry schema, the std `FilesystemHost` service when imported, and
/// the std `Console` plan with termination, byte output and byte input, since
/// the samples reach output through std wrappers such as `write_line` rather
/// than spelling `write_byte` themselves. This is test-owned acceptance, not
/// a package-review receipt.
fn staged_std_package_inputs(
    authored_project: &Path,
    root_path: &Path,
) -> Result<Option<PackageCompilationInputs>, Vec<Diagnostic>> {
    let project_root = root_path
        .parent()
        .expect("staged source has a project root");
    let Some(package_inputs) = copied_fixture_package_inputs(authored_project, project_root) else {
        return Ok(None);
    };
    let standard_library = fixture_package_identity(2);
    if package_inputs.package_root(standard_library).is_none() {
        return Ok(Some(package_inputs));
    }
    let standard_library_root = bundled_standard_library_root();

    // Every staged build binds `macos_arm64::ProgramEntry`, so the checked
    // dependency entry is accepted before the application is selected.
    let mut bindings = vec![macos_entry_acceptance::candidate_macos_entry_binding(
        &standard_library_root,
        standard_library,
    )?];
    let package_inputs = package_inputs
        .with_accepted_semantic_bindings(bindings.clone())
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot accept staged project entry binding: {errors:?}"
            ))]
        })?;
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs.clone()),
        ..CheckedCompileRequest::new(root_path, Some("macos_arm64"))
    })?;
    let source = std::fs::read_to_string(root_path).unwrap_or_default();
    // `omega_language_std::filesystem` also matches the raw `filesystem_host`
    // import; both reach the same std service.
    if source.contains("omega_language_std::filesystem") {
        bindings.push(
            preliminary
                .candidate_service_binding(
                    AcceptedSemanticBindingRole::FilesystemHostService,
                    standard_library,
                    "FilesystemHost",
                )
                .map_err(|diagnostic| vec![diagnostic])?,
        );
    }
    if source.contains("omega_language_std::console") {
        bindings.push(console_acceptance::candidate_console_exit_binding(
            &preliminary,
            standard_library,
            true,
            true,
        )?);
    }
    package_inputs
        .with_accepted_semantic_bindings(bindings)
        .map(Some)
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot accept staged project std bindings: {errors:?}"
            ))]
        })
}

fn copy_project_tree(
    source: &Path,
    destination: &Path,
    excluded: Option<&Path>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if excluded.is_some_and(|excluded| path == excluded)
            || matches!(entry.file_name().to_str(), Some("build" | "target"))
        {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_project_tree(&path, &target, excluded)?;
        } else {
            std::fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn write_exact_macos_build(project: &Path) {
    let path = project.join("build.omg");
    let mut source = std::fs::read_to_string(&path).unwrap_or_default();
    const BUILD: &str = "machine build(builder: &mut Build) {";
    const BINDING: &str = "builder.roots.bind(macos_arm64::ProgramEntry, Main::main);";
    if !source.contains(BINDING) {
        if let Some(start) = source.find(BUILD) {
            source.insert_str(
                start + BUILD.len(),
                "\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);",
            );
        } else {
            source.push_str(
                "\n\nmachine build(builder: &mut Build) {\n    builder.application(\"native-filesystem-canary\");\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);\n}\n",
            );
        }
    }
    std::fs::write(path, source).expect("write exact macOS ProgramEntry build root");
}

fn compile_run(fixture: fixture_roster::Fixture) -> (Option<i32>, String) {
    let canary = fixture.name;
    let source_dir = repo_root().join("tests/omega/pass").join(fixture.path);
    let build_dir =
        std::env::temp_dir().join(format!("omega-fscanary-{}-{}", canary, std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: source_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
    })
    .unwrap_or_else(|d| {
        panic!("{canary} should still compile via imported FilesystemHost:\n{d:#?}")
    });
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn assert_pass(fixture: fixture_roster::Fixture) {
    // These canaries signal success by writing "PASS: …" to stdout; their exit code
    // is the final write's byte count (not 0), so success is the stdout, not the code.
    let canary = fixture.name;
    let (_code, stdout) = compile_run(fixture);
    assert!(
        stdout.contains("PASS") && !stdout.contains("FAIL"),
        "{canary} expected PASS, got stdout: {stdout:?}"
    );
}
