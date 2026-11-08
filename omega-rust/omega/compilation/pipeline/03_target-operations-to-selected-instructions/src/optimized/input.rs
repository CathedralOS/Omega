//! What instruction selection reads from the products before it.
//!
//! The coordinator owns the target producer's output and the admission it was
//! lowered from; this stage borrows their plans and the boundary facts the
//! target rows are rejoined to, so it names no earlier stage's product.

use abstract_operations::AbstractOperationPlan;
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationWorkBudget,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use optimization_unit::PsiOptimizationUnit;
use target_operations::{BoundaryOriginFacts, TargetOperationPlan};

/// The target plan, the optimized source it was lowered from, and the
/// optimizer identities and governance selection carries forward.
#[derive(Debug)]
pub struct TargetSelectionInput<'input> {
    pub target: &'input TargetOperationPlan,
    pub source: &'input AbstractOperationPlan,
    pub unit: &'input PsiOptimizationUnit,
    pub origins: BoundaryOriginFacts<'input>,
    pub installed_entry_execution:
        Option<&'input provider_admission::AdmittedInstalledEntryExecution>,
    pub installed_interrupts: &'input provider_admission::AdmittedInstalledInterruptCohort,
    pub installed_stack_requirements:
        &'input [installation_evidence::InstalledInterruptStackRequirement],
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub manifest: PrePhysicalOptimizationManifestIdentity,
    pub selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
}
