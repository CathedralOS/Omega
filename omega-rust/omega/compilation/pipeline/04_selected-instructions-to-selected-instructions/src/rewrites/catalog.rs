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
    PRE_ALLOCATION_RULE_CATALOG, PreAllocationRuleCatalogEntry, SELECTED_LOWERING_RULE_CATALOG,
    SelectedLoweringRuleCatalogEntry,
};

/// One family's ordered rule rows inside the stage catalog. The slice names
/// the family catalog itself, so the stage table cannot drift from the
/// per-row detail the family owns.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SelectedStageRuleRows {
    SelectedLowering(&'static [SelectedLoweringRuleCatalogEntry]),
    PreAllocation(&'static [PreAllocationRuleCatalogEntry]),
    AllocationRecovery(&'static [AllocationRecoveryRuleCatalogEntry]),
}

impl SelectedStageRuleRows {
    /// Whether `optimization` is one of this family slice's admitted rules.
    pub fn contains(&self, optimization: Optimization) -> bool {
        match self {
            Self::SelectedLowering(rows) => rows
                .iter()
                .any(|entry| entry.optimization() == optimization),
            Self::PreAllocation(rows) => rows
                .iter()
                .any(|entry| entry.optimization() == optimization),
            Self::AllocationRecovery(rows) => rows
                .iter()
                .any(|entry| entry.optimization() == optimization),
        }
    }
}

/// One phase's slice of the stage catalog: every selectable rule the stage
/// admits under `phase`, ordered exactly as its family catalog orders it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedStageRuleCatalogSlice {
    phase: OptimizationExecutionPhase,
    rows: SelectedStageRuleRows,
}

impl SelectedStageRuleCatalogSlice {
    pub const fn new(phase: OptimizationExecutionPhase, rows: SelectedStageRuleRows) -> Self {
        Self { phase, rows }
    }

    receipt_accessor_rows!(ref SelectedStageRuleCatalogSlice {
        phase: OptimizationExecutionPhase,
        rows: SelectedStageRuleRows,
    });
}

/// The stage's one ordered catalog, in dispatch order: selected-lowering
/// rules run before pre-allocation rules, and both run before any
/// allocation-recovery rule, matching `optimize_selected_instructions`,
/// which rejects a mixed selection as an unsupported composition. Phases the
/// stage does not carry have no slice: they admit no selectable rule here
/// rather than listing an empty catalog.
pub(crate) const SELECTED_STAGE_RULE_CATALOG: [SelectedStageRuleCatalogSlice; 3] = [
    SelectedStageRuleCatalogSlice::new(
        OptimizationExecutionPhase::SelectedLowering,
        SelectedStageRuleRows::SelectedLowering(&SELECTED_LOWERING_RULE_CATALOG),
    ),
    SelectedStageRuleCatalogSlice::new(
        OptimizationExecutionPhase::PreAllocation,
        SelectedStageRuleRows::PreAllocation(&PRE_ALLOCATION_RULE_CATALOG),
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
