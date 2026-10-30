use super::CoreContractDecodeError;

/// The input extent one budget's ceilings are calibrated for, in the units a
/// pass measures its input by — blocks, instructions, operands and writes. A
/// pass over a larger input receives the ceilings once per started reference
/// extent (`OptimizationWorkBudget::for_extent`).
pub const OPTIMIZATION_REFERENCE_EXTENT: u64 = 1 << 16;

/// Hard ceilings for one named pass group. All axes are explicit and nonzero;
/// exhaustion is a deterministic pass failure, never permission to publish a
/// partial candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationWorkBudget {
    pub(crate) rule_evaluations: u64,
    pub(crate) candidates: u64,
    pub(crate) validation_steps: u64,
    pub(crate) commits: u64,
    pub(crate) iterations: u64,
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

    /// The ceilings for one pass over an input of `extent` units — one
    /// function's or one program's blocks, instructions, operands and
    /// writes: every axis is granted once per started
    /// `OPTIMIZATION_REFERENCE_EXTENT` of the input, and an input within the
    /// reference extent keeps the ceilings unchanged. Work that is linear in
    /// the input and fits the ceilings at the reference extent fits them at
    /// every extent, while work superlinear in the input still exhausts them.
    /// A fixed ceiling on linear work instead capped the size of every
    /// program or function a required pass could process.
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
