//! Optimizer module role: executable entrance. Canonical source-to-legal construction.

mod custody;
mod matchers;
mod ordinary_roster;
#[cfg(test)]
mod publication_input;
mod scalar_graph;
mod shared;
mod structural;

use crate::legalization::projected_structural_call_return;
use matchers::match_structural_unit_form;
#[cfg(test)]
pub(crate) use publication_input::accepts as accepts_fragment_publication_input;
use shared::*;
use structural::derive_source_structural_unit_function;

pub(crate) struct SourceFunctionRosters {
    pub scalar_functions: Vec<legalized_operations::LegalizedScalarFunction>,
    pub projected_structural_call_returns:
        Vec<legalized_operations::LegalizedProjectedStructuralCallReturn>,
}

/// Validate common custody once, then place every target function in exactly
/// one ordinary graph or retained projected-result closure.
pub(crate) fn derive_source_function_rosters(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<SourceFunctionRosters, LegalizationError> {
    validate_source_custody(target, abstract_plan, unit)?;
    let projected = projected_structural_call_return::derive(target, abstract_plan, unit)?;

    let mut rosters = SourceFunctionRosters {
        scalar_functions: Vec::new(),
        projected_structural_call_returns: projected.iter().cloned().collect(),
    };
    ordinary_roster::derive_remaining(
        &mut rosters,
        projected.as_ref(),
        target,
        abstract_plan,
        unit,
    )?;

    Ok(rosters)
}
use custody::validate_source_custody;
