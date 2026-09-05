//! Optimizer module role: executable entrance. Optimized-unit to abstract-plan projection coordination.
//!
//! Validation proceeds in one visible order: transformed unit and ledger,
//! identity bundle, pass manifests, then reconstructible projection shape.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperationPlan;
use omega_optimization_core::{BaselineDecisionLog, BaselineDecisionLogDecodeError};
use omega_optimization_core::{
    OptimizationCandidateVerdict, OptimizationExecutionPhase, OptimizationIdentityBundle,
    OptimizationPassManifestRecord, OptimizationRuleSetIdentity, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizedAbstractPlanProjectionIdentity, TargetCostModelIdentity, TransformationLedgerIdentity,
};
use omega_optimization_unit::{
    InvalidPsiTransformationLedger, ProvenanceDisposition, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiTransformationLedger,
};
use omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput;
use psi_core::FuelScheduleIdentity;
use psi_terminal::TerminalPsiIdentity;

use crate::{
    OptimizationUnitValidationError, validate_transformed_psi_optimization_unit,
    validate_verified_psi_optimization_unit,
};

mod custody;
mod error;
mod identity_bundle;
mod initial_ledger;
mod manifests;
mod model;
mod shape;
#[cfg(test)]
mod tests;

pub use error::OptimizedAbstractPlanProjectionError;
pub(super) use manifests::validate_manifests;
pub use model::ValidatedOptimizedAbstractPlanProjection;

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
