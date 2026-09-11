//! Optimizer module role: executable entrance. Canonical source-to-legal construction.

mod custody;
mod ordinary_roster;
#[cfg(test)]
mod publication_input;
mod scalar_graph;
mod shared;

#[cfg(test)]
pub(crate) use publication_input::accepts as accepts_fragment_publication_input;
use shared::*;

pub(crate) struct SourceFunctionRosters {
    pub scalar_functions: Vec<legalized_operations::LegalizedScalarFunction>,
}

/// Validate common custody once, then place every target function in exactly
/// one ordinary graph.
pub(crate) fn derive_source_function_rosters(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    verified_input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<SourceFunctionRosters, LegalizationError> {
    validate_source_custody(target, abstract_plan, unit, verified_input)?;

    let mut rosters = SourceFunctionRosters {
        scalar_functions: Vec::new(),
    };
    ordinary_roster::derive_remaining(&mut rosters, target, abstract_plan, unit)?;

    Ok(rosters)
}
use custody::validate_source_custody;
