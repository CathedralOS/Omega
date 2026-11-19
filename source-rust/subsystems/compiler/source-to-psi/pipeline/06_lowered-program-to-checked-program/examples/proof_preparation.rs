//! Replay real compiler contexts through private proof preparation owners.
#[path = "../src/known.rs"]
#[allow(dead_code)]
mod known;
#[path = "../src/prove.rs"]
#[allow(dead_code)]
mod prove;

// The measured implementation now belongs to source settlement. Keep this
// isolated benchmark's local proof tables in the same private namespace.
mod checking {
    pub(crate) use super::known;
}

const COUNT_ALLOCATIONS: bool = true;
include!("../../../../../../../tools/benchmark/proof-preparation/harness.rs");
