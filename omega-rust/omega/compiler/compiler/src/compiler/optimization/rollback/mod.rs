//! Optimizer module role: executable entrance. Exact-name release rollback settlement.

mod request;

pub use request::OptimizationRollbackInputError;

use optimization_core::OptimizationSelections;

use crate::compiler::OptimizationRollbackReceipt;

/// A release-tooling overlay that can only subtract exact rules selected by
/// `build.omg`. It cannot add, alias, or reorder an optimization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationRollback {
    requested_disabled: OptimizationSelections,
}

/// Complete artifact-production view of one release rollback request.
///
/// The effective selection and optional report receipt are constructed
/// together so callers cannot reimplement the empty-request identity case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OptimizationRollbackSettlement {
    effective: OptimizationSelections,
    receipt: Option<OptimizationRollbackReceipt>,
}

impl OptimizationRollbackSettlement {
    pub const fn effective(&self) -> &OptimizationSelections {
        &self.effective
    }

    pub fn into_receipt(self) -> Option<OptimizationRollbackReceipt> {
        self.receipt
    }
}

impl OptimizationRollback {
    /// Reconstruct the optional public report receipt for compatibility with
    /// callers that inspect rollback evidence independently. Artifact
    /// production consumes [`Self::settle`] so effective selection cannot
    /// detach from this receipt.
    pub fn reconcile(
        &self,
        build_selected: &OptimizationSelections,
    ) -> Option<OptimizationRollbackReceipt> {
        (!self.is_empty()).then(|| {
            OptimizationRollbackReceipt::new(
                build_selected.clone(),
                self.requested_disabled.clone(),
            )
        })
    }

    pub(crate) fn settle(
        &self,
        build_selected: &OptimizationSelections,
    ) -> OptimizationRollbackSettlement {
        let receipt = self.reconcile(build_selected);
        let effective = receipt.as_ref().map_or_else(
            || build_selected.clone(),
            |receipt| receipt.effective().clone(),
        );
        OptimizationRollbackSettlement { effective, receipt }
    }
}

#[cfg(test)]
mod tests;
