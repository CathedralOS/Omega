//! Time source correspondence without allocator counter updates in the loop.
const COUNT_ALLOCATIONS: bool = false;
include!("../../../../../../../tools/benchmark/service-correspondence/harness.rs");

#[cfg(test)]
include!("../../../../../../../tools/benchmark/service-correspondence/tests.rs");
