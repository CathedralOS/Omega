use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::FixedPrecoloredSplitRequirementError;

pub(super) struct Work(OptimizationWorkUsage);

impl Work {
    pub(super) fn new() -> Self {
        Self(OptimizationWorkUsage {
            validation_steps: 1,
            iterations: 2,
            ..Default::default()
        })
    }

    pub(super) fn function(
        &mut self,
        tied_rows: usize,
        early_rows: usize,
        early_uses: usize,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        increment(&mut self.0.rule_evaluations, 1)?;
        increment(&mut self.0.validation_steps, 1)?;
        increment(&mut self.0.validation_steps, to_u64(tied_rows)?)?;
        increment(&mut self.0.validation_steps, to_u64(early_rows)?)?;
        increment(&mut self.0.validation_steps, to_u64(early_uses)?)?;
        increment(&mut self.0.iterations, 1)
    }

    pub(super) fn register(
        &mut self,
        fixed_rows: usize,
        connector_rows: usize,
        entry_transition_rows: usize,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        increment(&mut self.0.rule_evaluations, 1)?;
        increment(&mut self.0.validation_steps, 1)?;
        increment(&mut self.0.validation_steps, to_u64(fixed_rows)?)?;
        increment(&mut self.0.validation_steps, to_u64(connector_rows)?)?;
        increment(&mut self.0.validation_steps, to_u64(entry_transition_rows)?)?;
        increment(&mut self.0.iterations, 1)
    }

    pub(super) fn point(
        &mut self,
        candidate_views: usize,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        increment(&mut self.0.candidates, 1)?;
        increment(&mut self.0.validation_steps, 1)?;
        increment(&mut self.0.validation_steps, to_u64(candidate_views)?)?;
        increment(&mut self.0.iterations, 1)
    }

    pub(super) fn segment(&mut self) -> Result<(), FixedPrecoloredSplitRequirementError> {
        increment(&mut self.0.commits, 1)
    }

    pub(super) fn incompatible_boundary(
        &mut self,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        increment(&mut self.0.commits, 1)
    }

    pub(super) fn finish(
        self,
        budget: OptimizationWorkBudget,
    ) -> Result<OptimizationWorkUsage, FixedPrecoloredSplitRequirementError> {
        if self.0.within(budget) {
            Ok(self.0)
        } else {
            Err(FixedPrecoloredSplitRequirementError::BudgetExceeded {
                required: self.0,
                budget,
            })
        }
    }
}

fn to_u64(value: usize) -> Result<u64, FixedPrecoloredSplitRequirementError> {
    u64::try_from(value).map_err(|_| FixedPrecoloredSplitRequirementError::WorkOverflow)
}

fn increment(target: &mut u64, amount: u64) -> Result<(), FixedPrecoloredSplitRequirementError> {
    *target = target
        .checked_add(amount)
        .ok_or(FixedPrecoloredSplitRequirementError::WorkOverflow)?;
    Ok(())
}
