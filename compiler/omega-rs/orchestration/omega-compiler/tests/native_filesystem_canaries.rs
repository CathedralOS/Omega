// Native regression coverage for the filesystem canaries — which had NO automated
// test before (they were compiled + run by hand per fire). Each obtains the
// `FilesystemHost` boundary from the single canonical std module
// (`use omega::language::std::filesystem_host;`) rather than an inline trait; this
// pins that they still COMPILE to a native mach-o and RUN correctly on real macOS,
// across the range: no-Path (close), Path+stat, multi-op CRUD, dirent walk, locking,
// dir ops. (These canaries signal success via "PASS: …" on stdout and exit with the
// final write's byte count, so the assertion is on stdout, not the exit code.)
#![cfg(target_os = "macos")]
use omega_compiler::{CompileOptions, compile};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-compiler lives under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

fn compile_run(canary: &str) -> (Option<i32>, String) {
    let main_path = repo_root()
        .join("canaries/pass/filesystem")
        .join(canary)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-fscanary-{}-{}", canary, std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("{canary} should still compile via imported FilesystemHost:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned())
}

fn assert_pass(canary: &str) {
    // These canaries signal success by writing "PASS: …" to stdout; their exit code
    // is the final write's byte count (not 0), so success is the stdout, not the code.
    let (_code, stdout) = compile_run(canary);
    assert!(
        stdout.contains("PASS") && !stdout.contains("FAIL"),
        "{canary} expected PASS, got stdout: {stdout:?}"
    );
}

#[test]
fn native_close_still_compiles_and_runs() {
    // No Path domain, only close(fd); just needs to compile + run without crashing.
    let (code, _) = compile_run("native_close");
    assert!(code.is_some(), "native_close should run to a normal exit");
}

#[test]
fn native_stat_still_passes() { assert_pass("native_stat"); }
#[test]
fn native_crud_still_passes() { assert_pass("native_crud"); }
#[test]
fn native_dirs_still_passes() { assert_pass("native_dirs"); }
#[test]
fn native_read_dir_iter_still_passes() { assert_pass("native_read_dir_iter"); }
#[test]
fn native_flock_still_passes() { assert_pass("native_flock"); }
