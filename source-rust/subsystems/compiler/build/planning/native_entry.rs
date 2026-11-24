//! Sponsor policy for running an installed native entry under observation.
use bounded_process::{BoundedCaptureLimits, BoundedProcessLimits};
use std::time::Duration;

#[derive(Clone, Copy)]
pub struct NativeEntryLimits {
    pub process: BoundedProcessLimits,
    pub capture: BoundedCaptureLimits,
    pub combined_output_bytes: u64,
}

impl NativeEntryLimits {
    /// Each execution receives its own process container and output allowance.
    pub const INSTALLED: Self = Self {
        process: BoundedProcessLimits::new(
            60,
            1024 * 1024 * 1024,
            8 * 1024 * 1024,
            64,
            1,
            1024 * 1024 * 1024,
            1024 * 1024 * 1024,
        ),
        capture: BoundedCaptureLimits::new(
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            Duration::from_secs(60),
            Duration::from_secs(5),
            Duration::from_millis(10),
        ),
        combined_output_bytes: 8 * 1024 * 1024,
    };
}
