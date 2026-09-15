//! Fixtures shared by the native filesystem canaries: compilation, exact
//! macOS entry builds, project copies and run assertions.

#![cfg(target_os = "macos")]

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
use compiler::CompileOptions;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ENTRY_STAGE: AtomicU64 = AtomicU64::new(1);

fn compile_program(
    options: CompileOptions,
) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
    let build_dir = options.build_dir();
    let report = compiler::compile(
        compiler::CompileRequest::new(options)
            .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)?;
    report
        .publish_retained_native_artifact(&build_dir)
        .map_err(|error| vec![diagnostics::Diagnostic::error(error)])
}

fn compile_exact_macos_entry(
    options: CompileOptions,
) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
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

    let result = compile_program(CompileOptions {
        root_path: stage_dir.join(
            options
                .root_path
                .file_name()
                .expect("native source has a file name"),
        ),
        build_dir: options.build_dir,
        target_name: Some("macos_arm64".to_owned()),
    });
    let _ = std::fs::remove_dir_all(&stage_dir);
    result
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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler lives under omega-rust/omega/compiler/compiler")
        .to_path_buf()
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
