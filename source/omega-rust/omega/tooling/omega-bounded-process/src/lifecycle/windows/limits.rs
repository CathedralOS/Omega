use std::io;

use crate::BoundedProcessLimits;

const WINDOWS_TIME_TICKS_PER_SECOND: u64 = 10_000_000;

#[derive(Debug, Clone, Copy)]
pub(super) struct WindowsJobLimits {
    pub(super) active_processes: u64,
    pub(super) process_memory_bytes: u64,
    pub(super) aggregate_memory_bytes: u64,
    pub(super) aggregate_cpu_ticks: u64,
}

impl WindowsJobLimits {
    pub(super) fn from_limits(limits: BoundedProcessLimits) -> io::Result<Self> {
        let aggregate_cpu_ticks = limits
            .cpu_seconds
            .checked_mul(WINDOWS_TIME_TICKS_PER_SECOND)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the Windows aggregate CPU limit exceeds the API field",
                )
            })?;
        Ok(Self {
            active_processes: limits.active_processes,
            process_memory_bytes: limits.process_memory_bytes,
            aggregate_memory_bytes: limits.aggregate_memory_bytes,
            aggregate_cpu_ticks,
        })
    }
}
