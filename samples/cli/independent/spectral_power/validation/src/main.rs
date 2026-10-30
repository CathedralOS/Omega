//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
use sample_runner::{Evidence, Sample};

/// Spectral power's roster entry for the shared capture harness.
const SAMPLE: Sample = Sample {
    package: "spectral-power-validation",
    review: false,
    evidence: Evidence::Adjacent,
};

fn main() {
    sample_runner::main(SAMPLE);
}
