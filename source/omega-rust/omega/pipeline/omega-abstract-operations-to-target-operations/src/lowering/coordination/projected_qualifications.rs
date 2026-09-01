//! Optimizer module role: semantic leaf. Fail-closed target-lowering fence for path qualification custody.

use omega_abstract_operations::AbstractOperationPlan;

use crate::LoweringError;

pub(super) fn reject_unsupported(plan: &AbstractOperationPlan) -> Result<(), LoweringError> {
    let functions = plan.functions.iter().flat_map(|function| {
        function
            .structural_parameters
            .iter()
            .map(|parameter| &parameter.projected_qualifications)
    });
    let boundaries = plan.boundary_machines.iter().flat_map(|boundary| {
        boundary
            .structural_parameters
            .iter()
            .map(|parameter| &parameter.projected_qualifications)
    });
    let providers = plan.provider_candidates.iter().flat_map(|candidate| {
        candidate
            .signature
            .parameters
            .iter()
            .map(|parameter| &parameter.projected_qualifications)
    });
    if functions
        .chain(boundaries)
        .chain(providers)
        .any(|rows| !rows.is_empty())
    {
        Err(LoweringError::UnsupportedProjectedStructuralQualifications)
    } else {
        Ok(())
    }
}
