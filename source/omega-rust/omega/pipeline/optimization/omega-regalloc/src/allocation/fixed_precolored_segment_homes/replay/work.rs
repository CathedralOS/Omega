use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::FixedPrecoloredSegmentHomeError;

pub(super) struct Work {
    usage: OptimizationWorkUsage,
}

impl Work {
    pub(super) fn new() -> Self {
        Self {
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                validation_steps: 1,
                commits: 1,
                iterations: 1,
                ..Default::default()
            },
        }
    }

    pub(super) fn function(&mut self) -> Result<(), FixedPrecoloredSegmentHomeError> {
        self.event(1, 0, 1, 0, 1)
    }
    pub(super) fn register(&mut self) -> Result<(), FixedPrecoloredSegmentHomeError> {
        self.event(1, 0, 1, 0, 1)
    }
    pub(super) fn segment(&mut self) -> Result<(), FixedPrecoloredSegmentHomeError> {
        self.event(0, 0, 1, 1, 1)
    }
    pub(super) fn domain(&mut self) -> Result<(), FixedPrecoloredSegmentHomeError> {
        self.event(1, 0, 1, 1, 1)
    }
    pub(super) fn pair(&mut self) -> Result<(), FixedPrecoloredSegmentHomeError> {
        self.event(0, 0, 1, 0, 1)
    }
    pub(super) fn candidate_pair(&mut self) -> Result<(), FixedPrecoloredSegmentHomeError> {
        self.event(0, 1, 1, 0, 0)
    }
    pub(super) fn viability_probe(&mut self) -> Result<(), FixedPrecoloredSegmentHomeError> {
        self.event(0, 1, 1, 0, 1)
    }

    pub(super) fn finish(
        self,
        budget: OptimizationWorkBudget,
    ) -> Result<OptimizationWorkUsage, FixedPrecoloredSegmentHomeError> {
        if !self.usage.within(budget) {
            return Err(FixedPrecoloredSegmentHomeError::BudgetExceeded {
                required: self.usage,
                budget,
            });
        }
        Ok(self.usage)
    }

    fn event(
        &mut self,
        rules: u64,
        candidates: u64,
        validation: u64,
        commits: u64,
        iterations: u64,
    ) -> Result<(), FixedPrecoloredSegmentHomeError> {
        add(&mut self.usage.rule_evaluations, rules)?;
        add(&mut self.usage.candidates, candidates)?;
        add(&mut self.usage.validation_steps, validation)?;
        add(&mut self.usage.commits, commits)?;
        add(&mut self.usage.iterations, iterations)
    }
}

fn add(target: &mut u64, amount: u64) -> Result<(), FixedPrecoloredSegmentHomeError> {
    *target = target
        .checked_add(amount)
        .ok_or(FixedPrecoloredSegmentHomeError::WorkOverflow)?;
    Ok(())
}
