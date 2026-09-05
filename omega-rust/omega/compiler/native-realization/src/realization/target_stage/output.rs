//! Current target-program ownership, separate from physical-stage evidence.

use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use std::sync::Arc;
use target_operations::{TargetOperationPlan, TargetOperationPlanWithNativeCallbacks};

/// Native authority roles, not optimization-history roles. Physical convergence
/// must preserve callback admissions and independently checked ranked evidence.
#[derive(Debug)]
pub(crate) enum NativeTargetStageEvidence {
    Ordinary(Box<ValidatedOptimizedTargetOperations>),
    Ranked,
}

/// Every completed target stage owns the same current representation.
/// Optimized translation evidence shares that original allocation rather than
/// storing a second copy or supplying the program through a history accessor.
#[derive(Debug)]
pub(crate) struct NativeTargetStageResult {
    program: Arc<TargetOperationPlanWithNativeCallbacks>,
    evidence: NativeTargetStageEvidence,
}

impl NativeTargetStageResult {
    pub(super) fn ranked(plan: TargetOperationPlan) -> Self {
        Self {
            program: Arc::new(TargetOperationPlanWithNativeCallbacks {
                plan,
                native_callback_arguments: Vec::new(),
            }),
            evidence: NativeTargetStageEvidence::Ranked,
        }
    }

    pub(super) fn ordinary(evidence: ValidatedOptimizedTargetOperations) -> Self {
        Self {
            program: evidence.shared_program(),
            evidence: NativeTargetStageEvidence::Ordinary(Box::new(evidence)),
        }
    }

    /// Return current data and its evidence only after checking their full join.
    /// Compact Terminal, entry, and target IDs do not establish plan equality.
    pub(crate) fn into_parts(
        self,
    ) -> Result<
        (
            Arc<TargetOperationPlanWithNativeCallbacks>,
            NativeTargetStageEvidence,
        ),
        &'static str,
    > {
        match &self.evidence {
            NativeTargetStageEvidence::Ordinary(evidence) => {
                if self.program != evidence.shared_program() {
                    return Err(
                        "current target program differs from its retained translation evidence",
                    );
                }
            }
            NativeTargetStageEvidence::Ranked => {
                if !self.program.native_callback_arguments.is_empty() {
                    return Err("ranked target authority cannot carry native callback arguments");
                }
            }
        }
        Ok((self.program, self.evidence))
    }
}

#[cfg(test)]
mod tests;
