use std::process::ExitStatus;

/// Platform-neutral status used for ordinary resolver control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolverExecutionExitStatus {
    success: bool,
    code: Option<i32>,
    unix_signal: Option<i32>,
}

impl ResolverExecutionExitStatus {
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
pub struct ResolverExecutionCompletion {
    status: ResolverExecutionExitStatus,
}

impl ResolverExecutionCompletion {
    pub(super) const fn new(status: ResolverExecutionExitStatus) -> Self {
        Self { status }
    }

    pub const fn status(&self) -> ResolverExecutionExitStatus {
        self.status
    }
}
