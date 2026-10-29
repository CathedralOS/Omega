//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
use sample_runner::{Evidence, Sample};

/// Thin-film reflectometry's roster entry for the shared capture harness.
const SAMPLE: Sample = Sample {
    package: "thin-film-reflectometry-validation",
    review: true,
    evidence: Evidence::Adjacent,
};

fn main() {
    sample_runner::main(SAMPLE);
}
