//! Headless GUI parity pin: `samples/gui/window_demo` runs interpreted to the
//! same exit 0 as the macOS-gated native build.
//! The virtual window system mints live handles, serves DCs/blits, reports
//! "no key / no event", and the sample's own 60-frame loop terminates -- no
//! real window opens, so this pin is platform-independent.
//!
//! The run needs ~40M interpreter steps (60 frames of software-rendered
//! pixels are genuine language-semantics work, not host ops), above the 10M
//! runaway default -- OMEGA_INTERP_STEP_BUDGET raises it for this process
//! (a dedicated test binary, so the env write races nothing).

use omega_compiler::compile_to_checked;
use std::path::PathBuf;

#[test]
fn window_demo_runs_headless_to_native_exit() {
    // SAFETY/order: this test file is its own binary and this is its only
    // test, so the process-global env write cannot race another test.
    unsafe { std::env::set_var("OMEGA_INTERP_STEP_BUDGET", "100000000") };

    let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../samples/gui/window_demo/main.omg");
    let checked = compile_to_checked(&sample, None).unwrap_or_else(|diagnostics| {
        panic!(
            "window_demo should compile for the interpreter:\n{}",
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let outcome = psi_checked_interpreter::interpret_entry(&checked, "Main::main", &[]);
    assert_eq!(
        outcome.error, None,
        "the headless run must not decline: {:?}",
        outcome.error
    );
    assert_eq!(
        outcome.exit_code,
        0,
        "headless window_demo must exit 0 like the native run; stderr:\n{}",
        String::from_utf8_lossy(&outcome.stderr)
    );
}
