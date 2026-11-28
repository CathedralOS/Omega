//! Allocation publishes its settled decisions without retaining its program.
//!
//! These records are part of the current physical form: publication needs the
//! exact allocation manifest and build selections, not the allocator's graphs.
//! Retention grants no authority; an independent validator compares the whole
//! record with the original allocation.

use std::sync::Arc;

use optimization_core::{OptimizationSelections, OptimizationWorkBudget};

use crate::PostAllocationOptimizationManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationPublicationFacts {
    record: Arc<PostAllocationOptimizationManifest>,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
}

impl AllocationPublicationFacts {
    /// Retain decisions at their settled allocation owner. This does not
    /// revalidate output that owner just produced or grant admission authority.
    pub fn retain(
        record: Arc<PostAllocationOptimizationManifest>,
        selections: OptimizationSelections,
        budget: OptimizationWorkBudget,
    ) -> Self {
        Self {
            record,
            selections,
            budget,
        }
    }

    pub const fn record(&self) -> &Arc<PostAllocationOptimizationManifest> {
        &self.record
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }
}
