// `OptimizationWorkBudget`: 5-axis nonzero work ceilings calibrated per
// `OPTIMIZATION_REFERENCE_EXTENT` (`1 << 16` input units).
//
// `for_extent` scales every axis by `extent.div_ceil(OPTIMIZATION_REFERENCE_EXTENT).max(1)`
// so linear-time passes that fit within their calibrated budget at the
// reference extent scale linearly to arbitrarily large functions and programs,
// while superlinear behavior still deterministically exhausts the budget.

use super::CoreContractDecodeError;

pub const OPTIMIZATION_REFERENCE_EXTENT: u64 = 1 << 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationWorkBudget {
    pub(crate) rule_evaluations: u64, // Maximum rule predicate evaluations per reference extent.
    pub(crate) candidates: u64, // Maximum candidate rewrites constructed per reference extent.
    pub(crate) validation_steps: u64, // Maximum validator steps executed per reference extent.
    pub(crate) commits: u64,    // Maximum accepted rewrites committed per reference extent.
    pub(crate) iterations: u64, // Maximum pass fixpoint iterations per reference extent.
}

crate::record_views! {
    record OptimizationWorkBudget;
    copied {
        rule_evaluations: u64,
        candidates: u64,
        validation_steps: u64,
        commits: u64,
        iterations: u64,
    }
}

impl OptimizationWorkBudget {
    pub fn new(
        rule_evaluations: u64,
        candidates: u64,
        validation_steps: u64,
        commits: u64,
        iterations: u64,
    ) -> Result<Self, InvalidOptimizationWorkBudget> {
        let budget = Self {
            rule_evaluations,
            candidates,
            validation_steps,
            commits,
            iterations,
        };
        if [
            budget.rule_evaluations,
            budget.candidates,
            budget.validation_steps,
            budget.commits,
            budget.iterations,
        ]
        .contains(&0)
        {
            return Err(InvalidOptimizationWorkBudget);
        }
        Ok(budget)
    }

    pub fn for_extent(self, extent: u64) -> Self {
        let multiple = extent.div_ceil(OPTIMIZATION_REFERENCE_EXTENT).max(1);
        Self {
            rule_evaluations: self.rule_evaluations.saturating_mul(multiple),
            candidates: self.candidates.saturating_mul(multiple),
            validation_steps: self.validation_steps.saturating_mul(multiple),
            commits: self.commits.saturating_mul(multiple),
            iterations: self.iterations.saturating_mul(multiple),
        }
    }

    pub fn encode(self) -> [u8; 40] {
        crate::optimization_core::encode_work_counters([
            self.rule_evaluations,
            self.candidates,
            self.validation_steps,
            self.commits,
            self.iterations,
        ])
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        let Some(fields) = crate::optimization_core::decode_work_counters(encoded) else {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 40,
                actual: encoded.len(),
            });
        };
        Self::new(fields[0], fields[1], fields[2], fields[3], fields[4])
            .map_err(|_| CoreContractDecodeError::ZeroWorkBudget)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidOptimizationWorkBudget;

crate::fixed_error_display!(
    InvalidOptimizationWorkBudget,
    "every optimization work-budget axis must be nonzero"
);
