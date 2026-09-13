//! Current target-program ownership, separate from physical-stage evidence.

use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use std::sync::Arc;
use target_operations::TargetOperationPlanWithNativeCallbacks;

/// Every completed target stage owns the same current representation.
/// Optimized translation evidence shares that original allocation rather than
/// storing a second copy or supplying the program through a history accessor.
#[derive(Debug)]
pub(crate) struct NativeTargetStageResult {
    program: Arc<TargetOperationPlanWithNativeCallbacks>,
    evidence: ValidatedOptimizedTargetOperations,
}

impl NativeTargetStageResult {
    pub(super) fn new(evidence: ValidatedOptimizedTargetOperations) -> Self {
        Self {
            program: evidence.shared_program(),
            evidence,
        }
    }

    /// Return current data and its evidence only after checking their full join.
    /// Compact Terminal, entry, and target IDs do not establish plan equality.
    pub(crate) fn into_parts(
        self,
    ) -> Result<
        (
            Arc<TargetOperationPlanWithNativeCallbacks>,
            ValidatedOptimizedTargetOperations,
        ),
        &'static str,
    > {
        if self.program != self.evidence.shared_program() {
            return Err("current target program differs from its retained translation evidence");
        }
        Ok((self.program, self.evidence))
    }
}

#[cfg(test)]
mod tests;
