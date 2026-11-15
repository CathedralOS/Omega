//! Replay real compiler contexts through private proof preparation owners.
#[path = "../src/known.rs"]
#[allow(dead_code)]
mod known;
#[path = "../src/prove.rs"]
#[allow(dead_code)]
mod prove;

const COUNT_ALLOCATIONS: bool = false;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tools/benchmark/proof-preparation/harness.rs"
));
