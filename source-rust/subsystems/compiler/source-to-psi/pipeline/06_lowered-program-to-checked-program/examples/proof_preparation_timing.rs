//! Replay real compiler contexts through private proof preparation owners.
#[path = "../src/known.rs"]
#[allow(dead_code)]
mod known;
#[path = "../src/prove.rs"]
#[allow(dead_code)]
mod prove;

// Match the private implementation namespace without importing another producer.
mod checking {
    pub(crate) use super::known;
}

const COUNT_ALLOCATIONS: bool = false;
include!("../../../../../../../tools/benchmark/proof-preparation/harness.rs");
