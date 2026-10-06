//! Optimizer module role: executable entrance. Pre-physical optimization-manifest projection and replay boundary.
//!
//! The record model, projection mechanics, independent validation, canonical
//! codec/identity, and human rendering descend into named leaves. This
//! entrance constructs the manifest of a run this compile produced and grants
//! its custody; `validate_pre_physical_optimization_manifest` replays it for
//! retained readers.

mod projection;

use optimization_core::BaselineDecisionLog;
use optimization_core::{
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationSelections,
    OptimizationWorkBudget, OptimizationWorkUsage, PrePhysicalOptimizationManifestIdentity,
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
    Ok(ValidatedPrePhysicalOptimizationManifest { record })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPrePhysicalOptimizationManifest {
    record: PrePhysicalOptimizationManifest,
}

impl ValidatedPrePhysicalOptimizationManifest {
    pub const fn record(&self) -> &PrePhysicalOptimizationManifest {
        &self.record
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePhysicalOptimizationManifestError {
    InitialUnitProjection,
    StructuralStatisticsOverflow,
}

impl std::fmt::Display for PrePhysicalOptimizationManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-physical optimization manifest: {self:?}"
        )
    }
}

impl std::error::Error for PrePhysicalOptimizationManifestError {}
