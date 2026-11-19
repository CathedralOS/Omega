//! Allocation publishes its settled decisions without retaining its program.
//!
//! These records are part of the current physical form: publication needs the
//! exact allocation manifest and build selections, not the allocator's graphs.
//! Retention grants no authority; an independent validator compares the whole
//! record with the original allocation, not merely its claimed identity.

use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use sha2::{Digest, Sha256};

use crate::PostAllocationOptimizationManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationPublicationFacts {
    record: PostAllocationOptimizationManifest,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
    identity: [u8; 32],
}

impl AllocationPublicationFacts {
    /// Retain decisions at their settled allocation owner. This does not
    /// revalidate output that owner just produced or grant admission authority.
    pub fn retain(
        record: PostAllocationOptimizationManifest,
        selections: OptimizationSelections,
        budget: OptimizationWorkBudget,
    ) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.allocation-publication-facts.v1\0");
        digest.update(record.identity.bytes());
        digest.update(selections.identity().bytes());
        digest.update(budget.encode());
        Self {
            record,
            selections,
            budget,
            identity: digest.finalize().into(),
        }
    }

    pub const fn record(&self) -> &PostAllocationOptimizationManifest {
        &self.record
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }

    pub const fn identity_bytes(&self) -> [u8; 32] {
        self.identity
    }
}
