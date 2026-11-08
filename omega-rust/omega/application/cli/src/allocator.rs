//! The process allocator. Checking allocates and frees many small tree and
//! fact nodes, and the macOS system allocator's tiny-zone paths dominated
//! package commands' profiles, so every build uses mimalloc.
//!
//! The `allocation-observation` feature wraps mimalloc to count allocations
//! and writes the count to the `OMEGA_ALLOCATION_PROFILE` path when `main`
//! returns. Ordinary builds compile `ReportOnExit` to nothing.

#[cfg(not(feature = "allocation-observation"))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "allocation-observation")]
#[global_allocator]
static ALLOCATOR: allocation_observation::ObservedAllocator<mimalloc::MiMalloc> =
    allocation_observation::ObservedAllocator::new(mimalloc::MiMalloc);

/// `+allocation-observation` in an observing build, so `omega --version`
/// never passes an instrumented binary off as an ordinary one.
pub(crate) const VERSION_SUFFIX: &str = if cfg!(feature = "allocation-observation") {
    "+allocation-observation"
} else {
    ""
};

/// Held for the whole of `main`; an observing build writes its report on drop.
pub(crate) struct ReportOnExit;

#[cfg(feature = "allocation-observation")]
impl Drop for ReportOnExit {
    fn drop(&mut self) {
        if let Some(path) = std::env::var_os("OMEGA_ALLOCATION_PROFILE")
            && let Err(error) = ALLOCATOR.write_report(std::path::Path::new(&path))
        {
            eprintln!("cannot write allocation observation: {error}");
        }
    }
}
