//! Time source correspondence without allocator counter updates in the loop.
const COUNT_ALLOCATIONS: bool = false;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tools/benchmark/service-correspondence/harness.rs"
));

#[cfg(test)]
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tools/benchmark/service-correspondence/tests.rs"
));
