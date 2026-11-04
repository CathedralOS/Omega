use crate::OptimizationUnitValidationError;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BoundaryMachineId, StructuralPlaceKind, StructuralTypeId};
use std::collections::BTreeMap;

/// No producer emits provider-attachment roots: providers hold no state, so a
/// function has no attachment field to specialize. A root is therefore
/// invalid rather than a specialization witness to replay.
pub(crate) fn validate_provider_attachment_specialization(
    function: &PsiOptimizationFunction,
    _boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    _types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    if function
        .structural_places
        .iter()
        .any(|place| matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
    {
        return Err(
            OptimizationUnitValidationError::InvalidProviderAttachmentSpecialization(
                function.machine,
            ),
        );
    }
    Ok(())
}
