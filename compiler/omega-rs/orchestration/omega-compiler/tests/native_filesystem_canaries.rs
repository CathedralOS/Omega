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
// The value-call transition-guard DEEP BUG, now FIXED (both halves): a callee that
// branches on a host-call result in an internal transition, whose bool result is
// assigned to a field. Guards the ordering fix + the field-mutation constant-fold fix.
#[test]
fn native_value_call_guard_passes() { assert_pass("native_value_call_guard"); }
// ENUM-transition-leaf delivery: a value-call whose callee transitions to enum
// leaves (Err / Ok{pair}) delivers the correct arm's tag+payload to a field. Guards
// the nullary-enum-variant frame-slot tag write (mutation/frame_slots.rs).
#[test]
fn native_enum_result_passes() { assert_pass("native_enum_result"); }
// The PAYLOAD-CARRYING ergonomic wrapper result natively (unblocked by the deep
// fix): `Filesystem::write_all -> UnitResult` reports Error for a bad path and Ok
// for a good one — the RESULT, not just the side effect, is now correct.
#[test]
fn native_wrapper_write_all_result_passes() { assert_pass("native_wrapper_write_all_result"); }
// `Filesystem::try_exists -> ExistsResult` Yes/No natively: the faithful 3-way now
// captures errno into a field in the entry (before branching) so the No-vs-Error
// split guards on a stored field, not a nested host-call-in-guard the deep fix
// doesn't reach at that nesting.
#[test]
fn native_wrapper_try_exists_passes() { assert_pass("native_wrapper_try_exists"); }

// The `file_journal` CLI SAMPLE (samples/cli/systems/file_journal) — a real
// end-to-end raw-seam workflow (mkdir -> create+write -> stat -> reopen+read ->
// rename -> remove -> rmdir) that tallies its 7 verified steps and exits with the
// count. Given green regression coverage HERE (this fs harness is green) rather
// than relying on samples_compile, which is currently red from a pre-existing
// aarch64 `b.ne target is not instruction aligned` encoder bug in many unrelated
// samples (algorithms/arithmetic/basics/… — NOT the fs work; see TASKS_FS.md).
#[test]
fn sample_file_journal_exits_7() {
    let main_path = repo_root().join("samples/cli/systems/file_journal/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-journal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|d| panic!("file_journal sample should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(7), "file_journal should verify all 7 steps and exit 7");
}

