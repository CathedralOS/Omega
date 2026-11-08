use std::process::{Command, Stdio};
use std::time::Duration;

use super::super::lifecycle::WindowsJobChild;
use super::support::{test_limits, wait_bounded};

#[test]
fn job_reaches_active_process_zero_after_normal_exit() {
    let mut command = Command::new("cmd.exe");
    command
        .args(["/D", "/S", "/C", "exit /b 0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let limits = test_limits(4, 512, 1024, Duration::from_secs(30));
    let mut child =
        WindowsJobChild::spawn_with_config(&mut command, limits).expect("spawn contained child");
    assert!(wait_bounded(&mut child, Duration::from_secs(5)).success());
}

#[test]
fn job_termination_reaches_active_process_zero() {
    let mut command = Command::new("cmd.exe");
    command
        .args(["/D", "/S", "/C", "ping -n 30 127.0.0.1 >nul"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let limits = test_limits(4, 512, 1024, Duration::from_secs(30));
    let mut child =
        WindowsJobChild::spawn_with_config(&mut command, limits).expect("spawn contained child");
    child.kill().expect("terminate complete Windows Job");
    assert!(!wait_bounded(&mut child, Duration::from_secs(5)).success());
}
