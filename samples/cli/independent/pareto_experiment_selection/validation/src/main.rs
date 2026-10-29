//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
use sample_runner::Sample;

/// Pareto experiment selection's roster entry for the shared capture harness.
const SAMPLE: Sample = Sample {
    package: "ind11-terminal-validation",
    review: false,
};

fn main() {
    sample_runner::main(SAMPLE);
}
