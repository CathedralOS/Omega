//! Current legal operations and the optimization configuration admitted for physical selection.
use crate::PreparedExecutionPlan;
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationWorkBudget,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};

#[derive(Debug)]
pub struct PreparedInstructionSelection {
    pub plan: std::sync::Arc<PreparedExecutionPlan>,
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub manifest: PrePhysicalOptimizationManifestIdentity,
    pub target_rules: optimization_core::OptimizationPhaseSelections,
    pub selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
}
