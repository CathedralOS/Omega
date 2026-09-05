//! Optimizer module role: executable entrance. Pre-physical optimization-manifest projection and replay boundary.
//!
//! The record model, projection mechanics, independent validation, canonical
//! codec/identity, and human rendering descend into named leaves. This
//! entrance alone constructs a candidate and grants validated manifest custody
//! after independent replay.

mod model;
mod projection;
mod validation;

pub use model::*;
pub use validation::validate_pre_physical_optimization_manifest;

use optimization_core::BaselineDecisionLog;
use optimization_core::{
    OptimizationExecutionPhase, OptimizationIdentityBundle, OptimizationPassManifestRecord,
    OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    PrePhysicalOptimizationManifestIdentity,
};
use optimization_unit::{
    OptimizationManifestStage, OptimizationStructuralStatistics, PhysicalOptimizationDataStatus,
    PrePhysicalOptimizationManifest, PsiOptimizationUnit, PsiTransformationLedger,
};
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput;

use crate::validation::ValidatedOptimizedAbstractPlanProjection;

#[allow(clippy::too_many_arguments)]
pub fn project_pre_physical_optimization_manifest(
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
    projection: ValidatedOptimizedAbstractPlanProjection,
) -> Result<ValidatedPrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError> {
    let mut record = projection::expected_record(
        input,
        final_unit,
        selections,
        psi_selections,
        budget_per_pass,
        usage,
        decisions,
        pass_manifests,
        ledger,
        bundle,
        projection,
    )?;
    record.identity = record.recomputed_identity();
    validate_pre_physical_optimization_manifest(
        &record,
        input,
        final_unit,
        selections,
        psi_selections,
        budget_per_pass,
        usage,
        decisions,
        pass_manifests,
        ledger,
        bundle,
        projection,
    )
}
