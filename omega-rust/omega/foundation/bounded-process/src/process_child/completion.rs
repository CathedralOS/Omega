use super::ProcessResourceUsage;
use std::process::ExitStatus;

/// Platform-neutral status used for ordinary caller control flow.
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

    pub(super) fn from_status(status: ExitStatus) -> Self {
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

/// Ordinary completion returned after the platform process container is closed and
/// the primary child is reaped. This is control-flow data, not evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedProcessCompletion {
    status: BoundedProcessExitStatus,
    resource_usage: Option<ProcessResourceUsage>,
}

impl BoundedProcessCompletion {
    pub(super) const fn new(
        status: BoundedProcessExitStatus,
        resource_usage: Option<ProcessResourceUsage>,
    ) -> Self {
        Self {
            status,
            resource_usage,
        }
    }

    pub const fn status(&self) -> BoundedProcessExitStatus {
        self.status
    }

    pub const fn resource_usage(&self) -> Option<&ProcessResourceUsage> {
        self.resource_usage.as_ref()
    }
}
