//! Count allocation requests separately from the transparent-System timing run.
const COUNT_ALLOCATIONS: bool = true;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tools/benchmark/service-correspondence/harness.rs"
));
