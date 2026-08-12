// Native regression coverage for the filesystem canaries — which had NO automated
// test before (they were compiled + run by hand per fire). Each obtains the
// `FilesystemHost` boundary from the single canonical std module
// (`use omega::language::std::filesystem_host;`) rather than an inline trait; this
// pins that they still COMPILE to a native mach-o and RUN correctly on real macOS,
// across the range: no-Path (close), Path+stat, multi-op CRUD, dirent walk, locking,
// dir ops. (These canaries signal success via "PASS: …" on stdout and exit with the
// final write's byte count, so the assertion is on stdout, not the exit code.)
#![cfg(target_os = "macos")]
use omega_compiler::{CompileOptions, compile as compile_program};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ENTRY_STAGE: AtomicU64 = AtomicU64::new(1);

fn compile_exact_macos_entry(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
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
        write_output: options.write_output,
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
    if !source.contains("target macos_arm64") {
        source.push_str("\n\ntarget macos_arm64 {\n}\n");
    }
    const BUILD: &str = "machine build(b: &mut Build) {";
    const BINDING: &str = "b.roots.bind(macos_arm64::ProgramEntry, Main::main);";
    if !source.contains(BINDING) {
        if let Some(start) = source.find(BUILD) {
            source.insert_str(
                start + BUILD.len(),
                "\n    b.roots.bind(macos_arm64::ProgramEntry, Main::main);",
            );
        } else {
            source.push_str(
                "\n\nmachine build(b: &mut Build) {\n    b.roots.bind(macos_arm64::ProgramEntry, Main::main);\n}\n",
            );
        }
    }
    std::fs::write(path, source).expect("write exact macOS ProgramEntry build root");
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-compiler lives under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

fn compile_run(canary: &str) -> (Option<i32>, String) {
    let source_dir = repo_root().join("canaries/pass/filesystem").join(canary);
    let build_dir =
        std::env::temp_dir().join(format!("omega-fscanary-{}-{}", canary, std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: source_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
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
fn native_stat_still_passes() {
    assert_pass("native_stat");
}
#[test]
fn native_crud_still_passes() {
    assert_pass("native_crud");
}
#[test]
fn native_dirs_still_passes() {
    assert_pass("native_dirs");
}
#[test]
fn native_read_dir_iter_still_passes() {
    assert_pass("native_read_dir_iter");
}
#[test]
fn native_flock_still_passes() {
    assert_pass("native_flock");
}
// The WRAPPER lock family (lock/lock_shared/try_lock/try_lock_shared/unlock,
// Rust File::lock parity) + metadata(File) (the fstat wrapper) -- first
// runtime coverage (both were zero-caller). flock has no msvcrt equivalent,
// so this lives here (macOS-gated) like native_flock, not in the differential
// RUN_CANARIES. Exit-coded (70 = full single-fd lock cycle + meta.len check);
// the interpreter leg was probe-verified at 70 when this landed.
#[test]
fn wrapper_lock_metadata_exit_runs() {
    let (code, _) = compile_run("wrapper_lock_metadata_exit");
    assert_eq!(code, Some(70), "wrapper lock/metadata cycle should exit 70");
}
// Closes the wrapper zero-caller sweep: set_times (futimens, read back via
// metadata(File)), the set_owner family (uid/gid -1 = unprivileged no-op),
// and symlink_metadata (lstat on a regular file). chown/futimens/lstat have
// no msvcrt rows, so macOS-gated here; interp leg probe-verified at 70.
#[test]
fn wrapper_times_owner_lstat_exit_runs() {
    let (code, _) = compile_run("wrapper_times_owner_lstat_exit");
    assert_eq!(code, Some(70), "wrapper times/owner/lstat should exit 70");
}
// The dirfd REWIND: a second first-entry read on the same fd must return the
// entry (native getdirentries advances the FD OFFSET; read_dir_entry_fd now
// lseeks to 0 first). The iterative remove_dir_all drain depends on this.
#[test]
fn dirfd_reread_exit_runs() {
    let (code, _) = compile_run("dirfd_reread_exit");
    assert_eq!(code, Some(70), "dirfd re-read should exit 70");
}
// The dir-walk wrapper family END TO END (create_dir_all -> read_dir_count /
// is_empty -> remove_dir_all): the capstone of the call-with-return arc.
#[test]
fn dir_walk_wrappers_exit_runs() {
    let (code, _) = compile_run("dir_walk_wrappers_exit");
    assert_eq!(code, Some(70), "native dir-walk family should exit 70");
}

// A directory whose packed dirents exceed the std wrapper's 512-byte buffer
// must be drained through repeated native getdirentries64 calls. This seeds the
// directory from Rust so the Omega probe can exercise count, stats, and indexed
// entry lookup beyond the first fill.
#[test]
fn posix_directory_wrappers_drain_multiple_native_fills() {
    let base = std::env::temp_dir().join(format!(
        "omega-native-readdir-multifill-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let assets = base.join("assets");
    std::fs::create_dir_all(&assets).expect("create multifill assets");
    for index in 0..48 {
        std::fs::write(
            assets.join(format!("entry_{index:02}_with_a_long_record_name.dat")),
            b"x",
        )
        .expect("seed multifill entry");
    }

    let main_path = base.join("main.omg");
    let source = format!(
        r#"use omega::language::std::filesystem;
use omega::language::std::console;

data Main {{
    fs: Filesystem;
    console: Console;
    result: IoResult;
    stats_result: DirStatsResult;
    entry_result: DirEntryResult;
    open_result: OpenResult;
    file_fd: i32;
    close_rc: i32;
}}

machine Main::main(&mut self) {{
    self.result = self.fs.read_dir_count("{}");
    transition self.result {{ IoResult::Ok {{ count }} -> check(count) _ -> fail() }}
    state check(&mut self, count: u64) {{ transition count == 48 {{ true -> stats() _ -> fail() }} }}
    state stats(&mut self) {{
        self.stats_result = self.fs.read_dir_stats("{}");
        transition self.stats_result {{ DirStatsResult::Ok {{ stats }} -> check_stats(stats) _ -> fail() }}
    }}
    state check_stats(&mut self, stats: DirStats) {{
        transition stats.entries == 48 && stats.subdirs == 0 && stats.files == 48 {{ true -> nth() _ -> fail() }}
    }}
    state nth(&mut self) {{
        self.entry_result = self.fs.read_dir_nth("{}", 47);
        transition self.entry_result {{ DirEntryResult::Ok {{ entry }} -> check_entry(entry) _ -> fail() }}
    }}
    state check_entry(&mut self, entry: DirEntry) {{
        transition entry.is_file && entry.name_len > 0 {{ true -> end() _ -> fail() }}
    }}
    state end(&mut self) {{
        self.entry_result = self.fs.read_dir_nth("{}", 48);
        transition self.entry_result {{ DirEntryResult::End -> fd_open() _ -> fail() }}
    }}
    state fd_open(&mut self) {{
        self.open_result = self.fs.open("{}");
        transition self.open_result {{ OpenResult::Ok {{ file }} -> fd_lookup(file) _ -> fail() }}
    }}
    state fd_lookup(&mut self, file: File) {{
        self.file_fd = file.fd;
        self.entry_result = self.fs.read_dir_entry_fd(self.file_fd, 47);
        transition self.entry_result {{ DirEntryResult::Ok {{ entry }} -> fd_check(entry) _ -> fail() }}
    }}
    state fd_check(&mut self, entry: DirEntry) {{
        transition entry.is_file && entry.name_len > 0 {{ true -> fd_close() _ -> fail() }}
    }}
    state fd_close(&mut self) {{
        self.close_rc = self.fs.close(File {{ fd: self.file_fd }});
        transition self.close_rc == 0 {{ true -> pass() _ -> fail() }}
    }}
    state pass(&mut self) {{ self.console.exit_process(70); }}
    state fail(&mut self) {{ self.console.exit_process(71); }}
}}
"#,
        assets.display(),
        assets.display(),
        assets.display(),
        assets.display(),
        assets.display()
    );
    std::fs::write(&main_path, source).expect("write multifill probe");

    let build_dir = base.join("build");
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("multifill POSIX wrappers should compile:\n{d:#?}"));
    let output = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run multifill probe");
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(
        output.status.code(),
        Some(70),
        "count, stats, and indexed lookup must drain every native buffer fill"
    );
}
#[test]
fn native_at_ops_passes() {
    assert_pass("native_at_ops");
}
#[test]
fn native_at_runtime_name_passes() {
    assert_pass("native_at_runtime_name");
}

// --- Promoted coverage -------------------------------------------------------
// These canaries were built + run BY HAND in earlier fires and never wired into
// this harness (each carried a "NOT registered … yet" note). All 36 compile to a
// native mach-o and PASS on real macOS/aarch64 (audited 2026-07-06); wiring them
// in gives the whole native fs surface automated regression coverage, not just
// the original 8. Grouped by the Rust std::fs area each exercises.

// Core byte I/O + open modes
#[test]
fn native_append_passes() {
    assert_pass("native_append");
}
#[test]
fn native_open_rw_passes() {
    assert_pass("native_open_rw");
}
#[test]
fn native_open_create_passes() {
    assert_pass("native_open_create");
}
#[test]
fn native_seek_passes() {
    assert_pass("native_seek");
}
#[test]
fn native_positioned_io_passes() {
    assert_pass("native_positioned_io");
}
#[test]
fn native_errno_passes() {
    assert_pass("native_errno");
}
#[test]
fn native_fs_workflow_passes() {
    assert_pass("native_fs_workflow");
}

// Value-call literal forwarding (aliased-literal operand resolution, step 14 fix #1)
#[test]
fn native_value_call_literal_passes() {
    assert_pass("native_value_call_literal");
}
#[test]
fn native_value_call_path_passes() {
    assert_pass("native_value_call_path");
}
// `let`-bound host call forwarded through a same-machine value-call (step 14
// layers 2+3: LocalData collection + LocalStorage emission) — the ergonomic
// wrapper's shape, for a SAME-data-type callee.
#[test]
fn native_value_call_local_passes() {
    assert_pass("native_value_call_local");
}

// Copy / buffer marshalling
#[test]
fn native_buffer_copy_passes() {
    assert_pass("native_buffer_copy");
}
#[test]
fn native_subslice_copy_passes() {
    assert_pass("native_subslice_copy");
}
#[test]
fn native_copy_preserve_passes() {
    assert_pass("native_copy_preserve");
}
#[test]
fn native_forwarded_slice_literal_passes() {
    assert_pass("native_forwarded_slice_literal");
}

// Links, rename, truncation, permissions
#[test]
fn native_rename_passes() {
    assert_pass("native_rename");
}
#[test]
fn native_hard_link_passes() {
    assert_pass("native_hard_link");
}
#[test]
fn native_symlink_passes() {
    assert_pass("native_symlink");
}
#[test]
fn native_set_len_passes() {
    assert_pass("native_set_len");
}
#[test]
fn native_permissions_passes() {
    assert_pass("native_permissions");
}
#[test]
fn native_fchmod_passes() {
    assert_pass("native_fchmod");
}
// Ownership: expects a NON-root user (a real chown to root -> EPERM). Would fail
// only if the suite were ever run as root, which the dev/CI macOS box is not.
#[test]
fn native_chown_passes() {
    assert_pass("native_chown");
}

// Existence / classification / path resolution
#[test]
fn native_exists_passes() {
    assert_pass("native_exists");
}
#[test]
fn native_try_exists_passes() {
    assert_pass("native_try_exists");
}
#[test]
fn native_filetype_passes() {
    assert_pass("native_filetype");
}
#[test]
fn native_canonicalize_passes() {
    assert_pass("native_canonicalize");
}
#[test]
fn native_try_clone_passes() {
    assert_pass("native_try_clone");
}
#[test]
fn native_read_dir_passes() {
    assert_pass("native_read_dir");
}

// Durability
#[test]
fn native_sync_passes() {
    assert_pass("native_sync");
}
#[test]
fn native_sync_data_passes() {
    assert_pass("native_sync_data");
}
#[test]
fn native_set_times_passes() {
    assert_pass("native_set_times");
}

// Metadata decode (struct stat byte-assembly)
#[test]
fn native_fstat_passes() {
    assert_pass("native_fstat");
}
#[test]
fn native_symlink_metadata_passes() {
    assert_pass("native_symlink_metadata");
}
#[test]
fn native_metadata_nlink_passes() {
    assert_pass("native_metadata_nlink");
}
#[test]
fn native_metadata_ino_passes() {
    assert_pass("native_metadata_ino");
}
#[test]
fn native_metadata_ctime_dev_passes() {
    assert_pass("native_metadata_ctime_dev");
}
#[test]
fn native_metadata_blocks_passes() {
    assert_pass("native_metadata_blocks");
}
#[test]
fn native_metadata_modified_passes() {
    assert_pass("native_metadata_modified");
}
#[test]
fn native_metadata_times_passes() {
    assert_pass("native_metadata_times");
}
#[test]
fn native_metadata_readonly_passes() {
    assert_pass("native_metadata_readonly");
}

#[test]
fn native_value_call_let_chain_passes() {
    assert_pass("native_value_call_let_chain");
}
// The SHIPPED ergonomic Filesystem wrapper natively (step 14 COMPLETE, all 5 layers)
#[test]
fn native_wrapper_write_all_passes() {
    assert_pass("native_wrapper_write_all");
}
// STAT wrapper `Filesystem::exists` natively via TERMINAL-VALUE COMPLETION —
// the no-transition workaround for the value-call guard-ordering bug.
#[test]
fn native_wrapper_exists_passes() {
    assert_pass("native_wrapper_exists");
}
// The value-call transition-guard DEEP BUG, now FIXED (both halves): a callee that
// branches on a host-call result in an internal transition, whose bool result is
// assigned to a field. Guards the ordering fix + the field-mutation constant-fold fix.
#[test]
fn native_value_call_guard_passes() {
    assert_pass("native_value_call_guard");
}
// ENUM-transition-leaf delivery: a value-call whose callee transitions to enum
// leaves (Err / Ok{pair}) delivers the correct arm's tag+payload to a field. Guards
// the nullary-enum-variant frame-slot tag write (mutation/frame_slots.rs).
#[test]
fn native_enum_result_passes() {
    assert_pass("native_enum_result");
}
// The PAYLOAD-CARRYING ergonomic wrapper result natively (unblocked by the deep
// fix): `Filesystem::write_all -> UnitResult` reports Error for a bad path and Ok
// for a good one — the RESULT, not just the side effect, is now correct.
#[test]
fn native_wrapper_write_all_result_passes() {
    assert_pass("native_wrapper_write_all_result");
}
// `Filesystem::try_exists -> ExistsResult` Yes/No natively: the faithful 3-way now
// captures errno into a field in the entry (before branching) so the No-vs-Error
// split guards on a stored field, not a nested host-call-in-guard the deep fix
// doesn't reach at that nesting.
#[test]
fn native_wrapper_try_exists_passes() {
    assert_pass("native_wrapper_try_exists");
}
// `Filesystem::metadata_path -> MetadataResult::Ok { meta }` with the PAYLOAD
// destructured and USED (`meta.len == 5`). Promoted 2026-07-08 from
// canaries/run/filesystem/wrapper_metadata_repro after the awaited real
// macOS/aarch64 run confirmed PASS — pins the two 2026-07-06 selection fixes
// (straight-line-defers-with-leaf; cast-field convert arm) natively on darwin.
#[test]
fn native_wrapper_metadata_passes() {
    assert_pass("native_wrapper_metadata");
}

// The `file_journal` CLI SAMPLE (samples/cli/systems/file_journal) — a real
// end-to-end mixed workflow: portable target-format open/create plus raw
// format-neutral mkdir/write/stat/read/rename/remove/rmdir operations. It
// tallies its 7 verified steps and exits with the count. Covered HERE rather
// than relying on samples_compile, which is currently red from a pre-existing
// aarch64 `b.ne target is not instruction aligned` encoder bug in many unrelated
// samples (algorithms/arithmetic/basics/… — NOT the fs work; see TASKS_FS.md).
#[test]
fn sample_file_journal_exits_7() {
    let main_path = repo_root().join("samples/cli/systems/file_journal/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-journal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("file_journal sample should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(7),
        "file_journal should verify all 7 steps and exit 7"
    );
}

// The `note_vault` CLI SAMPLE (samples/cli/systems/note_vault) -- the FULL
// wrapper surface: create_dir_all ->
// create_new -> write -> append x2 -> metadata_path -> modified-time
// BRIDGE into std::time (from_unix_seconds -> duration_since(now) Ok +
// sane gap) -> read_all -> open_with{write,truncate} compaction -> copy ->
// read_dir_count audit -> remove -> remove_dir_all teardown, tallying its
// 14 verified steps. Runs from a temp cwd so the vault tree lands there.
// Both engines probe-verified: 12 at the dir-walk extension (2026-07-09),
// 14 at the time-bridge extension (2026-07-10j).
#[test]
fn sample_note_vault_exits_14() {
    let main_path = repo_root().join("samples/cli/systems/note_vault/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-vault-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("note_vault sample should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .current_dir(&build_dir)
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(14),
        "note_vault should verify all 14 steps and exit 14"
    );
}

// The arm64 FLOAT-ARGUMENT calling convention: Math::round_nearest(x: f64) -> i64
// via libm lround. Proves an f64 arg is marshalled into v0 (RuntimeScalarFloat
// operand). The direct computed argument round_nearest(3.0 + 0.7) also proves
// host-argument scratch retains its FLOAT register class. It returns 4.
#[test]
fn native_float_arg_exits_4() {
    let main_path = repo_root().join("canaries/pass/float/native_float_arg/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-floatarg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("native_float_arg should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(4),
        "round_nearest(3.0 + 0.7) should be 4 (computed float arg in v0)"
    );
}

// The arm64 FLOAT-RETURN calling convention: Math::square_root(x: f64) -> f64 via
// libm sqrt. Proves the result comes back in d0 and is moved to x0 (fmov x0,d0)
// before the store; the stored f64 is round-tripped through round_nearest to
// verify the bits. sqrt(16.0) -> 4.0 -> round 4 -> exit 4.
#[test]
fn native_float_return_exits_4() {
    let main_path = repo_root().join("canaries/pass/float/native_float_return/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-floatret-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("native_float_return should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(4),
        "sqrt(16.0) round-tripped should be 4 (float return in d0)"
    );
}

// Two f64 ARGUMENTS in consecutive float registers (v0, v1) alongside a float
// return: Math::hypotenuse(x, y) -> f64 via libm hypot. Two direct computed
// arguments independently stage into FLOAT-class scratch before v0/v1.
// hypot(3.0 + 0.0, 4.0 + 0.0) -> 5.0, then round_nearest -> exit 5.
#[test]
fn native_float_two_args_exits_5() {
    let main_path = repo_root().join("canaries/pass/float/native_float_two_args/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-float2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("native_float_two_args should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(5),
        "hypot(3,4) round-tripped should be 5 (args in v0,v1)"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn returning_foreign_call_restores_canonical_float_control_state() {
    let main_path = repo_root().join("canaries/pass/float/foreign_control_state_restore/main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-control-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path.clone(),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("foreign float-control canary should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run foreign float-control canary");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(70),
        "checked arithmetic after fesetround must resume nearest-even; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// Multi-dylib linking: the first call into a SECOND dylib. objc_getClass lives in
// /usr/lib/libobjc.A.dylib (not libSystem), so the Mach-O must emit a 2nd
// LC_LOAD_DYLIB and bind the symbol at dylib ordinal 2. objc_getClass("NSObject")
// returns a non-null Class pointer -> exit 7. A broken second-dylib bind either
// yields cls==0 (exit 1) or aborts at dyld load (non-7 exit) — both caught here.
#[test]
fn objc_get_class_exits_7() {
    let main_path = repo_root().join("canaries/pass/objc/objc_get_class/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-objcclass-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("objc_get_class should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(7),
        "objc_getClass(NSObject) should be non-null (2nd dylib libobjc bound)"
    );
}

// sel_registerName + 2-arg objc_msgSend: [[NSObject class] alloc] returns a
// non-null instance -> exit 7. recv->x0, sel->x1, id result->x0.
#[test]
fn objc_alloc_exits_7() {
    let main_path = repo_root().join("canaries/pass/objc/objc_alloc/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-objcalloc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("objc_alloc should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(7),
        "[[NSObject class] alloc] should be non-null (2-arg objc_msgSend)"
    );
}

// 3-arg objc_msgSend with a SCALAR arg + determinate integer return:
// [NSObject respondsToSelector:@selector(alloc)] == 1 -> exit 8. recv->x0,
// sel->x1, arg(SEL)->x2, BOOL result in x0. The window path's arg shape
// (setActivationPolicy: int, activateIgnoringOtherApps: BOOL).
#[test]
fn objc_msgsend_scalar_exits_8() {
    let main_path = repo_root().join("canaries/pass/objc/objc_msgsend_scalar/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-objcscalar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("objc_msgsend_scalar should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(8),
        "[NSObject respondsToSelector:@selector(alloc)] should be 1 (3-arg scalar msgSend)"
    );
}

// Framework auto-loading: a program touching the objc runtime now loads
// Foundation + AppKit + CoreGraphics, so objc_getClass finds their classes.
// NSString + NSApplication + NSWindow all non-null -> exit 9. Also confirms
// AppKit loads cleanly from a bare CLI mach-o (no .app bundle).
#[test]
fn framework_classes_exits_9() {
    let main_path = repo_root().join("canaries/pass/objc/framework_classes/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-fwclasses-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("framework_classes should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(9),
        "NSString/NSApplication/NSWindow should all resolve (Foundation+AppKit loaded)"
    );
}

// objc_msgSend with a C-string arg + integer return VALUE, now that Foundation
// loads: NSString alloc/initWithUTF8String:"hello", [str length] == 5 -> exit 5.
#[test]
fn nsstring_length_exits_5() {
    let main_path = repo_root().join("canaries/pass/objc/nsstring_length/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-nsstrlen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("nsstring_length should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(5),
        "[[NSString alloc] initWithUTF8String:\"hello\"] length should be 5"
    );
}

// The arm64 HFA calling convention: a CGRect (4 doubles) passed BY VALUE lands in
// v0-v3. CGRectGetMaxX({10,20,30,40}) = v0+v2 = 40, CGRectGetMaxY = v1+v3 = 60;
// both round-tripped through round_nearest -> exit 6. Also proves CoreGraphics is
// bindable as a directly-called framework (no objc).
#[test]
fn cgrect_hfa_exits_6() {
    let main_path = repo_root().join("canaries/pass/objc/cgrect_hfa/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-cgrect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("cgrect_hfa should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(6),
        "CGRectGetMaxX/Y of {{10,20,30,40}} should be 40/60 (HFA in v0-v3)"
    );
}

// The MIXED HFA-plus-scalar objc_msgSend — a real NSWindow built by hand via
// [[NSWindow alloc] initWithContentRect:{0,0,200,150} styleMask:15 backing:2
// defer:0]. The rect goes in v0-v3, styleMask/backing/defer in x2-x4 (independent
// register files). Verifies the window is non-null AND [win styleMask] == 15, so
// both files are placed right. Headless-safe (never ordered on-screen). -> exit 3.
#[test]
fn nswindow_init_exits_3() {
    let main_path = repo_root().join("canaries/pass/objc/nswindow_init/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-nswin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("nswindow_init should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(3),
        "NSWindow initWithContentRect:...styleMask:15 should build + report styleMask 15 (HFA v0-v3 + x2-x4)"
    );
}

// The framebuffer -> CGImage blit path: CGColorSpaceCreateDeviceRGB (0 args) +
// CGBitmapContextCreate (7 register args, framebuffer pointer in x0) +
// CGBitmapContextCreateImage + CGImageGetWidth. A 4x4 BGRA buffer yields a
// CGImage whose width reads back as 4 -> exit 4.
#[test]
fn cgimage_blit_exits_4() {
    let main_path = repo_root().join("canaries/pass/objc/cgimage_blit/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-blit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("cgimage_blit should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(4),
        "CGBitmapContext -> CGImage of a 4x4 buffer should report width 4"
    );
}

// The full frame-presentation object graph: framebuffer -> CGImage -> NSImage
// (initWithCGImage:size:, a scalar in x2 + NSSize in v0,v1) -> NSImageView
// (setImage:) -> NSWindow content view (setContentView:) -> makeKeyAndOrderFront:.
// Verifies the image is attached to the view ([iv image] != nil) -> exit 5.
// Headless-safe: the assert is on the object graph, not on-screen visibility.
#[test]
fn present_frame_exits_5() {
    let main_path = repo_root().join("canaries/pass/objc/present_frame/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-present-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("present_frame should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(5),
        "present_frame should attach the CGImage-backed NSImage to the view"
    );
}

// The NON-BLOCKING event pump: 3x [NSApp nextEventMatchingMask:0xffffffff
// untilDate:[NSDate distantPast] inMode:"kCFRunLoopDefaultMode" dequeue:1] via the
// new send_scalar4 (4 args -> x2-x5). untilDate:distantPast is what makes it
// non-blocking; a regression to a blocking pump would HANG, so this test spawns
// with a deadline and fails loudly instead of hanging the suite. -> exit 6.
#[test]
fn event_pump_exits_6() {
    let main_path = repo_root().join("canaries/pass/objc/event_pump/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-pump-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("event_pump should compile:\n{d:#?}"));
    let mut child = Command::new(build_dir.join("omega-program"))
        .spawn()
        .expect("spawn");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let code = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status.code();
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = std::fs::remove_dir_all(&build_dir);
            panic!("event_pump HUNG — the pump blocked (untilDate not distantPast?)");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        code,
        Some(6),
        "event_pump should complete 3 non-blocking pumps and exit 6"
    );
}

// The macOS Gui-backend building block: an Omega machine that composes the objc
// window primitives (getClass/alloc/initWithContentRect:), reached through a
// same-data-type VALUE-CALL, returns a non-null NSWindow -> exit 7. This is the
// shape the macOS Gui backend uses (one trait-op-sized machine per Gui op); the
// remaining integration gap is provider wiring (boundary Gui trait -> this).
#[test]
fn gui_backend_valuecall_exits_7() {
    let main_path = repo_root().join("canaries/pass/objc/gui_backend_valuecall/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-vcgui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("gui_backend_valuecall should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(7),
        "a value-called Omega machine should compose objc into a non-null NSWindow"
    );
}

// The samples' WHOLE behavior, composed from the proven objc/CG primitives: open
// a window with an NSImageView content view, then a bounded 3-frame loop of
// blit (CGBitmapContext -> CGImage -> NSImage -> setImage:) + non-blocking event
// pump, then [window isVisible] + [window close]. This is samples/gui/window_demo's
// shape running natively. Bounded + headless-safe, but the pump could hang if it
// ever regressed to blocking, so the test spawns with a deadline. -> exit 4.
#[test]
fn native_gui_loop_exits_4() {
    let main_path = repo_root().join("canaries/pass/objc/native_gui_loop/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-guiloop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("native_gui_loop should compile:\n{d:#?}"));
    let mut child = Command::new(build_dir.join("omega-program"))
        .spawn()
        .expect("spawn");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let code = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status.code();
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = std::fs::remove_dir_all(&build_dir);
            panic!("native_gui_loop HUNG — the render/pump loop blocked");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        code,
        Some(4),
        "native_gui_loop should run the full window+blit+pump loop and exit 4"
    );
}

// The macOS Gui backend SHAPE: a separate GuiImpl data type (objc handle + scratch
// fields) implements window_create via objc calls and is reached through a FIELD
// value-call (self.gui.window_create()), like the shipped Filesystem wrapper. A
// non-null window from the through-field call -> exit 7. Confirms the backend can
// be an ordinary Omega wrapper data type; the remaining gap is substituting the
// sample's boundary Gui field with this provider on darwin.
#[test]
fn gui_impl_through_field_exits_7() {
    let main_path = repo_root().join("canaries/pass/objc/gui_impl_through_field/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-tfgui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("gui_impl_through_field should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(7),
        "a through-field GuiImpl wrapper should compose objc into a non-null NSWindow"
    );
}

// The sample-shaped window_create: a GuiImpl wrapper op taking i32 x/y/w/h args
// (as the sample's Gui.window_create does), converting i32 -> f64 via `as f64`
// into scratch fields, and building the NSWindow with an HFA rect -- all through a
// field value-call. Combines int-args-through-value-call + scvtf cast + objc.
// Non-null window -> exit 8. The hardest Gui op proven in its true sample shape.
#[test]
fn gui_window_i32_args_exits_8() {
    let main_path = repo_root().join("canaries/pass/objc/gui_window_i32_args/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-i32gui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("gui_window_i32_args should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(8),
        "i32-arg window_create should convert to an f64 rect and build a non-null NSWindow"
    );
}

// The shipped macOS Gui backend module (omega::language::std::macos_gui) driving
// the FULL window_demo behavior natively: window_create -> get_dc -> a bounded
// per-frame loop of fill (64x64 diagonal wash) -> blit ([i32;4096] framebuffer ->
// CGImage -> setImage:, asserts copied == 64) -> a 3-op message pump loop
// (msg_peek / msg_translate / msg_dispatch) -> is_window liveness -> advance, all
// through a gui: MacosGui concrete-provider field (the proven value-call model).
// Exercises all 7 Gui ops in their real sample loop shape; a clean run -> exit 3.
// This is samples/gui/window_demo minus Clock.sleep pacing + the read_line pause
// (both headless-CI concessions, not behavioral). Spawned with a deadline guard:
// nextEvent on a live window should return immediately (distantPast), but the
// window is real, so a stuck run is killed rather than hanging CI.
#[test]
fn macos_gui_module_exits_3() {
    let main_path = repo_root().join("canaries/pass/objc/macos_gui_module/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-macgui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("macos_gui_module should compile:\n{d:#?}"));
    let mut child = Command::new(build_dir.join("omega-program"))
        .spawn()
        .expect("spawn");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let code = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status.code();
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("macos_gui_module hung past 20s deadline (window pump never drained)");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        code,
        Some(3),
        "full window_demo behavior through MacosGui (all 7 Gui ops in loop) should exit 3"
    );
}

// The macOS `Clock.sleep` native lowering: `self.clock.sleep(ms)` through the
// UNCHANGED `Clock` boundary trait -> `poll(NULL, 0, ms)` (a millisecond sleep).
// The canary sleeps 3x150ms = ~450ms then exits 6. We TIME the run to confirm the
// units are MILLISECONDS: poll-as-milliseconds ~= 450ms; a units bug (usleep-style
// microseconds) would finish ~instantly, and a *1000 error would take ~450s. Assert
// the elapsed wall-clock lands in [250ms, 5s] -- loose enough for CI jitter but tight
// enough to catch a 1000x units error either direction.
#[test]
fn clock_sleep_poll_milliseconds_exits_6() {
    let main_path = repo_root().join("canaries/pass/objc/clock_sleep/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-clocksleep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("clock_sleep should compile:\n{d:#?}"));
    let start = std::time::Instant::now();
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let elapsed = start.elapsed();
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(6),
        "clock_sleep should run the 3x sleep loop and exit 6"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(250)
            && elapsed < std::time::Duration::from_secs(5),
        "3x150ms poll-sleep should take ~450ms (millisecond units); took {elapsed:?}"
    );
}

// Darwin provider-substitution canary: source declares only the abstract `Gui`
// boundary field. The native pipeline injects `MacosGui`, while the interpreter
// retains its abstract headless provider. A non-null window plus `get_dc` echo exits
// with 7.
#[test]
fn gui_provider_substitution_exits_7() {
    let main_path = repo_root().join("canaries/pass/objc/gui_provider_substitution/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-guisubst-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| {
        panic!(
            "gui_provider_substitution should compile via the injected MacosGui provider:\n{d:#?}"
        )
    });
    let mut child = Command::new(build_dir.join("omega-program"))
        .spawn()
        .expect("spawn");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let code = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status.code();
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("gui_provider_substitution hung past 20s deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        code,
        Some(7),
        "gui: Gui boundary field should be substituted to the MacosGui provider and exit 7"
    );
}

// THE MILESTONE: the UNTOUCHED samples/gui/window_demo runs natively end-to-end on
// macOS/aarch64. It opens a real NSWindow (via the substituted MacosGui provider),
// renders 60 frames of a software-rendered diagonal wash (each blit asserts all 64
// source scanlines copied), pumps events, paces with Clock.sleep (poll), then reads a
// line and exits 0. This exercises the whole stack: the Gui-provider substitution
// (#57), Clock.sleep -> poll (#55/fire23), AND the large-offset scalar loads (#59 --
// window_demo declares copied/alive/i AFTER pixels:[i32;4096], so the machine-index
// index load + guards land past the LDR scaled-immediate range). stdin is /dev/null so
// the trailing read_line returns EOF immediately; a deadline guard kills a stuck run.
#[test]
fn sample_window_demo_runs_natively_exits_0() {
    let main_path = repo_root().join("samples/gui/window_demo/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-window-demo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| {
        panic!("the untouched samples/gui/window_demo should compile to a native mach-o:\n{d:#?}")
    });
    let mut child = Command::new(build_dir.join("omega-program"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let code = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status.code();
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("window_demo hung past 30s deadline (window pump / render never finished)");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        code,
        Some(0),
        "the untouched window_demo should render 60 frames natively and exit 0"
    );
}

// The darwin INPUT-provider substitution (task #60): a program declaring the UNCHANGED
// `boundary trait Input` + an `input: Input` field (no `use`) -- like window_app. On
// darwin the compiler injects the bundled MacosInput provider and rewrites the field to
// MacosInput, so `self.input.key_state(27)` maps VK 27 (ESC) -> macOS keycode 53 and
// calls CGEventSourceKeyState. Headless CI does not hold ESC, so it returns 0 -> exit 4.
// Proves the substitution registry generalizes past Gui to Input, and the key-state
// lowering runs end-to-end.
#[test]
fn input_provider_substitution_exits_4() {
    let main_path = repo_root().join("canaries/pass/objc/input_provider_substitution/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-inputsubst-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("input_provider_substitution should compile via the injected MacosInput provider:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(4),
        "input: Input should be substituted to MacosInput; ESC not held -> exit 4"
    );
}

// The UNTOUCHED samples/gui/window_app runs natively: like window_demo but a STANDALONE
// app that stays open until ESC or the window is closed (an infinite render loop). It
// needs BOTH provider substitutions -- Gui (MacosGui) AND Input (MacosInput, for the ESC
// poll) -- plus Clock.sleep and the large-offset scalar loads. Headless CI never presses
// ESC / closes the window, so it renders forever; we confirm it STARTS and RENDERS
// without crashing for 2s (a non-zero exit inside 2s = a crash), then kill it.
#[test]
fn sample_window_app_renders_natively() {
    let main_path = repo_root().join("samples/gui/window_app/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-window-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| {
        panic!("the untouched samples/gui/window_app should compile to a native mach-o:\n{d:#?}")
    });
    let mut child = Command::new(build_dir.join("omega-program"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    // Let it render for ~2s; a crash would surface as an early non-zero exit.
    let watch_until = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let early = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break Some(status);
        }
        if std::time::Instant::now() > watch_until {
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&build_dir);
    if let Some(status) = early {
        // It exited on its own within 2s -- only a clean exit 0 is acceptable (ESC
        // detection or window close); any other code is a crash/assertion failure.
        assert_eq!(
            status.code(),
            Some(0),
            "window_app exited early with a non-zero code (crash) instead of rendering"
        );
    }
    // Still running after 2s => rendering the infinite loop fine; killed above.
}

// aarch64 SATURATING signed divide/modulo (task #62): normal cases plus the TYPE_MIN
// / -1 corner. Unlike x86 idiv, aarch64 sdiv does not trap there (it wraps to
// TYPE_MIN); Saturating must instead clamp `i32::MIN / -1` up to `i32::MAX` and give
// `i32::MIN % -1 == 0`. The canary checks 23/5=4, 23%5=3, i32::MIN/-1=i32::MAX,
// i32::MIN%-1=0, 10/-1=-10 -> exit 7.
#[test]
fn saturating_divide_native_exits_7() {
    let main_path = repo_root().join("canaries/pass/arithmetic/saturating_divide_native/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-satdiv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("saturating_divide_native should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(
        out.status.code(),
        Some(7),
        "saturating signed div/mod (incl. i32::MIN/-1 -> i32::MAX) should exit 7"
    );
}

// The UNTOUCHED samples/gui/windowed_calculator runs natively: a persistent calculator
// window combining Gui + Input (ESC / keys) + Clock + Saturating i32 arithmetic
// (add/sub/mul AND divide/modulo). It needs the Gui + Input substitutions, Clock.sleep,
// the large-offset scalar loads, AND aarch64 saturating divide/modulo (task #62). Like
// window_app it stays open until closed, so we confirm it STARTS + RENDERS without
// crashing for 2s, then kill it.
#[test]
fn sample_windowed_calculator_renders_natively() {
    let main_path = repo_root().join("samples/gui/windowed_calculator/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-calc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("the untouched samples/gui/windowed_calculator should compile to a native mach-o:\n{d:#?}"));
    let mut child = Command::new(build_dir.join("omega-program"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    let watch_until = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let early = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break Some(status);
        }
        if std::time::Instant::now() > watch_until {
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&build_dir);
    if let Some(status) = early {
        assert_eq!(
            status.code(),
            Some(0),
            "windowed_calculator exited early with a non-zero code (crash) instead of rendering"
        );
    }
}

// The UNTOUCHED samples/gui/image_viewer runs natively: it loads img{0,1,2}.bmp from
// disk (the fs raw seam), decodes each 24bpp BMP into a top-down 32bpp framebuffer, and
// software-blits it into a window; RIGHT/LEFT flip, ESC closes. It combines EVERY native
// capability built for the gui samples: the Gui + Input provider substitutions,
// Clock.sleep, FilesystemHost, AND the large-offset scalar sweep (task #61 -- its two
// 16KB arrays push most fields past the LDR/STR/ADD immediate ranges). Human-interactive
// (waits for a window close), so we confirm it STARTS + RENDERS without crashing for 2s.
// Run from the sample dir so the relative img*.bmp paths resolve.
#[test]
fn sample_image_viewer_renders_natively() {
    let sample_dir = repo_root().join("samples/gui/image_viewer");
    let main_path = sample_dir.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-image-viewer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| {
        panic!("the untouched samples/gui/image_viewer should compile to a native mach-o:\n{d:#?}")
    });
    let mut child = Command::new(build_dir.join("omega-program"))
        .current_dir(&sample_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    let watch_until = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let early = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break Some(status);
        }
        if std::time::Instant::now() > watch_until {
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&build_dir);
    if let Some(status) = early {
        assert_eq!(
            status.code(),
            Some(0),
            "image_viewer exited early with a non-zero code (crash) instead of rendering"
        );
    }
}
