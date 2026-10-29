//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
use sample_runner::Sample;

/// Conservative advection's roster entry for the shared capture harness.
const SAMPLE: Sample = Sample {
    package: "advection-terminal-validation",
    review: true,
};

fn main() {
    sample_runner::main(SAMPLE);
}
