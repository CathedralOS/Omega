use super::fixture_roster;
use super::{compile_exact_macos_entry, repo_root};
use compiler::CompileOptions;
use std::process::Command;

#[test]
fn sample_file_journal_exits_7() {
    let main_path = repo_root().join("samples/cli/systems/file_journal/main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-journal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::NATIVE_FLOAT_ARG.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-floatarg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::NATIVE_FLOAT_RETURN.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-floatret-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::NATIVE_FLOAT_TWO_ARGS.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-float2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::FOREIGN_CONTROL_STATE_RESTORE.path)
        .join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-control-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path.clone(),
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::OBJC_GET_CLASS.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-objcclass-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::OBJC_ALLOC.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-objcalloc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::OBJC_MSGSEND_SCALAR.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-objcscalar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::FRAMEWORK_CLASSES.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-fwclasses-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::NSSTRING_LENGTH.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-nsstrlen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::CGRECT_HFA.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-cgrect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::NSWINDOW_INIT.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-nswin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::CGIMAGE_BLIT.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-blit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::PRESENT_FRAME.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-present-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
