use super::fixture_roster;
use super::{compile_exact_macos_entry, repo_root};
use compiler::CompileOptions;
use std::process::Command;

#[test]
fn event_pump_exits_6() {
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::EVENT_PUMP.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-pump-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::GUI_BACKEND_VALUECALL.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-vcgui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::NATIVE_GUI_LOOP.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-guiloop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::GUI_IMPL_THROUGH_FIELD.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-tfgui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::GUI_WINDOW_I32_ARGS.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-i32gui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::MACOS_GUI_MODULE.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-macgui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::CLOCK_SLEEP.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-clocksleep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::GUI_PROVIDER_SUBSTITUTION.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-guisubst-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let report = compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
    })
    .unwrap_or_else(|d| {
        panic!("the untouched samples/gui/window_demo should compile to a native mach-o:\n{d:#?}")
    });
    // A selected macOS GUI product publishes one `.app`; the checked
    // executable accessor names the inner `Contents/MacOS/<name>` path
    // (wiki/spec/build/macos_application.md).
    let package_root = report
        .checked_native_package_path()
        .expect("window_demo publishes one .app package")
        .to_path_buf();
    let executable = report
        .checked_native_executable_path()
        .expect("window_demo retains the inner executable path")
        .to_path_buf();
    assert_eq!(executable, package_root.join("Contents/MacOS/window-demo"));
    let mut child = Command::new(&executable)
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::INPUT_PROVIDER_SUBSTITUTION.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-inputsubst-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
    })
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
    let report = compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
    })
    .unwrap_or_else(|d| {
        panic!("the untouched samples/gui/window_app should compile to a native mach-o:\n{d:#?}")
    });
    let package_root = report
        .checked_native_package_path()
        .expect("window_app publishes one .app package")
        .to_path_buf();
    let executable = report
        .checked_native_executable_path()
        .expect("window_app retains the inner executable path")
        .to_path_buf();
    assert_eq!(executable, package_root.join("Contents/MacOS/window-app"));
    let mut child = Command::new(&executable)
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
    let main_path = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::SATURATING_DIVIDE_NATIVE.path)
        .join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-satdiv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
    let report = compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
    })
        .unwrap_or_else(|d| panic!("the untouched samples/gui/windowed_calculator should compile to a native mach-o:\n{d:#?}"));
    let executable = report
        .checked_native_executable_path()
        .expect("windowed_calculator retains the inner executable path")
        .to_path_buf();
    assert_eq!(
        executable,
        report
            .checked_native_package_path()
            .expect("windowed_calculator publishes one .app package")
            .join("Contents/MacOS/windowed-calculator")
    );
    let mut child = Command::new(&executable)
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
    let report = compile_exact_macos_entry(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
    })
    .unwrap_or_else(|d| {
        panic!("the untouched samples/gui/image_viewer should compile to a native mach-o:\n{d:#?}")
    });
    // image_viewer is deferred from the v1 package contract for resource
    // inclusion, but its inner executable still runs directly with the
    // sample directory as its working directory.
    let executable = report
        .checked_native_executable_path()
        .expect("image_viewer retains the inner executable path")
        .to_path_buf();
    assert_eq!(
        executable,
        report
            .checked_native_package_path()
            .expect("image_viewer publishes one .app package")
            .join("Contents/MacOS/image-viewer")
    );
    let mut child = Command::new(&executable)
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
