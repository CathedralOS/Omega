//! Work consumed along every bounded optimization axis.

use crate::OptimizationWorkBudget;

use super::OptimizationManifestDecodeError;

/// Actual work consumed by one pass. Zero is valid; publication separately
/// proves that every axis stayed within the selected nonzero budget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct OptimizationWorkUsage {
    pub rule_evaluations: u64,
    pub candidates: u64,
    pub validation_steps: u64,
    pub commits: u64,
    pub iterations: u64,
}

impl OptimizationWorkUsage {
    pub const fn within(self, budget: OptimizationWorkBudget) -> bool {
        self.rule_evaluations <= budget.rule_evaluations()
            && self.candidates <= budget.candidates()
            && self.validation_steps <= budget.validation_steps()
            && self.commits <= budget.commits()
            && self.iterations <= budget.iterations()
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

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizationManifestDecodeError> {
        let Some(fields) = crate::optimization_core::decode_work_counters(encoded) else {
            return Err(OptimizationManifestDecodeError::WrongLength {
                expected: 40,
                actual: encoded.len(),
            });
        };
        Ok(Self {
            rule_evaluations: fields[0],
            candidates: fields[1],
            validation_steps: fields[2],
            commits: fields[3],
            iterations: fields[4],
        })
    }
}
