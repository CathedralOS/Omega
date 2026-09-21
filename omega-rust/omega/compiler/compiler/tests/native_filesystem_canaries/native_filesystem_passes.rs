use super::fixture_roster;
use super::{assert_pass, compile_exact_macos_entry, compile_run};
use compiler::CompileOptions;
use std::process::Command;

#[test]
fn native_close_still_compiles_and_runs() {
    // No Path domain, only close(fd); just needs to compile + run without crashing.
    let (code, _) = compile_run(fixture_roster::NATIVE_CLOSE);
    assert!(code.is_some(), "native_close should run to a normal exit");
}

#[test]
fn native_stat_still_passes() {
    assert_pass(fixture_roster::NATIVE_STAT);
}

#[test]
fn native_crud_still_passes() {
    assert_pass(fixture_roster::NATIVE_CRUD);
}

#[test]
fn native_dirs_still_passes() {
    assert_pass(fixture_roster::NATIVE_DIRS);
}

#[test]
fn native_read_dir_iter_still_passes() {
    assert_pass(fixture_roster::NATIVE_READ_DIR_ITER);
}

#[test]
fn native_flock_still_passes() {
    assert_pass(fixture_roster::NATIVE_FLOCK);
}
// The WRAPPER lock family (lock/lock_shared/try_lock/try_lock_shared/unlock,
// Rust File::lock parity) + metadata(File) (the fstat wrapper) -- first
// runtime coverage (both were zero-caller). flock has no msvcrt equivalent,
// so this lives here (macOS-gated) like native_flock, not in the differential
// RUN_CANARIES. Exit-coded (70 = full single-fd lock cycle + meta.len check);
// the interpreter leg was probe-verified at 70 when this landed.

#[test]
fn wrapper_lock_metadata_exit_runs() {
    let (code, _) = compile_run(fixture_roster::WRAPPER_LOCK_METADATA_EXIT);
    assert_eq!(code, Some(70), "wrapper lock/metadata cycle should exit 70");
}
// Closes the wrapper zero-caller sweep: set_times (futimens, read back via
// metadata(File)), the set_owner family (uid/gid -1 = unprivileged no-op),
// and symlink_metadata (lstat on a regular file). chown/futimens/lstat have
// no msvcrt rows, so macOS-gated here; interp leg probe-verified at 70.

#[test]
fn wrapper_times_owner_lstat_exit_runs() {
    let (code, _) = compile_run(fixture_roster::WRAPPER_TIMES_OWNER_LSTAT_EXIT);
    assert_eq!(code, Some(70), "wrapper times/owner/lstat should exit 70");
}
// The dirfd REWIND: a second first-entry read on the same fd must return the
// entry (native getdirentries advances the FD OFFSET; read_dir_entry_fd now
// lseeks to 0 first). The iterative remove_dir_all drain depends on this.

#[test]
fn dirfd_reread_exit_runs() {
    let (code, _) = compile_run(fixture_roster::DIRFD_REREAD_EXIT);
    assert_eq!(code, Some(70), "dirfd re-read should exit 70");
}
// The dir-walk wrapper family END TO END (create_dir_all -> read_dir_count /
// is_empty -> remove_dir_all): the capstone of the call-with-return arc.

