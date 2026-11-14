// Process-wide compilation activity counters for review diagnostics and tests.
//
// These relaxed atomics measure requests and frontier reuse across the process,
// not semantic authority or per-invocation snapshots when compilations overlap.
// Executed (non-memoized) check counts live with Psi's repeated-check owner;
// comparing `settled_check_totals` against Psi's executed-check counter
// distinguishes fresh checks from memoized hits.

static CHECKED_COMPILATION_CALLS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[doc(hidden)]
pub fn checked_compilation_call_count() -> u64 {
    CHECKED_COMPILATION_CALLS.load(std::sync::atomic::Ordering::Relaxed)
}

static TYPED_FRONTIER_REUSES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SOURCE_ASSEMBLY_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static BUILD_EXECUTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SOURCE_BUILD_EXECUTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PROVIDER_DEFAULT_EXECUTIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy, Debug)]
pub enum BuildExecutionPurpose {
    Source,           // Original program or package companion, including its compiler default.
    ProviderDefaults, // Separate captured companion supplying provider defaults.
}

static PRODUCT_SOURCE_COMPLETIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
static TERMINAL_PRODUCTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PRODUCT_SOURCE_CHECKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[doc(hidden)]
pub fn product_source_check_count() -> u64 {
    PRODUCT_SOURCE_CHECKS.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn record_product_source_check(elapsed: std::time::Duration) {
    PRODUCT_SOURCE_CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    record_settled_check(elapsed);
}

#[doc(hidden)]
pub fn terminal_production_count() -> u64 {
    TERMINAL_PRODUCTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn record_terminal_production() {
    TERMINAL_PRODUCTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[doc(hidden)]
pub fn build_execution_count() -> u64 {
    BUILD_EXECUTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

#[doc(hidden)]
pub fn product_source_completion_count() -> u64 {
    PRODUCT_SOURCE_COMPLETIONS.load(std::sync::atomic::Ordering::Relaxed)
}

#[doc(hidden)]
pub fn source_build_execution_count() -> u64 {
    SOURCE_BUILD_EXECUTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

#[doc(hidden)]
pub fn provider_defaults_execution_count() -> u64 {
    PROVIDER_DEFAULT_EXECUTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn record_build_execution(purpose: BuildExecutionPurpose) {
    BUILD_EXECUTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let counter = match purpose {
        BuildExecutionPurpose::Source => &SOURCE_BUILD_EXECUTIONS,
        BuildExecutionPurpose::ProviderDefaults => &PROVIDER_DEFAULT_EXECUTIONS,
    };
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn record_product_source_completion() {
    PRODUCT_SOURCE_COMPLETIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[doc(hidden)]
pub fn source_assembly_invocation_count() -> u64 {
    SOURCE_ASSEMBLY_CALLS.load(std::sync::atomic::Ordering::Relaxed)
}

#[doc(hidden)]
pub fn typed_frontier_reuse_count() -> u64 {
    TYPED_FRONTIER_REUSES.load(std::sync::atomic::Ordering::Relaxed)
}

static SETTLED_CHECK_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SETTLED_CHECK_MICROS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[doc(hidden)]
pub fn settled_check_totals() -> (u64, u64) {
    (
        SETTLED_CHECK_CALLS.load(std::sync::atomic::Ordering::Relaxed),
        SETTLED_CHECK_MICROS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

pub fn record_settled_check(elapsed: std::time::Duration) {
    SETTLED_CHECK_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    SETTLED_CHECK_MICROS.fetch_add(
        u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
        std::sync::atomic::Ordering::Relaxed,
    );
}

pub fn record_checked_compilation() {
    CHECKED_COMPILATION_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn record_source_assembly() {
    SOURCE_ASSEMBLY_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

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
        let products = product_source_check_count();
        record_product_source_check(std::time::Duration::from_micros(7));
        assert_eq!(product_source_check_count(), products + 1);
        assert_eq!(settled_check_totals(), (settled.0 + 2, settled.1 + 18));
    }
}
