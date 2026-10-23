//! Process-wide compilation activity counters for review diagnostics and tests.
//!
//! These measure requests and reuse, not semantic authority or successful checks.
//! Recording sites own event boundaries. Totals use relaxed independent atomics:
//! a pair is not a coherent concurrent snapshot, and deltas are not per-invocation
//! accounting when compilations overlap. Executed (non-memoized) check counts
//! remain with Psi's repeated-check owner.

static CHECKED_COMPILATION_CALLS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Monotonic checked-request count for review-pass measurements.
#[doc(hidden)]
pub fn checked_compilation_call_count() -> u64 {
    CHECKED_COMPILATION_CALLS.load(std::sync::atomic::Ordering::Relaxed)
}

static TYPED_FRONTIER_REUSES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SOURCE_ASSEMBLY_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static BUILD_EXECUTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PRODUCT_SOURCE_COMPLETIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
static TERMINAL_PRODUCTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Product-adapter Psi production attempts, including independent callback roots.
#[doc(hidden)]
pub fn terminal_production_count() -> u64 {
    TERMINAL_PRODUCTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn record_terminal_production() {
    TERMINAL_PRODUCTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Actual Build machine execution attempts, excluding retained results.
#[doc(hidden)]
pub fn build_execution_count() -> u64 {
    BUILD_EXECUTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Product frontend completions, including generated-source incorporation.
#[doc(hidden)]
pub fn product_source_completion_count() -> u64 {
    PRODUCT_SOURCE_COMPLETIONS.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn record_build_execution() {
    BUILD_EXECUTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn record_product_source_completion() {
    PRODUCT_SOURCE_COMPLETIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Monotonic assemblies entered; a reused typed frontier does not assemble again.
#[doc(hidden)]
pub fn source_assembly_invocation_count() -> u64 {
    SOURCE_ASSEMBLY_CALLS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Monotonic exact frontend reuses across checked children.
#[doc(hidden)]
pub fn typed_frontier_reuse_count() -> u64 {
    TYPED_FRONTIER_REUSES.load(std::sync::atomic::Ordering::Relaxed)
}

static SETTLED_CHECK_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SETTLED_CHECK_MICROS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Monotonic settled-check `(calls, wall microseconds)` totals.
#[doc(hidden)]
pub fn settled_check_totals() -> (u64, u64) {
    (
        SETTLED_CHECK_CALLS.load(std::sync::atomic::Ordering::Relaxed),
        SETTLED_CHECK_MICROS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Account one settled checking call, whether it succeeded or rejected.
pub fn record_settled_check(elapsed: std::time::Duration) {
    SETTLED_CHECK_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    SETTLED_CHECK_MICROS.fetch_add(
        u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
        std::sync::atomic::Ordering::Relaxed,
    );
}

static PRELIMINARY_CHECK_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PRELIMINARY_CHECK_MICROS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Monotonic `(calls, wall microseconds)` totals of the pre-build
/// declaration-authority checking calls, including memo hits and rejections.
/// Compare with Psi's executed-check counts to distinguish fresh checks from
/// served requests; elapsed time alone does not establish memo reuse.
#[doc(hidden)]
pub fn preliminary_check_totals() -> (u64, u64) {
    (
        PRELIMINARY_CHECK_CALLS.load(std::sync::atomic::Ordering::Relaxed),
        PRELIMINARY_CHECK_MICROS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Account one pre-build checking request, whether it checked, took a retained
/// judgment, or rejected. Elapsed covers the whole lowering request so review
/// accounting can weigh requests against executed checks.
pub fn record_preliminary_check(elapsed: std::time::Duration) {
    PRELIMINARY_CHECK_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    PRELIMINARY_CHECK_MICROS.fetch_add(
        u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
        std::sync::atomic::Ordering::Relaxed,
    );
}
/// Record entry to the checked-compilation API, including rejected requests.
pub fn record_checked_compilation() {
    CHECKED_COMPILATION_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Record an assembly attempt; exact typed-frontier reuse does not assemble.
pub fn record_source_assembly() {
    SOURCE_ASSEMBLY_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Record one retained typed frontier consumed by a checked child.
pub fn record_typed_frontier_reuse() {
    TYPED_FRONTIER_REUSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_retain_separate_request_reuse_and_check_totals() {
        let requests = checked_compilation_call_count();
        let assemblies = source_assembly_invocation_count();
        let reuses = typed_frontier_reuse_count();
        let settled = settled_check_totals();
        record_checked_compilation();
        record_source_assembly();
        record_typed_frontier_reuse();
        record_settled_check(std::time::Duration::from_micros(11));
        assert_eq!(checked_compilation_call_count(), requests + 1);
        assert_eq!(source_assembly_invocation_count(), assemblies + 1);
        assert_eq!(typed_frontier_reuse_count(), reuses + 1);
        assert_eq!(settled_check_totals(), (settled.0 + 1, settled.1 + 11));
    }
}
