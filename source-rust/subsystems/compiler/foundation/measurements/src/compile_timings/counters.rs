// Process-wide Build activation counters for regression tests.
//
// These relaxed atomics count Build activations across the process. They are
// not semantic authority or per-invocation snapshots when compilations overlap.

static SOURCE_BUILD_EXECUTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PROVIDER_DEFAULT_EXECUTIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy, Debug)]
pub enum BuildExecutionPurpose {
    Source,           // Original program or package companion, including its compiler default.
    ProviderDefaults, // Separate captured companion supplying provider defaults.
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
    let counter = match purpose {
        BuildExecutionPurpose::Source => &SOURCE_BUILD_EXECUTIONS,
        BuildExecutionPurpose::ProviderDefaults => &PROVIDER_DEFAULT_EXECUTIONS,
    };
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_purpose_counts_only_its_own_build_executions() {
        let sources = source_build_execution_count();
        let providers = provider_defaults_execution_count();
        record_build_execution(BuildExecutionPurpose::Source);
        assert!(source_build_execution_count() > sources);
        record_build_execution(BuildExecutionPurpose::ProviderDefaults);
        assert!(provider_defaults_execution_count() > providers);
    }
}
