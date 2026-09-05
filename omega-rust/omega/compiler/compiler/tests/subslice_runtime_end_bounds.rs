// Native run-tests for the runtime-END subslice + domained-slice `.len` guard
// unlock (TASKS_FS.md step: native `create_dir_all` path scan). Two backend
// facts are exercised end to end through the canonical Terminal-Psi route:
//
//   1. a `&[u8] in Path` parameter exposes its slice-descriptor length;
//   2. a machine-field END bound materializes the correct runtime subslice.
//
// These are held in a dedicated file (not the shared, hot `canary_suite.rs`).
use compiler::CompileOptions;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "fixture_rosters/subslice_runtime_end_bounds.rs"]
mod fixture_roster;

fn compile(
    options: CompileOptions,
) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
    let build_dir = options.build_dir();
    let report = compiler::compile(
        compiler::CompileRequest::new(options)
            .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact),
    )?;
    report
        .publish_retained_native_artifact(&build_dir)
        .map_err(|error| vec![diagnostics::Diagnostic::error(error)])
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler lives under omega-rust/omega/compiler/compiler")
        .to_path_buf()
}

fn compile_and_run(canary_rel: &str, tag: &str) -> std::process::Output {
    let profile = target::TargetProfile::host();
    let canary = repo_root().join("tests/omega/pass").join(canary_rel);
    let build_dir = std::env::temp_dir().join(format!("omega-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some(profile.target_name().to_owned()),
    })
    .unwrap_or_else(|diagnostics| panic!("{canary_rel} should compile:\n{diagnostics:#?}"));

    let executable = if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    };
    let output = Command::new(build_dir.join(executable))
        .output()
        .expect("canary should run");
    let _ = std::fs::remove_dir_all(&build_dir);
    output
}

fn assert_exit(canary_rel: &str, tag: &str, expected: i32) {
    let output = compile_and_run(canary_rel, tag);
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{canary_rel} expected exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn domained_slice_len_guard_lowers() {
    // Fix #1 alone: `self.k < path.len` over a `&[u8] in Path` param.
    assert_exit(
        fixture_roster::DOMAINED_SLICE_LEN_GUARD_EXIT,
        "domained-len-guard",
        70,
    );
}

#[test]
fn runtime_end_subslice_machine_field_bound_materializes() {
    // Fix #2 alone: `sub[0..self.k]` with a machine-field END, plain slice.
    assert_exit(
        fixture_roster::RUNTIME_END_SUBSLICE_MACHINE_FIELD_EXIT,
        "rt-end-subslice",
        70,
    );
}

#[test]
fn domained_runtime_end_subslice_materializes() {
    // Both fixes: the create_dir_all path-scan shape (domained param + machine
    // -field-end subslice + subslice-domain grant), minus the fs call.
    assert_exit(
        fixture_roster::DOMAINED_RUNTIME_END_SUBSLICE_EXIT,
        "domained-rt-end-subslice",
        70,
    );
}

// NOTE: an end-to-end native `mkdir path[0..k]` canary is deferred by ONE more
// native-seam gap: darwin `create_dir` binds to the C symbol `_mkdir(const char*,
// mode)`, which reads a NUL-terminated string. The subslice descriptor is correct
// (proven by the exit-70 slice tests above), but the seam passes the slice ptr
// directly, so `_mkdir` reads past the subslice len to the source literal's
// trailing `\0`. A non-NUL-terminated path arg to a C char* fs symbol must first
// be copied into a NUL-terminated scratch buffer. Tracked in TASKS_FS.md.
