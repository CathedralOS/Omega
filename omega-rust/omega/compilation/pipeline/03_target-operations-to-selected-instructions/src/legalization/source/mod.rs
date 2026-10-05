//! Optimizer module role: executable entrance. Canonical source-to-legal construction.

use crate::LegalizationError;
use abstract_operations::AbstractOperationPlan;
use optimization_unit::PsiOptimizationUnit;
use target_operations::TargetOperationPlan;
mod custody;
mod ordinary_roster;
mod scalar_graph;

pub(crate) struct SourceFunctionRosters {
    pub scalar_functions: Vec<legalized_operations::LegalizedScalarFunction>,
}

/// Validate common custody once, then place every target function in exactly
/// one ordinary graph.
pub(crate) fn derive_source_function_rosters(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<SourceFunctionRosters, LegalizationError> {
    validate_source_custody(target, abstract_plan, unit)?;
    // A retained callback argument must join to one exact normalized foreign
    // row before any roster derives from this plan.
    super::scalar_graph_input::normalized_foreign::validate_native_callback_roster(target)?;

    let mut rosters = SourceFunctionRosters {
        scalar_functions: Vec::new(),
    };
    ordinary_roster::derive_remaining(&mut rosters, target, abstract_plan, unit)?;

    Ok(rosters)
}
use custody::validate_source_custody;
