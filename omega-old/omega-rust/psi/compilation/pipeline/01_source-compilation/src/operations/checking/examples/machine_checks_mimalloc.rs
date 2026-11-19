//! Match the ordinary compiler's allocator when measuring independent checks.
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tools/benchmark/machine-checks/harness.rs"
));
