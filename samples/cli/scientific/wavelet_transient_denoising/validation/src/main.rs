//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
//!
//! This validation reviewed bounded native execution: every published child ran
//! under the ceilings [`CAPS`] restates, and the validator's own outer timeout
//! closes the process group if preparation stalls before a child exists. Moving
//! here keeps those ceilings instead of widening them.
use sample_runner::{BoundedCaptureLimits, BoundedProcessLimits, Caps, Evidence, Sample};
use std::time::Duration;

/// The ceilings this validation's own runner already held each native child to:
/// ten CPU seconds, half a gigabyte of address space, one megabyte of file
/// output, sixty-four descriptors, eight processes, half a gigabyte of process
/// and of aggregate memory, sixty-four kilobytes of stdout, sixteen of stderr,
/// a fifteen second drain, two seconds of cleanup and an eighty kilobyte budget.
const CAPS: Caps = Caps {
    process: BoundedProcessLimits::new(
        10,
        512 * 1024 * 1024,
        1024 * 1024,
        64,
        8,
        512 * 1024 * 1024,
        512 * 1024 * 1024,
    ),
    capture: BoundedCaptureLimits::new(
        64 * 1024,
        16 * 1024,
        Duration::from_secs(15),
        Duration::from_secs(2),
        Duration::from_millis(10),
    ),
    budget_bytes: 80 * 1024,
    boundary: "wavelet numerical validation",
};

/// Wavelet transient denoising's roster entry for the shared capture harness.
const SAMPLE: Sample = Sample {
    package: "wavelet-transient-validation",
    review: false,
    caps: Some(CAPS),
    evidence: Evidence::Adjacent,
    frozen: None,
};

fn main() {
    sample_runner::main(SAMPLE);
}
