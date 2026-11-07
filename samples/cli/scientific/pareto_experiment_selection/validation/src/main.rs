//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
use sample_runner::{Cases, Evidence, FrozenCases, Sample};

/// Pareto experiment selection's roster entry for the shared capture harness.
///
/// Every `data/*.stdin` case is compared against `expected/<case>.stdout` and
/// its frozen exit, exactly as this sample's private runner did before it
/// shared the harness: `--evidence` fails on the first wrong expectation
/// instead of recording it.
const SAMPLE: Sample = Sample {
    package: "ind11-terminal-validation",
    review: false,
    caps: None,
    evidence: Evidence::Adjacent,
    frozen: Some(FrozenCases {
        directory: "build/validation-terminal",
        fuel: 10_000_000,
        cases: Cases::ExpectedFiles,
    }),
};

fn main() {
    sample_runner::main(SAMPLE);
}
