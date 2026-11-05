//! Distinguish program/package Build starts from its core-default supplier.
//!
//! These probes read process-wide diagnostics during a serialized test sequence;
//! they are not invocation accounting or a coherent concurrent snapshot.

use artifacts::compile_timings::counters;

#[derive(Clone, Copy)]
pub struct BuildExecutionCounts {
    total: u64,
    source: u64,
    supplier: u64,
}

impl BuildExecutionCounts {
    pub fn capture() -> Self {
        Self {
            total: counters::build_execution_count(),
            source: counters::source_build_execution_count(),
            supplier: counters::provider_defaults_execution_count(),
        }
    }

    /// A cold one-source request prepares its separate core supplier once.
    pub fn provider_defaults_delta(self) -> u64 {
        counters::provider_defaults_execution_count() - self.supplier
    }

    /// A separate supplier start is not another execution of the source Build.
    /// Keep both actual deltas visible; no-replay checks still use total starts.
    pub fn source_build_delta(self) -> u64 {
        let current = Self::capture();
        let source = current.source - self.source;
        println!(
            "Build attempts: source={source} supplier={} total={}",
            current.supplier - self.supplier,
            current.total - self.total,
        );
        source
    }
}
