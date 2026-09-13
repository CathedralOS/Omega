#![cfg(unix)]

use super::{
    BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessInput, BoundedProcessRunError,
    BoundedProcessStream, run_bounded_process,
};
use crate::{BoundedProcessLimits, BoundedProcessPrepared};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

fn process_limits() -> BoundedProcessLimits {
    BoundedProcessLimits::new(
        30,
        1024 * 1024 * 1024,
        64 * 1024 * 1024,
        64,
        8,
        512 * 1024 * 1024,
        768 * 1024 * 1024,
    )
}

fn capture_limits(
    stdout_bytes: usize,
    stderr_bytes: usize,
    timeout: Duration,
) -> BoundedCaptureLimits {
    BoundedCaptureLimits::new(
        stdout_bytes,
        stderr_bytes,
        timeout,
        Duration::from_millis(500),
        Duration::from_millis(5),
    )
}

fn shell(script: &str) -> BoundedProcessPrepared {
    let shell = Path::new("/bin/sh")
        .canonicalize()
        .expect("canonical test shell");
    let mut command = Command::new(shell);
    command.arg("-c").arg(script);
    BoundedProcessPrepared::new(command, process_limits(), "test bounded process")
        .expect("prepare test process")
}

#[test]
fn transfers_input_and_drains_both_output_streams() {
    let output = run_bounded_process(
        shell("read value; printf '%s' \"$value\"; printf err >&2"),
        BoundedProcessInput::Bytes(b"input\n".to_vec()),
        capture_limits(16, 16, Duration::from_secs(5)),
        BoundedCaptureBudget::new(32),
    )
    .expect("bounded duplex execution succeeds");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"input");
    assert_eq!(output.stderr, b"err");
}

#[test]
fn rejects_each_output_ceiling_and_the_shared_budget() {
    for (stream, redirect) in [
        (BoundedProcessStream::Stdout, ""),
        (BoundedProcessStream::Stderr, "1>&2"),
    ] {
        let error = run_bounded_process(
            shell(&format!("printf 12345 {redirect}")),
            BoundedProcessInput::Null,
            capture_limits(4, 4, Duration::from_secs(5)),
            BoundedCaptureBudget::new(16),
        )
        .expect_err("stream ceiling must reject");
        let expected = matches!(
            &error,
            BoundedProcessRunError::OutputOverflow { stream: actual, limit: 4 }
                if *actual == stream
        );
        let fail_closed_macos_cleanup =
            cfg!(target_os = "macos") && matches!(&error, BoundedProcessRunError::Cleanup(_));
        assert!(
            expected || fail_closed_macos_cleanup,
            "unexpected overflow error: {error:?}"
        );
    }

    let budget = BoundedCaptureBudget::new(7);
    let error = run_bounded_process(
        shell("printf 1234; printf 5678 >&2"),
        BoundedProcessInput::Null,
        capture_limits(4, 4, Duration::from_secs(5)),
        budget.clone(),
    )
    .expect_err("shared output budget must reject");
    let expected = matches!(
        &error,
        BoundedProcessRunError::AggregateOutputOverflow {
            ceiling: 7,
            attempted: 8
        }
    );
    let fail_closed_macos_cleanup =
        cfg!(target_os = "macos") && matches!(&error, BoundedProcessRunError::Cleanup(_));
    assert!(
        expected || fail_closed_macos_cleanup,
        "unexpected budget error: {error:?}"
    );
    assert!(budget.observed() <= budget.ceiling());
}

#[test]
fn timeout_terminates_the_owned_process_group() {
    let root = std::env::temp_dir().join(format!("omega-bounded-timeout-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create timeout test root");
    let marker = root.join("survived");
    let mut prepared = shell("(sleep 0.25; printf survived > \"$MARKER\") & exec sleep 10");
    prepared.env("MARKER", &marker);
    let started = Instant::now();
    let error = run_bounded_process(
        prepared,
        BoundedProcessInput::Null,
        capture_limits(16, 16, Duration::from_millis(50)),
        BoundedCaptureBudget::new(32),
    )
    .expect_err("deadline must reject");
    assert!(matches!(error, BoundedProcessRunError::TimedOut { .. }));
    assert!(started.elapsed() < Duration::from_secs(2));
    std::thread::sleep(Duration::from_millis(400));
    assert!(
        !marker.exists(),
        "descendant survived process-group cleanup"
    );
    std::fs::remove_dir_all(root).expect("remove timeout test root");
}

#[test]
fn completed_parent_closes_descendant_held_pipes() {
    let started = Instant::now();
    let output = run_bounded_process(
        shell("(sleep 10) &"),
        BoundedProcessInput::Null,
        capture_limits(16, 16, Duration::from_secs(2)),
        BoundedCaptureBudget::new(32),
    )
    .expect("natural parent exit still closes descendants");
    assert!(output.status.success());
    assert!(started.elapsed() < Duration::from_secs(1));
}
