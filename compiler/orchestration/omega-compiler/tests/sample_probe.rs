// THROWAWAY TEST — delete before finishing.
// Probes new samples: compiles each and checks exit code.
// Also probes the alarm_scheduler miscompile to narrow the root cause.

use omega_compiler::{CompileOptions, compile};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repo root")
        .to_path_buf()
}

fn executable_name() -> &'static str {
    if cfg!(windows) { "omega-program.exe" } else { "omega-program" }
}

fn compile_and_run(sample_name: &str) -> Option<i32> {
    let sample = repo_root().join("samples").join(sample_name);
    let build_dir = std::env::temp_dir()
        .join(format!("omega-{}-{}", sample_name.replace('_', "-"), std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: sample.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        write_output: true,
    })
    .ok()?;

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .ok()?;
    let code = output.status.code();
    let _ = fs::remove_dir_all(&build_dir);
    code
}

#[cfg(windows)]
#[test]
fn alarm_scheduler_exits_70() {
    let code = compile_and_run("alarm_scheduler");
    assert_eq!(code, Some(70), "alarm_scheduler: expected 70, got {:?}", code);
}

#[cfg(windows)]
#[test]
fn pixel_canvas_exits_70() {
    let code = compile_and_run("pixel_canvas");
    assert_eq!(code, Some(70), "pixel_canvas: expected 70, got {:?}", code);
}

#[cfg(windows)]
#[test]
fn width_mixer_exits_70() {
    let code = compile_and_run("width_mixer");
    assert_eq!(code, Some(70), "width_mixer: expected 70, got {:?}", code);
}

#[cfg(windows)]
#[test]
fn dual_float_exits_70() {
    let code = compile_and_run("dual_float");
    assert_eq!(code, Some(70), "dual_float: expected 70, got {:?}", code);
}

#[cfg(windows)]
#[test]
fn task_runner_exits_70() {
    let code = compile_and_run("task_runner");
    assert_eq!(code, Some(70), "task_runner: expected 70, got {:?}", code);
}

// Miscompile isolation probes for alarm_scheduler.
// alarm_probe tests the minimal trigger: 5-arm case dispatch with Trigger at position 3.
#[cfg(windows)]
#[test]
fn alarm_probe_trigger_exits_70() {
    let code = compile_and_run("alarm_probe");
    assert_eq!(code, Some(70), "alarm_probe (Trigger x2 should give fire_count=2): expected 70, got {:?}", code);
}
