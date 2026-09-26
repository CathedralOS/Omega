use super::CoreContractDecodeError;
use std::fmt;

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

    pub const fn rule_evaluations(self) -> u64 {
        self.rule_evaluations
    }

    pub const fn candidates(self) -> u64 {
        self.candidates
    }

    pub const fn validation_steps(self) -> u64 {
        self.validation_steps
    }

    pub const fn commits(self) -> u64 {
        self.commits
    }

    pub const fn iterations(self) -> u64 {
        self.iterations
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        if encoded.len() != 40 {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 40,
                actual: encoded.len(),
            });
        }
        let value = |index: usize| {
            u64::from_le_bytes(
                encoded[index * 8..index * 8 + 8]
                    .try_into()
                    .expect("checked work-budget width"),
            )
        };
        Self::new(value(0), value(1), value(2), value(3), value(4))
            .map_err(|_| CoreContractDecodeError::ZeroWorkBudget)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidOptimizationWorkBudget;

impl fmt::Display for InvalidOptimizationWorkBudget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("every optimization work-budget axis must be nonzero")
    }
}

impl std::error::Error for InvalidOptimizationWorkBudget {}
