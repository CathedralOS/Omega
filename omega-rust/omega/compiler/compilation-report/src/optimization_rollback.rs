//! The three optimization sets a release rollback keeps apart: what the build
//! selected, what a release asked to turn off, and the intersection that actually
//! was removed before the selected stages executed.

use optimization_core::{Optimization, OptimizationSelections};

/// Release-tooling custody for one subtractive exact-rule overlay.
///
/// The authored build selection remains intact. `requested_disabled` is the
/// fleet/release request, `actually_disabled` is its intersection with this
/// build, and `effective` is the only set allowed to enter artifact production.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationRollbackReceipt {
    build_selected: OptimizationSelections,
    requested_disabled: OptimizationSelections,
    actually_disabled: OptimizationSelections,
    effective: OptimizationSelections,
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

    pub fn is_consistent(&self) -> bool {
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
