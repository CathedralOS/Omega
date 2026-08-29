use std::io;

use crate::process::limits::{
    CHILD_AGGREGATE_CPU_SECONDS, CHILD_AGGREGATE_MEMORY_BYTES, CHILD_PROCESS_LIMIT,
    CHILD_PROCESS_MEMORY_BYTES,
};

const WINDOWS_TIME_TICKS_PER_SECOND: u64 = 10_000_000;

#[derive(Debug, Clone, Copy)]
pub(super) struct WindowsJobLimits {
    pub(super) active_processes: u64,
    pub(super) process_memory_bytes: u64,
    pub(super) aggregate_memory_bytes: u64,
    pub(super) aggregate_cpu_ticks: u64,
}

impl WindowsJobLimits {
    pub(super) fn production() -> io::Result<Self> {
        let aggregate_cpu_ticks = CHILD_AGGREGATE_CPU_SECONDS
            .checked_mul(WINDOWS_TIME_TICKS_PER_SECOND)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the Windows aggregate CPU limit exceeds the API field",
                )
            })?;
        Ok(Self {
            active_processes: CHILD_PROCESS_LIMIT,
            process_memory_bytes: CHILD_PROCESS_MEMORY_BYTES,
            aggregate_memory_bytes: CHILD_AGGREGATE_MEMORY_BYTES,
            aggregate_cpu_ticks,
        })
    }
}
