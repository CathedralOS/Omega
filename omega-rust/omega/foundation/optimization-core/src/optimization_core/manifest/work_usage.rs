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
        let mut encoded = [0; 40];
        for (index, value) in [
            self.rule_evaluations,
            self.candidates,
            self.validation_steps,
            self.commits,
            self.iterations,
        ]
        .into_iter()
        .enumerate()
        {
            encoded[index * 8..index * 8 + 8].copy_from_slice(&value.to_le_bytes());
        }
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizationManifestDecodeError> {
        if encoded.len() != 40 {
            return Err(OptimizationManifestDecodeError::WrongLength {
                expected: 40,
                actual: encoded.len(),
            });
        }
        let value = |index: usize| {
            u64::from_le_bytes(
                encoded[index * 8..index * 8 + 8]
                    .try_into()
                    .expect("checked work-usage width"),
            )
        };
        Ok(Self {
            rule_evaluations: value(0),
            candidates: value(1),
            validation_steps: value(2),
            commits: value(3),
            iterations: value(4),
        })
    }
}
