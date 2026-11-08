// Subtractive exact-name optimization rollback overlay (`OptimizationRollback`)
// and artifact-production settlement (`OptimizationRollbackSettlement`).
//
// Release rollback can only subtract exact optimization rules selected by
// `build.omg`; it can never add, alias, or reorder optimizations. `settle`
// binds the effective selection and optional `OptimizationRollbackReceipt`
// together so artifact production and reporting cannot diverge.

mod receipt;
mod request;

pub use receipt::OptimizationRollbackReceipt;
pub use request::OptimizationRollbackInputError;

use crate::OptimizationSelections;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationRollback {
    requested_disabled: OptimizationSelections, // Canonical set of exact rules requested to be disabled.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationRollbackSettlement {
    effective: OptimizationSelections, // Remaining build-selected rules after subtracting `requested_disabled`.
    receipt: Option<OptimizationRollbackReceipt>, // Present whenever `requested_disabled` was non-empty.
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

    pub fn settle(
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
