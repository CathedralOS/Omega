//! Optimizer module role: shared bounded-work accounting.
//!
//! Every bounded pass in this stage charges its visitors into one logical
//! [`OptimizationWorkUsage`] receipt and admits the exact total against the
//! selected budget. The checked arithmetic, the exact refusal and the budget
//! admission live here once; a pass only names the axis and amount each
//! visitor charges, started from its own explicit initial receipt.
//!
//! Overflow and budget exhaustion are deterministic refusals, never permission
//! to publish a partial candidate. Admission happens where the total becomes
//! known — the pass that accumulated it — so a receiving seal of freshly
//! produced output does not replay the same condition.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

/// Why bounded work accounting refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkDefect {
    /// A logical counter overflowed `u64`; the receipt can no longer stay exact.
    Overflow,
    /// The exact total work exceeded the selected budget.
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

failed_display!(WorkDefect, "bounded work accounting refused");

impl std::error::Error for WorkDefect {}

/// Convert one `usize` visit count into its exact chargeable amount.
pub(crate) fn count(value: usize) -> Result<u64, WorkDefect> {
    u64::try_from(value).map_err(|_| WorkDefect::Overflow)
}

/// One bounded pass's receipt under construction.
pub(crate) struct Work {
    usage: OptimizationWorkUsage,
}

impl Work {
    /// Start from the pass's exact initial receipt. Every pass names its own
    /// starting totals rather than inheriting a default, so an initial charge
    /// is never implicit.
    pub(crate) const fn new(initial: OptimizationWorkUsage) -> Self {
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

    /// Admit the exact accumulated receipt against the pass's budget.
    pub(crate) fn finish(
        self,
        budget: OptimizationWorkBudget,
    ) -> Result<OptimizationWorkUsage, WorkDefect> {
        if self.usage.within(budget) {
            Ok(self.usage)
        } else {
            Err(WorkDefect::BudgetExceeded {
                required: self.usage,
                budget,
            })
        }
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
