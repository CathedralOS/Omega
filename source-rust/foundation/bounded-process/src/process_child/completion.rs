// Platform-neutral exit status (`BoundedProcessExitStatus`) and completion
// record (`BoundedProcessCompletion`) returned after a bounded process
// container is closed and reaped.

use super::ProcessResourceUsage;
use std::process::ExitStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedProcessExitStatus {
    success: bool,
    code: Option<i32>,
    unix_signal: Option<i32>,
}

impl BoundedProcessExitStatus {
    pub const fn success(&self) -> bool {
        self.success
    }

    pub const fn code(&self) -> Option<i32> {
        self.code
    }

    pub const fn unix_signal(&self) -> Option<i32> {
        self.unix_signal
    }
}

impl From<ExitStatus> for BoundedProcessExitStatus {
    fn from(status: ExitStatus) -> Self {
        #[cfg(unix)]
        let unix_signal = {
            use std::os::unix::process::ExitStatusExt;
            status.signal()
        };
        #[cfg(not(unix))]
        let unix_signal = None;
        Self {
            success: status.success(),
            code: status.code(),
            unix_signal,
        }
    }
}

/// Resource ceilings whose enforcement the host explicitly reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessLimit {
    ActiveProcess,
    ProcessMemory,
    AggregateMemory,
    AggregateCpu,
}

impl std::fmt::Display for ProcessLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ActiveProcess => "active process count",
            Self::ProcessMemory => "per-process memory",
            Self::AggregateMemory => "aggregate memory",
            Self::AggregateCpu => "aggregate CPU time",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedProcessCompletion {
    status: BoundedProcessExitStatus,
    resource_usage: Option<ProcessResourceUsage>,
    limits_reached: Vec<ProcessLimit>,
}

impl BoundedProcessCompletion {
    pub(super) const fn new(
        status: BoundedProcessExitStatus,
        resource_usage: Option<ProcessResourceUsage>,
        limits_reached: Vec<ProcessLimit>,
    ) -> Self {
        Self {
            status,
            resource_usage,
            limits_reached,
        }
    }

    pub const fn status(&self) -> BoundedProcessExitStatus {
        self.status
    }

    pub fn limits_reached(&self) -> &[ProcessLimit] {
        &self.limits_reached
    }

    pub const fn resource_usage(&self) -> Option<&ProcessResourceUsage> {
        self.resource_usage.as_ref()
    }
}