// The arm64 FLOAT-ARGUMENT calling convention: Math::round_nearest(x: f64) -> i64
// via libm lround. Proves an f64 arg is marshalled into v0 (RuntimeScalarFloat
// operand). round_nearest(3.7) == 4 -> exit 4.
#[test]
fn native_float_arg_exits_4() {
    let main_path = repo_root().join("canaries/pass/float/native_float_arg/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-floatarg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("native_float_arg should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(4), "round_nearest(3.7) should be 4 (float arg in v0)");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("native_float_return should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(4), "sqrt(16.0) round-tripped should be 4 (float return in d0)");
}

// Two f64 ARGUMENTS in consecutive float registers (v0, v1) alongside a float
// return: Math::hypotenuse(x, y) -> f64 via libm hypot. hypot(3.0, 4.0) -> 5.0
// round-tripped through round_nearest -> exit 5.
#[test]
fn native_float_two_args_exits_5() {
    let main_path = repo_root().join("canaries/pass/float/native_float_two_args/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-float2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("native_float_two_args should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(5), "hypot(3,4) round-tripped should be 5 (args in v0,v1)");
}

// Three f64 args reaching v2 (the HFA ABI proof): Math::fused_multiply_add(x,y,z)
// -> f64 via libm fma. fma(2,3,4) = 10.0 round-tripped through round_nearest ->
// exit 10. Establishes that an HFA of ≤4 doubles (NSRect) marshals into v0–v3,
// since HFA members and N separate double args occupy the same v-registers.
#[test]
fn native_float_three_args_exits_10() {
    let main_path = repo_root().join("canaries/pass/float/native_float_three_args/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-float3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("native_float_three_args should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(10), "fma(2,3,4) round-tripped should be 10 (args in v0,v1,v2)");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("objc_get_class should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(7), "objc_getClass(NSObject) should be non-null (2nd dylib libobjc bound)");
}

// sel_registerName + 2-arg objc_msgSend: [[NSObject class] alloc] returns a
// non-null instance -> exit 7. recv->x0, sel->x1, id result->x0.
#[test]
fn objc_alloc_exits_7() {
    let main_path = repo_root().join("canaries/pass/objc/objc_alloc/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-objcalloc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("objc_alloc should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(7), "[[NSObject class] alloc] should be non-null (2-arg objc_msgSend)");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("objc_msgsend_scalar should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(8), "[NSObject respondsToSelector:@selector(alloc)] should be 1 (3-arg scalar msgSend)");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("framework_classes should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(9), "NSString/NSApplication/NSWindow should all resolve (Foundation+AppKit loaded)");
}

// objc_msgSend with a C-string arg + integer return VALUE, now that Foundation
// loads: NSString alloc/initWithUTF8String:"hello", [str length] == 5 -> exit 5.
#[test]
fn nsstring_length_exits_5() {
    let main_path = repo_root().join("canaries/pass/objc/nsstring_length/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-nsstrlen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("nsstring_length should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(5), "[[NSString alloc] initWithUTF8String:\"hello\"] length should be 5");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("cgrect_hfa should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(6), "CGRectGetMaxX/Y of {{10,20,30,40}} should be 40/60 (HFA in v0-v3)");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("nswindow_init should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(3), "NSWindow initWithContentRect:...styleMask:15 should build + report styleMask 15 (HFA v0-v3 + x2-x4)");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("cgimage_blit should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(4), "CGBitmapContext -> CGImage of a 4x4 buffer should report width 4");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("present_frame should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(5), "present_frame should attach the CGImage-backed NSImage to the view");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("event_pump should compile:\n{d:#?}"));
    let mut child = Command::new(build_dir.join("omega-program")).spawn().expect("spawn");
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
    assert_eq!(code, Some(6), "event_pump should complete 3 non-blocking pumps and exit 6");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("gui_backend_valuecall should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(7), "a value-called Omega machine should compose objc into a non-null NSWindow");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("native_gui_loop should compile:\n{d:#?}"));
    let mut child = Command::new(build_dir.join("omega-program")).spawn().expect("spawn");
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
    assert_eq!(code, Some(4), "native_gui_loop should run the full window+blit+pump loop and exit 4");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("gui_impl_through_field should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(7), "a through-field GuiImpl wrapper should compose objc into a non-null NSWindow");
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
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("gui_window_i32_args should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(8), "i32-arg window_create should convert to an f64 rect and build a non-null NSWindow");
}

// The shipped macOS Gui backend module (omega::language::std::macos_gui): its
// MacosGui wrapper drives the sample-shaped Gui ops -- window_create (Win32
// signature) -> get_dc -> blit (a [i32;4096] framebuffer -> CGImage -> setImage:)
// -> is_window -> window_destroy, through a gui: MacosGui field. window != 0 AND
// get_dc(win) == win AND blit > 0 -> exit 9. (is_window / window_destroy run but
// are not gated on visibility, headless-safe.)
#[test]
fn macos_gui_module_exits_9() {
    let main_path = repo_root().join("canaries/pass/objc/macos_gui_module/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-macgui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile(CompileOptions { root_path: main_path, build_dir: Some(build_dir.clone()), target_name: None, write_output: true })
        .unwrap_or_else(|d| panic!("macos_gui_module should compile:\n{d:#?}"));
    let out = Command::new(build_dir.join("omega-program")).output().expect("run");
    let _ = std::fs::remove_dir_all(&build_dir);
    assert_eq!(out.status.code(), Some(9), "MacosGui window lifecycle (create/get_dc/is_window/destroy) should run and exit 9");
}
