// Native run-tests for the runtime-END subslice + domained-slice `.len` guard
// unlock (TASKS_FS.md step: native `create_dir_all` path scan). Two backend
// fixes are exercised end to end:
//
//   1. omega-state-guards `is_slice_descriptor` peels `Constrained` off a
//      reference referee, so a `&[u8] in Path` (= `Reference { Constrained { Slice } }`)
//      param's `.len` resolves as a slice-descriptor length in a guard operand.
//      Without it, `self.k < path.len` was refused (silently-dropped-guard).
//
//   2. omega-instruction-selection `resolve_subslice_bound` accepts a Machine
//      -region (machine FIELD) bound as the END of a runtime subslice, so
//      `path[0..self.k]` materializes its `{ptr, len}` descriptor. The START
//      stays frame-only (its indexed-address instruction has a frame-only index).
//
// These are held in a dedicated file (not the shared, hot `canary_suite.rs`).
use omega_compiler::CompileOptions;
use std::path::{Path, PathBuf};
use std::process::Command;

fn compile(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    omega_compiler::compile(omega_compiler::CompileRequest::new(options))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect(
            "omega-compiler lives under source/omega-rust/omega/orchestration/omega-compiler",
        )
        .to_path_buf()
}

fn compile_and_run(canary_rel: &str, tag: &str) -> std::process::Output {
    let profile = omega_target::TargetProfile::host();
    let canary = repo_root().join("tests/canaries/pass").join(canary_rel);
    let build_dir = std::env::temp_dir().join(format!("omega-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some(profile.target_name().to_owned()),
        write_output: true,
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
        "slices/domained_slice_len_guard_exit",
        "domained-len-guard",
        70,
    );
}

#[test]
fn runtime_end_subslice_machine_field_bound_materializes() {
    // Fix #2 alone: `sub[0..self.k]` with a machine-field END, plain slice.
    assert_exit(
        "slices/runtime_end_subslice_machine_field_exit",
        "rt-end-subslice",
        70,
    );
}

#[test]
fn domained_runtime_end_subslice_materializes() {
    // Both fixes: the create_dir_all path-scan shape (domained param + machine
    // -field-end subslice + subslice-domain grant), minus the fs call.
    assert_exit(
        "slices/domained_runtime_end_subslice_exit",
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
