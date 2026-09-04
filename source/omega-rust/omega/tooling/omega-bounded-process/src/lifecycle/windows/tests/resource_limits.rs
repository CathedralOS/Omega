use std::time::Duration;

use super::super::lifecycle::JobLimitEvent;
use super::support::{JOB_LIMIT_TEST_LOCK, MIB, run_worker, test_limits};

#[test]
fn job_active_process_limit_rejects_excess_descendant() {
    let _serial = JOB_LIMIT_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let control = test_limits(4, 512, 1024, Duration::from_secs(30));
    let (status, events) = run_worker("fanout", "2,1", control, Duration::from_secs(10));
    assert!(status.success(), "below-limit fanout should succeed");
    assert!(!events.contains(&JobLimitEvent::ActiveProcess));

    let limited = test_limits(2, 512, 1024, Duration::from_secs(30));
    let (status, events) = run_worker("fanout", "2,0", limited, Duration::from_secs(10));
    assert!(
        status.success(),
        "worker should observe the rejected excess child"
    );
    assert!(events.contains(&JobLimitEvent::ActiveProcess));
}

#[test]
fn job_process_memory_limit_blocks_excess_commit() {
    let _serial = JOB_LIMIT_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let control = test_limits(4, 256, 512, Duration::from_secs(30));
    let (status, events) = run_worker(
        "touch",
        &format!("{},0", 32 * MIB),
        control,
        Duration::from_secs(10),
    );
    assert!(
        status.success(),
        "below-limit process memory should succeed"
    );
    assert!(!events.contains(&JobLimitEvent::ProcessMemory));

    let limited = test_limits(4, 128, 512, Duration::from_secs(30));
    let (status, events) = run_worker(
        "touch",
        &format!("{},0", 256 * MIB),
        limited,
        Duration::from_secs(10),
    );
    assert!(!status.success(), "over-limit process memory must fail");
    assert!(events.contains(&JobLimitEvent::ProcessMemory));
}

#[test]
fn job_aggregate_memory_limit_spans_descendants() {
    let _serial = JOB_LIMIT_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let control = test_limits(4, 256, 512, Duration::from_secs(30));
    let (status, events) = run_worker(
        "aggregate-memory",
        &format!("1,{},1", 32 * MIB),
        control,
        Duration::from_secs(10),
    );
    assert!(
        status.success(),
        "below-limit aggregate memory should succeed"
    );
    assert!(!events.contains(&JobLimitEvent::AggregateMemory));

    let limited = test_limits(4, 256, 256, Duration::from_secs(30));
    let (status, events) = run_worker(
        "aggregate-memory",
        &format!("2,{},0", 160 * MIB),
        limited,
        Duration::from_secs(15),
    );
    assert!(
        status.success(),
        "worker should observe an aggregate-memory child failure"
    );
    assert!(events.contains(&JobLimitEvent::AggregateMemory));
}

#[test]
fn job_aggregate_cpu_limit_terminates_the_job() {
    let _serial = JOB_LIMIT_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let control = test_limits(2, 256, 512, Duration::from_secs(5));
    let (status, events) = run_worker("spin", "100", control, Duration::from_secs(10));
    assert!(status.success(), "below-limit aggregate CPU should succeed");
    assert!(!events.contains(&JobLimitEvent::AggregateCpu));

    // Keep the CPU threshold well below the worker's wall-clock spin. On a
    // heavily loaded test host, one second of user time need not accrue during
    // five seconds of wall time.
    let limited = test_limits(2, 256, 512, Duration::from_millis(100));
    let (status, events) = run_worker("spin", "5000", limited, Duration::from_secs(10));
    assert!(!status.success(), "over-limit aggregate CPU must terminate");
    assert!(events.contains(&JobLimitEvent::AggregateCpu));
}
