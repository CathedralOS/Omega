// Uses the real public Build/target selection driver and original MiMalloc.

#[global_allocator]
static ALLOCATOR: selected_body_allocation_trace::Tracking<mimalloc::MiMalloc> =
    selected_body_allocation_trace::Tracking(mimalloc::MiMalloc);

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../build-system/orchestration/examples/package_batch.rs"
));
