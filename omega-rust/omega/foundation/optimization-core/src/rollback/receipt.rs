// `OptimizationRollbackReceipt`: partitions `build_selected` into
// `actually_disabled` (`build_selected ∩ requested_disabled`) and `effective`
// (`build_selected \ requested_disabled`) while retaining `requested_disabled`
// for release audit output.

use crate::{Optimization, OptimizationSelections};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationRollbackReceipt {
    build_selected: OptimizationSelections, // Original selection authored in `build.omg`.
    requested_disabled: OptimizationSelections, // Exact rules requested to be disabled by the rollback overlay.
    actually_disabled: OptimizationSelections, // Intersection of `build_selected` and `requested_disabled`.
    effective: OptimizationSelections, // Remainder of `build_selected` after subtracting `requested_disabled`.
}

impl OptimizationRollbackReceipt {
    pub fn new(
        build_selected: OptimizationSelections,
        requested_disabled: OptimizationSelections,
    ) -> Self {
        let actually_disabled = canonical_subset(&build_selected, |optimization| {
            requested_disabled.contains(optimization)
        });
        let effective = canonical_subset(&build_selected, |optimization| {
            !requested_disabled.contains(optimization)
        });
        Self {
            build_selected,
            requested_disabled,
            actually_disabled,
            effective,
        }
    }

    pub const fn build_selected(&self) -> &OptimizationSelections {
        &self.build_selected
    }

    pub const fn requested_disabled(&self) -> &OptimizationSelections {
        &self.requested_disabled
    }

    pub const fn actually_disabled(&self) -> &OptimizationSelections {
        &self.actually_disabled
    }

    pub const fn effective(&self) -> &OptimizationSelections {
        &self.effective
    }

    #[cfg(test)]
    pub(crate) fn is_consistent(&self) -> bool {
        self.actually_disabled
            == canonical_subset(&self.build_selected, |optimization| {
                self.requested_disabled.contains(optimization)
            })
            && self.effective
                == canonical_subset(&self.build_selected, |optimization| {
                    !self.requested_disabled.contains(optimization)
                })
    }
}

impl std::fmt::Display for OptimizationRollbackReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "requested=[{}]; applied=[{}]; effective=[{}]",
            exact_names(&self.requested_disabled),
            exact_names(&self.actually_disabled),
            exact_names(&self.effective),
        )
    }
}

fn canonical_subset(
    source: &OptimizationSelections,
    retain: impl Fn(Optimization) -> bool,
) -> OptimizationSelections {
    OptimizationSelections::new(
        source
            .as_slice()
            .iter()
            .copied()
            .filter(|item| retain(*item)),
    )
    .expect("a subset of a canonical optimization selection remains duplicate-free")
}

fn exact_names(selections: &OptimizationSelections) -> String {
    selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join(", ")
}
