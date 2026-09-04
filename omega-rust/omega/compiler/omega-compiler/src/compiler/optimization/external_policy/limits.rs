use std::time::Duration;

use omega_bounded_process::BoundedCaptureLimits;

use super::ExternalPolicyExecutionFailure;

/// Exact transport limits for one external-policy exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExternalPolicyExecutionLimits {
    request_bytes: usize,
    response_bytes: usize,
    stderr_bytes: usize,
    captured_output_bytes: u64,
    timeout: Duration,
    cleanup_timeout: Duration,
    poll_interval: Duration,
}

impl ExternalPolicyExecutionLimits {
    pub(crate) const fn new(
        request_bytes: usize,
        response_bytes: usize,
        stderr_bytes: usize,
        captured_output_bytes: u64,
        timeout: Duration,
        cleanup_timeout: Duration,
        poll_interval: Duration,
    ) -> Self {
        Self {
            request_bytes,
            response_bytes,
            stderr_bytes,
            captured_output_bytes,
            timeout,
            cleanup_timeout,
            poll_interval,
        }
    }

    pub(super) fn validate(self) -> Result<(), ExternalPolicyExecutionFailure> {
        let maximum_captured_output =
            u64::try_from(self.response_bytes)
                .ok()
                .and_then(|response| {
                    u64::try_from(self.stderr_bytes)
                        .ok()
                        .and_then(|stderr| response.checked_add(stderr))
                });
        if self.request_bytes == 0
            || self.response_bytes == 0
            || self.stderr_bytes == 0
            || self.captured_output_bytes == 0
            || maximum_captured_output.is_none_or(|maximum| self.captured_output_bytes > maximum)
            || self.timeout.is_zero()
            || self.cleanup_timeout.is_zero()
            || self.poll_interval.is_zero()
        {
            return Err(ExternalPolicyExecutionFailure::InvalidLimits);
        }
        Ok(())
    }

    pub(super) const fn request_bytes(self) -> usize {
        self.request_bytes
    }

    pub(super) const fn captured_output_bytes(self) -> u64 {
        self.captured_output_bytes
    }

    pub(super) const fn capture_limits(self) -> BoundedCaptureLimits {
        BoundedCaptureLimits::new(
            self.response_bytes,
            self.stderr_bytes,
            self.timeout,
            self.cleanup_timeout,
            self.poll_interval,
        )
    }
}
