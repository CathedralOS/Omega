//! Optimizer module role: executable entrance. Optimized-unit to abstract-plan projection coordination.
//!
//! Validation proceeds in one visible order: transformed unit and ledger,
//! identity bundle, pass manifests, then reconstructible projection shape.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractOperationPlan;
use optimization_core::{BaselineDecisionLog, BaselineDecisionLogDecodeError};
use optimization_core::{
    OptimizationCandidateVerdict, OptimizationExecutionPhase, OptimizationIdentityBundle,
    OptimizationPassManifestRecord, OptimizationRuleSetIdentity, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizedAbstractPlanProjectionIdentity, TargetCostModelIdentity, TransformationLedgerIdentity,
};
use optimization_unit::{
    InvalidPsiTransformationLedger, ProvenanceDisposition, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiTransformationLedger,
};
use semantic_vocabulary::FuelScheduleIdentity;
use terminal_psi::TerminalPsiIdentity;
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput;

use crate::validation::{
    validate_transformed_psi_optimization_unit, validate_verified_psi_optimization_unit,
};
use optimization_unit_semantics::OptimizationUnitValidationError;

mod custody;
mod error;
mod identity_bundle;
mod initial_ledger;
mod manifests;
mod shape;
#[cfg(test)]
mod tests;

pub use error::OptimizedAbstractPlanProjectionError;
pub(super) use manifests::validate_manifests;

#[allow(clippy::too_many_arguments)]
pub fn validate_optimized_abstract_plan_projection(
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    projected: &AbstractOperationPlan,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    expected_rule_set: OptimizationRuleSetIdentity,
    expected_cost_model: TargetCostModelIdentity,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
) -> Result<ValidatedOptimizedAbstractPlanProjection, OptimizedAbstractPlanProjectionError> {
    let initial_identity = initial_ledger::validate_initial_and_ledger(input, final_unit, ledger)?;
    identity_bundle::validate_identity_bundle(
        selections,
        psi_selections,
        expected_rule_set,
        expected_cost_model,
        decisions,
        ledger,
        bundle,
    )?;
    validate_manifests(pass_manifests, expected_rule_set, ledger)?;
    shape::validate_projection_shape(input.plan(), final_unit, projected)?;

    Ok(ValidatedOptimizedAbstractPlanProjection {
        psi: final_unit.psi,
        fuel_schedule: final_unit.fuel_schedule,
        initial_unit: initial_identity,
        final_unit: final_unit.identity,
        selections: selections.identity(),
        psi_selections: psi_selections.identity(),
        ledger: ledger.identity(),
        bundle: bundle.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.optimized-abstract-plan-projection.v33",
        ),
    })
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
