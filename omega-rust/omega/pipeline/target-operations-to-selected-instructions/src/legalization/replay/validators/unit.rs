use crate::legalization::catalog::{
    LegalizationFormDescriptor, LegalizationFormRecipe, LegalizationShapeConstraints,
    LegalizationValidatorKind, UnitLegalizationValidatorKind, legalization_form_for_recipe,
};

use super::super::shared::*;

pub(crate) fn validate_unit_form(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    recipe: legalized_operations::UnitLegalizationRecipe,
) -> Option<&'static LegalizationFormDescriptor> {
    let descriptor = legalization_form_for_recipe(LegalizationFormRecipe::Unit(recipe))?;
    let (
        LegalizationValidatorKind::Unit(UnitLegalizationValidatorKind::ReturnUnit),
        LegalizationShapeConstraints::Unit(constraints),
    ) = (descriptor.validator, descriptor.constraints)
    else {
        return None;
    };
    let TargetOperation::UnitBody(body) = &target.operation else {
        return None;
    };
    (body.parameters.is_empty()
        && abstracted.structural_parameters.is_empty()
        && optimized.structural_parameters.is_empty()
        && abstracted.entry_claims.is_empty()
        && optimized.entry_claim_declarations.is_empty()
        && optimized.entry_claims.is_empty()
        && optimized.declared_places.is_empty()
        && abstracted.published_service_ceiling.is_empty()
        && optimized.published_service_ceiling.is_empty()
        && matches!(
            body.operations.as_slice(),
            [TargetUnitOperation::Return { .. }]
        )
        && abstracted.block_entries.len() == constraints.block_count
        && optimized.blocks.len() == constraints.block_count
        && abstracted.operations.len() == constraints.operation_count
        && optimized
            .blocks
            .first()
            .is_some_and(|block| block.nodes.len() == constraints.node_count)
        && abstracted.parameters.len() == constraints.scalar_parameter_count
        && optimized.parameters.len() == constraints.scalar_parameter_count)
        .then_some(descriptor)
}

pub(super) fn independently_plain_unit_contract(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> bool {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return false;
    };
    body.parameters.is_empty()
        && abstracted.structural_parameters.is_empty()
        && optimized.structural_parameters.is_empty()
        && abstracted.entry_claims.is_empty()
        && optimized.entry_claim_declarations.is_empty()
        && optimized.entry_claims.is_empty()
        && optimized.declared_places.is_empty()
        && abstracted.published_service_ceiling.is_empty()
        && optimized.published_service_ceiling.is_empty()
        && matches!(
            body.operations.as_slice(),
            [TargetUnitOperation::Return { .. }]
        )
}
