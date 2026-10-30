//! Capture actual numerical native and Terminal output from the same product.
//! This sample shares the independent capture harness and supplies its roster.
use sample_runner::{Evidence, Sample};

/// Hidden-markov inference's roster entry for the shared capture harness.
const SAMPLE: Sample = Sample {
    package: "hmm-terminal-validation",
    review: true,
    caps: None,
    evidence: Evidence::Terminal {
        sources: &[
            "main.omg",
            "build.omg",
            "domain/inference.omg",
            "domain/viterbi.omg",
            "io/trace.omg",
        ],
    },
    frozen: None,
};

fn main() {
    sample_runner::main(SAMPLE);
}
