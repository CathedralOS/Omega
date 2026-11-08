/*
  Bounded optimization work-usage accounting (`WorkDefect`, `charge_step`,
  `admit_budget`) shared across selected-instruction analyses and rewrites.

  Accumulates visitor step counts into an `OptimizationWorkUsage` receipt and
  enforces deterministic refusal on arithmetic overflow or budget exhaustion.
*/

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkDefect {
    Overflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

failed_display!(WorkDefect, "bounded work accounting refused");

pub(crate) fn count(value: usize) -> Result<u64, WorkDefect> {
    u64::try_from(value).map_err(|_| WorkDefect::Overflow)
}

pub fn combine(
    left: OptimizationWorkUsage,
    right: OptimizationWorkUsage,
) -> Result<OptimizationWorkUsage, WorkDefect> {
    Ok(OptimizationWorkUsage {
        rule_evaluations: checked(left.rule_evaluations, right.rule_evaluations)?,
        candidates: checked(left.candidates, right.candidates)?,
        validation_steps: checked(left.validation_steps, right.validation_steps)?,
        commits: checked(left.commits, right.commits)?,
        iterations: checked(left.iterations, right.iterations)?,
    })
}

fn checked(left: u64, right: u64) -> Result<u64, WorkDefect> {
    left.checked_add(right).ok_or(WorkDefect::Overflow)
}

pub fn admit<E>(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
    refused: impl FnOnce(OptimizationWorkUsage, OptimizationWorkBudget) -> E,
) -> Result<(), E> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(refused(usage, budget))
    }
}

pub(crate) struct Work {
    usage: OptimizationWorkUsage,
}

impl Work {
    pub const fn new(initial: OptimizationWorkUsage) -> Self {
        Self { usage: initial }
    }

    pub(crate) fn rule_evaluations(&mut self, amount: u64) -> Result<(), WorkDefect> {
        add(&mut self.usage.rule_evaluations, amount)
    }

    pub(crate) fn candidates(&mut self, amount: u64) -> Result<(), WorkDefect> {
        add(&mut self.usage.candidates, amount)
    }

    pub(crate) fn validation_steps(&mut self, amount: u64) -> Result<(), WorkDefect> {
        add(&mut self.usage.validation_steps, amount)
    }

    pub(crate) fn commits(&mut self, amount: u64) -> Result<(), WorkDefect> {
        add(&mut self.usage.commits, amount)
    }

    pub(crate) fn iterations(&mut self, amount: u64) -> Result<(), WorkDefect> {
        add(&mut self.usage.iterations, amount)
    }

    pub(crate) fn finish(
        self,
        budget: OptimizationWorkBudget,
    ) -> Result<OptimizationWorkUsage, WorkDefect> {
        admit(self.usage, budget, |required, budget| {
            WorkDefect::BudgetExceeded { required, budget }
        })?;
        Ok(self.usage)
    }
}

fn add(target: &mut u64, amount: u64) -> Result<(), WorkDefect> {
    *target = target.checked_add(amount).ok_or(WorkDefect::Overflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(
        rule_evaluations: u64,
        candidates: u64,
        validation_steps: u64,
        commits: u64,
        iterations: u64,
    ) -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(
            rule_evaluations,
            candidates,
            validation_steps,
            commits,
            iterations,
        )
        .expect("explicit nonzero budget")
    }

    #[test]
    fn charges_accumulate_on_their_own_axis() {
        let mut work = Work::new(OptimizationWorkUsage {
            iterations: 2,
            ..Default::default()
        });
        work.rule_evaluations(1).unwrap();
        work.candidates(3).unwrap();
        work.validation_steps(4).unwrap();
        work.commits(1).unwrap();
        work.iterations(5).unwrap();
        assert_eq!(
            work.finish(budget(1, 3, 4, 1, 7)).unwrap(),
            OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 3,
                validation_steps: 4,
                commits: 1,
                iterations: 7,
            }
        );
    }

    #[test]
    fn overflow_refuses_instead_of_wrapping() {
        let mut work = Work::new(OptimizationWorkUsage {
            commits: u64::MAX,
            ..Default::default()
        });
        assert_eq!(work.commits(1), Err(WorkDefect::Overflow));
    }

    #[test]
    fn finish_refuses_exactly_the_axes_over_budget() {
        let mut work = Work::new(OptimizationWorkUsage {
            iterations: 1,
            ..Default::default()
        });
        work.validation_steps(3).unwrap();
        let selected = budget(1, 1, 2, 1, 1);
        assert_eq!(
            work.finish(selected),
            Err(WorkDefect::BudgetExceeded {
                required: OptimizationWorkUsage {
                    validation_steps: 3,
                    iterations: 1,
                    ..Default::default()
                },
                budget: selected,
            })
        );
    }

    #[test]
    fn count_converts_exact_visit_counts() {
        assert_eq!(count(0), Ok(0));
        assert_eq!(count(usize::MAX), Ok(usize::MAX as u64));
    }
}
