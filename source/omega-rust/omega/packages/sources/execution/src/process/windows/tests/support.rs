use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::super::lifecycle::{JobLimitEvent, WindowsJobChild};
use super::super::limits::WindowsJobLimits;

pub(super) const WORKER_MODE: &str = "OMEGA_RESOLVER_WINDOWS_JOB_WORKER";
pub(super) const WORKER_VALUE: &str = "OMEGA_RESOLVER_WINDOWS_JOB_WORKER_VALUE";
pub(super) const WORKER_TEST: &str = "process::windows::tests::worker::job_limit_worker";
pub(super) const MIB: u64 = 1024 * 1024;
pub(super) static JOB_LIMIT_TEST_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn wait_bounded(
    child: &mut WindowsJobChild,
    timeout: Duration,
) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("query Windows Job completion") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "Windows Job did not reach active-process zero"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn test_limits(
    active_processes: u64,
    process_memory_mib: u64,
    aggregate_memory_mib: u64,
    aggregate_cpu: Duration,
) -> WindowsJobLimits {
    let aggregate_cpu_ticks = u64::try_from(aggregate_cpu.as_nanos() / u128::from(100_u64))
        .expect("test CPU limit fits Windows ticks");
    WindowsJobLimits {
        active_processes,
        process_memory_bytes: process_memory_mib * MIB,
        aggregate_memory_bytes: aggregate_memory_mib * MIB,
        aggregate_cpu_ticks,
    }
}

pub(super) fn worker_command(mode: &str, value: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("resolve test executable"));
    command
        .args(["--exact", WORKER_TEST, "--test-threads=1"])
        .env(WORKER_MODE, mode)
        .env(WORKER_VALUE, value)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

pub(super) fn run_worker(
    mode: &str,
    value: &str,
    limits: WindowsJobLimits,
    timeout: Duration,
) -> (std::process::ExitStatus, Vec<JobLimitEvent>) {
    let mut command = worker_command(mode, value);
    let mut child = WindowsJobChild::spawn_with_config(&mut command, limits)
        .expect("spawn limited Windows Job worker");
    let status = wait_bounded(&mut child, timeout);
    (status, child.limit_events.clone())
}
