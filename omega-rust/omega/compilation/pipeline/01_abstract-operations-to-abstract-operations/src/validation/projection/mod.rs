//! Optimizer module role: executable entrance. Optimized-unit to abstract-plan projection coordination.
//!
//! Validation proceeds in one visible order: transformed unit and ledger,
//! identity bundle, pass manifests, then reconstructible projection shape.


use optimization_core::{
    OptimizationIdentityBundle, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizedAbstractPlanProjectionIdentity, TransformationLedgerIdentity,
};
use optimization_unit::{
    PsiOptimizationUnit, PsiTransformationLedger,
};
use semantic_vocabulary::FuelScheduleIdentity;
use terminal_psi::TerminalPsiIdentity;



/// Seal the projection receipt of a run this compile produced. The ledger
/// records the initial unit identity it started from; the rewrites were
/// validated as the pass manager committed them.
pub fn seal_optimized_abstract_plan_projection(
    final_unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
) -> ValidatedOptimizedAbstractPlanProjection {
    ValidatedOptimizedAbstractPlanProjection {
        psi: final_unit.psi,
        fuel_schedule: final_unit.fuel_schedule,
        initial_unit: ledger.input(),
        final_unit: final_unit.identity,
        selections: selections.identity(),
        psi_selections: psi_selections.identity(),
        ledger: ledger.identity(),
        bundle: bundle.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.optimized-abstract-plan-projection.v33",
        ),
    }
}


/// Validator-owned receipt for one optimized-unit to abstract-plan projection.
///
/// This is a custody identity, not the final native realization identity. The
/// final unit is independently identified by its canonical content; the
/// transformation ledger separately retains the accepted rewrite history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedOptimizedAbstractPlanProjection {
    psi: TerminalPsiIdentity,
    fuel_schedule: FuelScheduleIdentity,
    initial_unit: OptimizationUnitIdentity,
    final_unit: OptimizationUnitIdentity,
    /// Complete source-visible suite requested by the root build.
    selections: OptimizationSelectionIdentity,
    /// Exact selection subset whose Psi passes this receipt validates.
    psi_selections: OptimizationSelectionIdentity,
    ledger: TransformationLedgerIdentity,
    bundle: optimization_core::OptimizationIdentityBundleIdentity,
    validator: OptimizationValidatorIdentity,
}

impl ValidatedOptimizedAbstractPlanProjection {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn initial_unit(self) -> OptimizationUnitIdentity {
        self.initial_unit
    }

    pub const fn final_unit(self) -> OptimizationUnitIdentity {
        self.final_unit
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn psi_selections(self) -> OptimizationSelectionIdentity {
        self.psi_selections
    }

    pub const fn ledger(self) -> TransformationLedgerIdentity {
        self.ledger
    }

    pub const fn bundle(self) -> optimization_core::OptimizationIdentityBundleIdentity {
        self.bundle
    }

    pub const fn validator(self) -> OptimizationValidatorIdentity {
        self.validator
    }

    /// Domain-separated custody identity of every independently validated
    /// source, revision, selection, ledger, bundle, and validator field.
    /// This is suitable for downstream joins but grants no physical-emission
    /// or publication authority.
    pub fn identity(self) -> OptimizedAbstractPlanProjectionIdentity {
        let mut canonical = Vec::with_capacity(272);
        canonical.extend_from_slice(&self.psi.vocabulary_marker.get().to_le_bytes());
        canonical.extend_from_slice(self.psi.program_fingerprint.as_bytes());
        canonical.extend_from_slice(&self.fuel_schedule.marker().to_le_bytes());
        canonical.extend_from_slice(&self.initial_unit.bytes());
        canonical.extend_from_slice(&self.final_unit.bytes());
        canonical.extend_from_slice(&self.selections.bytes());
        canonical.extend_from_slice(&self.psi_selections.bytes());
        canonical.extend_from_slice(&self.ledger.bytes());
        canonical.extend_from_slice(&self.bundle.bytes());
        canonical.extend_from_slice(&self.validator.bytes());
        OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(&canonical)
    }
}