#[test]
fn dir_walk_wrappers_exit_runs() {
    let (code, _) = compile_run(fixture_roster::DIR_WALK_WRAPPERS_EXIT);
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
use omega::language::core::service;

data Main {{
    fs: Filesystem;
    console: Service<Console>;
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
    assert_pass(fixture_roster::NATIVE_AT_OPS);
}

#[test]
fn native_at_runtime_name_passes() {
    assert_pass(fixture_roster::NATIVE_AT_RUNTIME_NAME);
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
    assert_pass(fixture_roster::NATIVE_APPEND);
}

#[test]
fn native_open_rw_passes() {
    assert_pass(fixture_roster::NATIVE_OPEN_RW);
}

#[test]
fn native_open_create_passes() {
    assert_pass(fixture_roster::NATIVE_OPEN_CREATE);
}

#[test]
fn native_seek_passes() {
    assert_pass(fixture_roster::NATIVE_SEEK);
}

#[test]
fn native_positioned_io_passes() {
    assert_pass(fixture_roster::NATIVE_POSITIONED_IO);
}

#[test]
fn native_errno_passes() {
    assert_pass(fixture_roster::NATIVE_ERRNO);
}

#[test]
fn native_fs_workflow_passes() {
    assert_pass(fixture_roster::NATIVE_FS_WORKFLOW);
}

// Value-call literal forwarding (aliased-literal operand resolution, step 14 fix #1)

#[test]
fn native_value_call_literal_passes() {
    assert_pass(fixture_roster::NATIVE_VALUE_CALL_LITERAL);
}

#[test]
fn native_value_call_path_passes() {
    assert_pass(fixture_roster::NATIVE_VALUE_CALL_PATH);
}
// `let`-bound host call forwarded through a same-machine value-call (step 14
// layers 2+3: LocalData collection + LocalStorage emission) — the ergonomic
// wrapper's shape, for a SAME-data-type callee.

#[test]
fn native_value_call_local_passes() {
    assert_pass(fixture_roster::NATIVE_VALUE_CALL_LOCAL);
}

// Copy / buffer marshalling

#[test]
fn native_buffer_copy_passes() {
    assert_pass(fixture_roster::NATIVE_BUFFER_COPY);
}

#[test]
fn native_subslice_copy_passes() {
    assert_pass(fixture_roster::NATIVE_SUBSLICE_COPY);
}

#[test]
fn native_copy_preserve_passes() {
    assert_pass(fixture_roster::NATIVE_COPY_PRESERVE);
}

#[test]
fn native_forwarded_slice_literal_passes() {
    assert_pass(fixture_roster::NATIVE_FORWARDED_SLICE_LITERAL);
}

// Links, rename, truncation, permissions

#[test]
fn native_rename_passes() {
    assert_pass(fixture_roster::NATIVE_RENAME);
}

#[test]
fn native_hard_link_passes() {
    assert_pass(fixture_roster::NATIVE_HARD_LINK);
}

#[test]
fn native_symlink_passes() {
    assert_pass(fixture_roster::NATIVE_SYMLINK);
}

#[test]
fn native_set_len_passes() {
    assert_pass(fixture_roster::NATIVE_SET_LEN);
}

#[test]
fn native_permissions_passes() {
    assert_pass(fixture_roster::NATIVE_PERMISSIONS);
}

#[test]
fn native_fchmod_passes() {
    assert_pass(fixture_roster::NATIVE_FCHMOD);
}
// Ownership: expects a NON-root user (a real chown to root -> EPERM). Would fail
// only if the suite were ever run as root, which the dev/CI macOS box is not.

#[test]
fn native_chown_passes() {
    assert_pass(fixture_roster::NATIVE_CHOWN);
}

// Existence / classification / path resolution

#[test]
fn native_exists_passes() {
    assert_pass(fixture_roster::NATIVE_EXISTS);
}

#[test]
fn native_try_exists_passes() {
    assert_pass(fixture_roster::NATIVE_TRY_EXISTS);
}

#[test]
fn native_filetype_passes() {
    assert_pass(fixture_roster::NATIVE_FILETYPE);
}

#[test]
fn native_canonicalize_passes() {
    assert_pass(fixture_roster::NATIVE_CANONICALIZE);
}

#[test]
fn native_try_clone_passes() {
    assert_pass(fixture_roster::NATIVE_TRY_CLONE);
}

#[test]
fn native_read_dir_passes() {
    assert_pass(fixture_roster::NATIVE_READ_DIR);
}

// Durability

#[test]
fn native_sync_passes() {
    assert_pass(fixture_roster::NATIVE_SYNC);
}

#[test]
fn native_sync_data_passes() {
    assert_pass(fixture_roster::NATIVE_SYNC_DATA);
}

#[test]
fn native_set_times_passes() {
    assert_pass(fixture_roster::NATIVE_SET_TIMES);
}

// Metadata decode (struct stat byte-assembly)

#[test]
fn native_fstat_passes() {
    assert_pass(fixture_roster::NATIVE_FSTAT);
}

#[test]
fn native_symlink_metadata_passes() {
    assert_pass(fixture_roster::NATIVE_SYMLINK_METADATA);
}

#[test]
fn native_metadata_nlink_passes() {
    assert_pass(fixture_roster::NATIVE_METADATA_NLINK);
}

#[test]
fn native_metadata_ino_passes() {
    assert_pass(fixture_roster::NATIVE_METADATA_INO);
}

#[test]
fn native_metadata_ctime_dev_passes() {
    assert_pass(fixture_roster::NATIVE_METADATA_CTIME_DEV);
}

#[test]
fn native_metadata_blocks_passes() {
    assert_pass(fixture_roster::NATIVE_METADATA_BLOCKS);
}

#[test]
fn native_metadata_modified_passes() {
    assert_pass(fixture_roster::NATIVE_METADATA_MODIFIED);
}

#[test]
fn native_metadata_times_passes() {
    assert_pass(fixture_roster::NATIVE_METADATA_TIMES);
}

#[test]
fn native_metadata_readonly_passes() {
    assert_pass(fixture_roster::NATIVE_METADATA_READONLY);
}

#[test]
fn native_value_call_let_chain_passes() {
    assert_pass(fixture_roster::NATIVE_VALUE_CALL_LET_CHAIN);
}
// The SHIPPED ergonomic Filesystem wrapper natively (step 14 COMPLETE, all 5 layers)

#[test]
fn native_wrapper_write_all_passes() {
    assert_pass(fixture_roster::NATIVE_WRAPPER_WRITE_ALL);
}
// STAT wrapper `Filesystem::exists` natively via TERMINAL-VALUE COMPLETION —
// the no-transition workaround for the value-call guard-ordering bug.

#[test]
fn native_wrapper_exists_passes() {
    assert_pass(fixture_roster::NATIVE_WRAPPER_EXISTS);
}
// The value-call transition-guard DEEP BUG, now FIXED (both halves): a callee that
// branches on a host-call result in an internal transition, whose bool result is
// assigned to a field. Guards the ordering fix + the field-mutation constant-fold fix.

#[test]
fn native_value_call_guard_passes() {
    assert_pass(fixture_roster::NATIVE_VALUE_CALL_GUARD);
}
// ENUM-transition-leaf delivery: a value-call whose callee transitions to enum
// leaves (Err / Ok{pair}) delivers the correct arm's tag+payload to a field. Guards
// the nullary-enum-variant frame-slot tag write (mutation/frame_slots.rs).

#[test]
fn native_enum_result_passes() {
    assert_pass(fixture_roster::NATIVE_ENUM_RESULT);
}
// The PAYLOAD-CARRYING ergonomic wrapper result natively (unblocked by the deep
// fix): `Filesystem::write_all -> UnitResult` reports Error for a bad path and Ok
// for a good one — the RESULT, not just the side effect, is now correct.

#[test]
fn native_wrapper_write_all_result_passes() {
    assert_pass(fixture_roster::NATIVE_WRAPPER_WRITE_ALL_RESULT);
}
// `Filesystem::try_exists -> ExistsResult` Yes/No natively: the faithful 3-way now
// captures errno into a field in the entry (before branching) so the No-vs-Error
// split guards on a stored field, not a nested host-call-in-guard the deep fix
// doesn't reach at that nesting.

#[test]
fn native_wrapper_try_exists_passes() {
    assert_pass(fixture_roster::NATIVE_WRAPPER_TRY_EXISTS);
}
// `Filesystem::metadata_path -> MetadataResult::Ok { meta }` with the PAYLOAD
// destructured and USED (`meta.len == 5`). Promoted 2026-07-08 from
// tests/omega/run/filesystem/wrapper_metadata_repro after the awaited real
// macOS/aarch64 run confirmed PASS — pins the two 2026-07-06 selection fixes
// (straight-line-defers-with-leaf; cast-field convert arm) natively on darwin.

#[test]
fn native_wrapper_metadata_passes() {
    assert_pass(fixture_roster::NATIVE_WRAPPER_METADATA);
}

// The `file_journal` CLI SAMPLE (samples/cli/systems/file_journal) — a real
// end-to-end mixed workflow: portable target-format open/create plus raw
// format-neutral mkdir/write/stat/read/rename/remove/rmdir operations. It
// tallies its 7 verified steps and exits with the count. Covered HERE rather
// than relying on samples_compile, which is currently red from a pre-existing
// aarch64 `b.ne target is not instruction aligned` encoder bug in many unrelated
// samples (algorithms/arithmetic/basics/… — NOT the fs work; see TASKS_FS.md).
