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
#[test]
fn native_at_ops_passes() { assert_pass("native_at_ops"); }
#[test]
fn native_at_runtime_name_passes() { assert_pass("native_at_runtime_name"); }

// --- Promoted coverage -------------------------------------------------------
// These canaries were built + run BY HAND in earlier fires and never wired into
// this harness (each carried a "NOT registered … yet" note). All 36 compile to a
// native mach-o and PASS on real macOS/aarch64 (audited 2026-07-06); wiring them
// in gives the whole native fs surface automated regression coverage, not just
// the original 8. Grouped by the Rust std::fs area each exercises.

// Core byte I/O + open modes
#[test]
fn native_append_passes() { assert_pass("native_append"); }
#[test]
fn native_open_rw_passes() { assert_pass("native_open_rw"); }
#[test]
fn native_open_create_passes() { assert_pass("native_open_create"); }
#[test]
fn native_seek_passes() { assert_pass("native_seek"); }
#[test]
fn native_positioned_io_passes() { assert_pass("native_positioned_io"); }
#[test]
fn native_errno_passes() { assert_pass("native_errno"); }
#[test]
fn native_fs_workflow_passes() { assert_pass("native_fs_workflow"); }

// Value-call literal forwarding (aliased-literal operand resolution, step 14 fix #1)
#[test]
fn native_value_call_literal_passes() { assert_pass("native_value_call_literal"); }
#[test]
fn native_value_call_path_passes() { assert_pass("native_value_call_path"); }
// `let`-bound host call forwarded through a same-machine value-call (step 14
// layers 2+3: LocalData collection + LocalStorage emission) — the ergonomic
// wrapper's shape, for a SAME-data-type callee.
#[test]
fn native_value_call_local_passes() { assert_pass("native_value_call_local"); }

// Copy / buffer marshalling
#[test]
fn native_buffer_copy_passes() { assert_pass("native_buffer_copy"); }
#[test]
fn native_subslice_copy_passes() { assert_pass("native_subslice_copy"); }
#[test]
fn native_copy_preserve_passes() { assert_pass("native_copy_preserve"); }
#[test]
fn native_forwarded_slice_literal_passes() { assert_pass("native_forwarded_slice_literal"); }

// Links, rename, truncation, permissions
#[test]
fn native_rename_passes() { assert_pass("native_rename"); }
#[test]
fn native_hard_link_passes() { assert_pass("native_hard_link"); }
#[test]
fn native_symlink_passes() { assert_pass("native_symlink"); }
#[test]
fn native_set_len_passes() { assert_pass("native_set_len"); }
#[test]
fn native_permissions_passes() { assert_pass("native_permissions"); }
#[test]
fn native_fchmod_passes() { assert_pass("native_fchmod"); }
// Ownership: expects a NON-root user (a real chown to root -> EPERM). Would fail
// only if the suite were ever run as root, which the dev/CI macOS box is not.
#[test]
fn native_chown_passes() { assert_pass("native_chown"); }

// Existence / classification / path resolution
#[test]
fn native_exists_passes() { assert_pass("native_exists"); }
#[test]
fn native_try_exists_passes() { assert_pass("native_try_exists"); }
#[test]
fn native_filetype_passes() { assert_pass("native_filetype"); }
#[test]
fn native_canonicalize_passes() { assert_pass("native_canonicalize"); }
#[test]
fn native_try_clone_passes() { assert_pass("native_try_clone"); }
#[test]
fn native_read_dir_passes() { assert_pass("native_read_dir"); }

// Durability
#[test]
fn native_sync_passes() { assert_pass("native_sync"); }
#[test]
fn native_sync_data_passes() { assert_pass("native_sync_data"); }
#[test]
fn native_set_times_passes() { assert_pass("native_set_times"); }

// Metadata decode (struct stat byte-assembly)
#[test]
fn native_fstat_passes() { assert_pass("native_fstat"); }
#[test]
fn native_symlink_metadata_passes() { assert_pass("native_symlink_metadata"); }
#[test]
fn native_metadata_nlink_passes() { assert_pass("native_metadata_nlink"); }
#[test]
fn native_metadata_ino_passes() { assert_pass("native_metadata_ino"); }
#[test]
fn native_metadata_ctime_dev_passes() { assert_pass("native_metadata_ctime_dev"); }
#[test]
fn native_metadata_blocks_passes() { assert_pass("native_metadata_blocks"); }
#[test]
fn native_metadata_modified_passes() { assert_pass("native_metadata_modified"); }
#[test]
fn native_metadata_times_passes() { assert_pass("native_metadata_times"); }
#[test]
fn native_metadata_readonly_passes() { assert_pass("native_metadata_readonly"); }

#[test]
fn native_value_call_let_chain_passes() { assert_pass("native_value_call_let_chain"); }
// The SHIPPED ergonomic Filesystem wrapper natively (step 14 COMPLETE, all 5 layers)
#[test]
fn native_wrapper_write_all_passes() { assert_pass("native_wrapper_write_all"); }
// STAT wrapper `Filesystem::exists` natively via TERMINAL-VALUE COMPLETION —
// the no-transition workaround for the value-call guard-ordering bug.
#[test]
fn native_wrapper_exists_passes() { assert_pass("native_wrapper_exists"); }
