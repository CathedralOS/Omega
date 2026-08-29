//! Validated optimized-plan projection custody receipt.

use super::*;

/// Validator-owned receipt for one optimized-unit to abstract-plan projection.
///
/// This is a custody identity, not the final native realization identity. The
/// final unit is independently identified by its canonical content; the
/// transformation ledger separately retains the accepted rewrite history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedOptimizedAbstractPlanProjection {
    pub(super) psi: TerminalPsiIdentity,
    pub(super) fuel_schedule: FuelScheduleIdentity,
    pub(super) initial_unit: OptimizationUnitIdentity,
    pub(super) final_unit: OptimizationUnitIdentity,
    /// Complete source-visible suite requested by the root build.
    pub(super) selections: OptimizationSelectionIdentity,
    /// Exact selection subset whose Psi passes this receipt validates.
    pub(super) psi_selections: OptimizationSelectionIdentity,
    pub(super) ledger: TransformationLedgerIdentity,
    pub(super) bundle: omega_optimization_core::OptimizationIdentityBundleIdentity,
    pub(super) validator: OptimizationValidatorIdentity,
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

    pub const fn bundle(self) -> omega_optimization_core::OptimizationIdentityBundleIdentity {
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
