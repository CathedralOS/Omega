//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
use sample_runner::{Cases, Evidence, FrozenCases, Sample};

/// Spatial diffusion's roster entry for the shared capture harness.
///
/// Plain capture writes beside each named input; `--evidence` instead replays
/// the frozen oracle matrix this validation has always compared. Each case must
/// meet its frozen exit and leave stderr empty, while the oracle's numerical
/// stdout stays with `validate.py`.
const SAMPLE: Sample = Sample {
    package: "sci04-terminal-validation",
    review: false,
    caps: None,
    evidence: Evidence::Adjacent,
    frozen: Some(FrozenCases {
        directory: "build/validation/terminal",
        fuel: 100_000_000,
        cases: Cases::OracleRows,
    }),
};

fn main() {
    sample_runner::main(SAMPLE);
}
