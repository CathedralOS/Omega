use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::FixedPrecoloredSplitRequirementError;

pub(super) struct Work {
    usage: OptimizationWorkUsage,
}

impl Work {
    pub(super) fn new() -> Self {
        Self {
            usage: OptimizationWorkUsage {
                validation_steps: 1,
                iterations: 2,
                ..Default::default()
            },
        }
    }

    pub(super) fn function(
        &mut self,
        tied_rows: usize,
        early_rows: usize,
        early_uses: usize,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        add(&mut self.usage.rule_evaluations, 1)?;
        add(&mut self.usage.validation_steps, 1)?;
        add(&mut self.usage.validation_steps, count(tied_rows)?)?;
        add(&mut self.usage.validation_steps, count(early_rows)?)?;
        add(&mut self.usage.validation_steps, count(early_uses)?)?;
        add(&mut self.usage.iterations, 1)
    }

    pub(super) fn register(
        &mut self,
        fixed_rows: usize,
        connector_rows: usize,
        entry_transition_rows: usize,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        add(&mut self.usage.rule_evaluations, 1)?;
        add(&mut self.usage.validation_steps, 1)?;
        add(&mut self.usage.validation_steps, count(fixed_rows)?)?;
        add(&mut self.usage.validation_steps, count(connector_rows)?)?;
        add(
            &mut self.usage.validation_steps,
            count(entry_transition_rows)?,
        )?;
        add(&mut self.usage.iterations, 1)
    }

    pub(super) fn point(
        &mut self,
        candidate_views: usize,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        add(&mut self.usage.candidates, 1)?;
        add(&mut self.usage.validation_steps, 1)?;
        add(&mut self.usage.validation_steps, count(candidate_views)?)?;
        add(&mut self.usage.iterations, 1)
    }

    pub(super) fn segment(&mut self) -> Result<(), FixedPrecoloredSplitRequirementError> {
        add(&mut self.usage.commits, 1)
    }

    pub(super) fn incompatible_boundary(
        &mut self,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        add(&mut self.usage.commits, 1)
    }

    pub(super) fn finish(
        self,
        budget: OptimizationWorkBudget,
    ) -> Result<OptimizationWorkUsage, FixedPrecoloredSplitRequirementError> {
        if !self.usage.within(budget) {
            return Err(FixedPrecoloredSplitRequirementError::BudgetExceeded {
                required: self.usage,
                budget,
            });
        }
        Ok(self.usage)
    }
}

fn count(value: usize) -> Result<u64, FixedPrecoloredSplitRequirementError> {
    u64::try_from(value).map_err(|_| FixedPrecoloredSplitRequirementError::WorkOverflow)
}

fn add(target: &mut u64, amount: u64) -> Result<(), FixedPrecoloredSplitRequirementError> {
    *target = target
        .checked_add(amount)
        .ok_or(FixedPrecoloredSplitRequirementError::WorkOverflow)?;
    Ok(())
}
