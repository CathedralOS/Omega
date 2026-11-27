// A native optimization phase stops when its work passes the budget, so a pass
// that grows superlinearly fails deterministically instead of stalling the build.
//
// `OptimizationWorkBudget` holds five nonzero ceilings calibrated for a
// reference extent of `1 << 16` input units. `for_extent` multiplies every
// ceiling by the number of started reference extents, so a linear pass that
// fits at the reference extent fits at every size. `OptimizationWorkUsage`
// counts the work a pass did on the same five axes.

const OPTIMIZATION_REFERENCE_EXTENT: u64 = 1 << 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationWorkBudget {
    pub(crate) rule_evaluations: u64, // Maximum rule predicate evaluations per reference extent.
    pub(crate) candidates: u64, // Maximum candidate rewrites constructed per reference extent.
    pub(crate) validation_steps: u64, // Maximum validator steps executed per reference extent.
    pub(crate) commits: u64,    // Maximum accepted rewrites committed per reference extent.
    pub(crate) iterations: u64, // Maximum pass fixpoint iterations per reference extent.
}

::wire::record_views! {
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
        encode_work_counters([
            self.rule_evaluations,
            self.candidates,
            self.validation_steps,
            self.commits,
            self.iterations,
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidOptimizationWorkBudget;

::wire::fixed_error_display!(
    InvalidOptimizationWorkBudget,
    "every optimization work-budget axis must be nonzero"
);

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
        encode_work_counters([
            self.rule_evaluations,
            self.candidates,
            self.validation_steps,
            self.commits,
            self.iterations,
        ])
    }
}

fn encode_work_counters(fields: [u64; 5]) -> [u8; 40] {
    let mut encoded = [0; 40];
    for (index, value) in fields.into_iter().enumerate() {
        ::wire::codec_bytes::patch_encoded(&mut encoded, index * 8, value.to_le_bytes());
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::{
        InvalidOptimizationWorkBudget, OPTIMIZATION_REFERENCE_EXTENT, OptimizationWorkBudget,
        OptimizationWorkUsage,
    };

    #[test]
    fn budgets_refuse_a_zero_axis() {
        assert_eq!(
            OptimizationWorkBudget::new(0, 1, 1, 1, 1),
            Err(InvalidOptimizationWorkBudget)
        );
    }

    #[test]
    fn extent_scaled_ceilings_grow_once_per_started_reference_extent() {
        let budget = OptimizationWorkBudget::new(10, 20, 30, 4, 5).unwrap();
        assert_eq!(budget.for_extent(0), budget);
        assert_eq!(budget.for_extent(OPTIMIZATION_REFERENCE_EXTENT), budget);
        assert_eq!(
            budget.for_extent(OPTIMIZATION_REFERENCE_EXTENT + 1),
            OptimizationWorkBudget::new(20, 40, 60, 8, 10).unwrap()
        );
        assert_eq!(
            budget.for_extent(3 * OPTIMIZATION_REFERENCE_EXTENT),
            OptimizationWorkBudget::new(30, 60, 90, 12, 15).unwrap()
        );
        let linear =
            OptimizationWorkBudget::new(1, 1, 2 * OPTIMIZATION_REFERENCE_EXTENT, 1, 1).unwrap();
        for extent in [
            1,
            OPTIMIZATION_REFERENCE_EXTENT,
            7 * OPTIMIZATION_REFERENCE_EXTENT + 5,
        ] {
            assert!(2 * extent <= linear.for_extent(extent).validation_steps());
        }
        let extent = 64 * OPTIMIZATION_REFERENCE_EXTENT;
        assert!(extent * extent > linear.for_extent(extent).validation_steps());
        let wide = OptimizationWorkBudget::new(1, 1, u64::MAX / 2, 1, 1).unwrap();
        assert_eq!(
            wide.for_extent(3 * OPTIMIZATION_REFERENCE_EXTENT)
                .validation_steps(),
            u64::MAX
        );
    }

    #[test]
    fn usage_checks_every_budget_axis() {
        let budget = OptimizationWorkBudget::new(1, 2, 3, 4, 5).unwrap();
        assert!(OptimizationWorkUsage::default().within(budget));
        let exact = OptimizationWorkUsage {
            rule_evaluations: 1,
            candidates: 2,
            validation_steps: 3,
            commits: 4,
            iterations: 5,
        };
        assert!(exact.within(budget));
        for excess in [
            OptimizationWorkUsage {
                rule_evaluations: 2,
                ..exact
            },
            OptimizationWorkUsage {
                candidates: 3,
                ..exact
            },
            OptimizationWorkUsage {
                validation_steps: 4,
                ..exact
            },
            OptimizationWorkUsage {
                commits: 5,
                ..exact
            },
            OptimizationWorkUsage {
                iterations: 6,
                ..exact
            },
        ] {
            assert!(
                !excess.within(budget),
                "over-budget work admitted: {excess:?}"
            );
        }
    }
}
