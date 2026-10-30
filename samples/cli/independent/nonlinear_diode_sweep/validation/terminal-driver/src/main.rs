//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
use sample_runner::{Evidence, Sample};

/// Nonlinear diode sweep's roster entry for the shared capture harness.
const SAMPLE: Sample = Sample {
    package: "new031-terminal-validation",
    review: true,
    evidence: Evidence::Terminal {
        sources: &[
            "main.omg",
            "build.omg",
            "domain/shockley.omg",
            "domain/solver.omg",
            "io/protocol.omg",
        ],
    },
};

fn main() {
    sample_runner::main(SAMPLE);
}
