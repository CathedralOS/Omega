//! Optimizer module role: stage catalog. The selected-instructions stage's
//! single ordered rule catalog.
//!
//! `wiki/spec/build/optimizations.md` gives each stage one ordered catalog:
//! an exact entry binds its rule to the stage that admits it. The family
//! catalogs — [`SELECTED_LOWERING_RULE_CATALOG`] and
//! [`ALLOCATION_RECOVERY_RULE_CATALOG`] — remain the per-row detail owners
//! (policy, pair-rule rosters). [`SELECTED_STAGE_RULE_CATALOG`] is the stage's
//! one declaration of which phases carry selectable rules and in what order,
//! so the phase resolvers' membership checks and any custody consumer read
//! one stage catalog rather than a per-family name table.

use optimization_core::{Optimization, OptimizationExecutionPhase};

use super::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry,
    SELECTED_LOWERING_RULE_CATALOG, SelectedLoweringRuleCatalogEntry,
};

/// One family's ordered rule rows inside the stage catalog. The slice names
/// the family catalog itself, so the stage table cannot drift from the
/// per-row detail the family owns.
#[derive(Debug, Clone, Copy)]
pub enum SelectedStageRuleRows {
    SelectedLowering(&'static [SelectedLoweringRuleCatalogEntry]),
    AllocationRecovery(&'static [AllocationRecoveryRuleCatalogEntry]),
}

impl SelectedStageRuleRows {
    /// Whether `optimization` is one of this family slice's admitted rules.
    pub fn contains(&self, optimization: Optimization) -> bool {
        match self {
            Self::SelectedLowering(rows) => rows
                .iter()
                .any(|entry| entry.optimization() == optimization),
            Self::AllocationRecovery(rows) => rows
                .iter()
                .any(|entry| entry.optimization() == optimization),
        }
    }

    /// The slice's admitted rules in catalog order.
    pub fn optimizations(&self) -> Vec<Optimization> {
        match self {
            Self::SelectedLowering(rows) => rows.iter().map(|entry| entry.optimization()).collect(),
            Self::AllocationRecovery(rows) => {
                rows.iter().map(|entry| entry.optimization()).collect()
            }
        }
    }

    /// The slice's rule count.
    pub fn len(&self) -> usize {
        match self {
            Self::SelectedLowering(rows) => rows.len(),
            Self::AllocationRecovery(rows) => rows.len(),
        }
    }

    /// Whether the slice admits no rules.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One phase's slice of the stage catalog: every selectable rule the stage
/// admits under `phase`, ordered exactly as its family catalog orders it.
#[derive(Debug, Clone, Copy)]
pub struct SelectedStageRuleCatalogSlice {
    phase: OptimizationExecutionPhase,
    rows: SelectedStageRuleRows,
}

impl SelectedStageRuleCatalogSlice {
    pub const fn new(phase: OptimizationExecutionPhase, rows: SelectedStageRuleRows) -> Self {
        Self { phase, rows }
    }

    pub const fn phase(&self) -> OptimizationExecutionPhase {
        self.phase
    }

    pub const fn rows(&self) -> SelectedStageRuleRows {
        self.rows
    }

    /// Whether `optimization` is admitted under this slice's phase.
    pub fn contains(&self, optimization: Optimization) -> bool {
        self.rows.contains(optimization)
    }
}

/// The stage's one ordered catalog, in dispatch order: selected-lowering
/// rules run before any allocation-recovery rule, matching
/// `optimize_selected_instructions`, which rejects a mixed
/// selection as an unsupported composition. Phases the stage does not carry
/// (for example `PreAllocation`) have no slice: they admit no selectable rule
/// here rather than listing an empty catalog.
pub const SELECTED_STAGE_RULE_CATALOG: [SelectedStageRuleCatalogSlice; 2] = [
    SelectedStageRuleCatalogSlice::new(
        OptimizationExecutionPhase::SelectedLowering,
        SelectedStageRuleRows::SelectedLowering(&SELECTED_LOWERING_RULE_CATALOG),
    ),
    SelectedStageRuleCatalogSlice::new(
        OptimizationExecutionPhase::AllocationRecovery,
        SelectedStageRuleRows::AllocationRecovery(&ALLOCATION_RECOVERY_RULE_CATALOG),
    ),
];

/// The catalog slice admitting rules under `phase`, if this stage carries it.
pub(crate) fn selected_stage_rule_rows(
    phase: OptimizationExecutionPhase,
) -> Option<SelectedStageRuleRows> {
    SELECTED_STAGE_RULE_CATALOG
        .iter()
        .find(|slice| slice.phase() == phase)
        .map(|slice| slice.rows())
}

/// Stage-level membership: `optimization` is a selectable rule under `phase`.
/// This is the membership authority both phase resolvers consult.
pub(crate) fn selected_stage_catalog_contains(
    phase: OptimizationExecutionPhase,
    optimization: Optimization,
) -> bool {
    selected_stage_rule_rows(phase).is_some_and(|rows| rows.contains(optimization))
}

#[cfg(test)]
mod tests {
    use super::{
        SELECTED_STAGE_RULE_CATALOG, SelectedStageRuleRows, selected_stage_catalog_contains,
        selected_stage_rule_rows,
    };
    use optimization_core::{Optimization, OptimizationExecutionPhase};

    /// The stage catalog's phase slices appear in dispatch order and name the
    /// family catalogs themselves — the stage table cannot drift from the
    /// family-owned rows.
    #[test]
    fn stage_catalog_names_each_family_catalog_in_dispatch_order() {
        assert_eq!(SELECTED_STAGE_RULE_CATALOG.len(), 2);
        let [lowering, recovery] = SELECTED_STAGE_RULE_CATALOG;
        assert_eq!(
            lowering.phase(),
            OptimizationExecutionPhase::SelectedLowering
        );
        assert_eq!(
            recovery.phase(),
            OptimizationExecutionPhase::AllocationRecovery
        );
        assert!(matches!(
            lowering.rows(),
            SelectedStageRuleRows::SelectedLowering(_)
        ));
        assert!(matches!(
            recovery.rows(),
            SelectedStageRuleRows::AllocationRecovery(_)
        ));
    }

    /// Every selectable `Optimization` of a carried phase appears in that
    /// phase's slice in vocabulary order, and no optimization lands in two
    /// slices — a stage catalog that skipped or duplicated a rule would
    /// misreport the admitted surface.
    #[test]
    fn stage_catalog_covers_each_carried_phases_vocabulary_exactly() {
        for slice in SELECTED_STAGE_RULE_CATALOG {
            let declared = Optimization::ALL
                .into_iter()
                .filter(|optimization| optimization.execution_phase() == slice.phase())
                .collect::<Vec<_>>();
            assert_eq!(declared, slice.rows().optimizations());
            for optimization in Optimization::ALL {
                assert_eq!(
                    slice.contains(optimization),
                    declared.contains(&optimization),
                );
            }
        }
        let mut flattened = Vec::new();
        for slice in SELECTED_STAGE_RULE_CATALOG {
            flattened.extend(slice.rows().optimizations());
        }
        flattened.sort_by_key(|optimization| *optimization as u32);
        flattened.dedup();
        assert_eq!(
            flattened.len(),
            SELECTED_STAGE_RULE_CATALOG
                .iter()
                .map(|slice| slice.rows().len())
                .sum::<usize>(),
        );
    }

    /// Phases the stage does not carry have no slice and admit nothing —
    /// absence from the stage catalog is the rejection, not an empty table a
    /// resolver could pass over silently.
    #[test]
    fn phases_without_a_slice_admit_no_rule() {
        let carried = [
            OptimizationExecutionPhase::SelectedLowering,
            OptimizationExecutionPhase::AllocationRecovery,
        ];
        for phase in [
            OptimizationExecutionPhase::CheckedTrees,
            OptimizationExecutionPhase::Psi,
            OptimizationExecutionPhase::AbstractOperations,
            OptimizationExecutionPhase::TargetOperations,
            OptimizationExecutionPhase::PreAllocation,
            OptimizationExecutionPhase::PostAllocationMachine,
            OptimizationExecutionPhase::FunctionRelativeLayout,
        ] {
            assert!(selected_stage_rule_rows(phase).is_none());
            for optimization in Optimization::ALL {
                assert!(!selected_stage_catalog_contains(phase, optimization));
            }
        }
        for phase in carried {
            assert!(selected_stage_rule_rows(phase).is_some());
        }
    }
}
