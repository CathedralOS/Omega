//! Structural-call Unit forms; ordinary scalars use the instruction graph.
mod model;
use legalized_operations::StructuralUnitLegalizationRecipe;
pub(super) use model::*;

const fn structural_unit_form(
    recipe: StructuralUnitLegalizationRecipe,
    producer_matcher: StructuralUnitLegalizationMatcherKind,
    operations: StructuralUnitOperationShape,
    projected_selected_instruction_count: usize,
    validator: StructuralUnitLegalizationValidatorKind,
) -> LegalizationFormDescriptor {
    LegalizationFormDescriptor {
        recipe,
        producer_matcher,
        constraints: StructuralUnitShapeConstraints {
            block_count: 1,
            scalar_parameter_count: 0,
            operations,
        },
        cost: LegalizationStructuralCost {
            projected_selected_instruction_count,
            introduced_temporary_count: 0,
        },
        validator,
    }
}

pub(super) const LEGALIZATION_FORMS: [LegalizationFormDescriptor; 4] = [
    structural_unit_form(
        StructuralUnitLegalizationRecipe::ReturnUnitV1,
        StructuralUnitLegalizationMatcherKind::ReturnOnly,
        StructuralUnitOperationShape::ReturnOnly,
        1,
        StructuralUnitLegalizationValidatorKind::ReturnOnly,
    ),
    structural_unit_form(
        StructuralUnitLegalizationRecipe::AuthoredCallThenReturnUnitV1,
        StructuralUnitLegalizationMatcherKind::AuthoredCall,
        StructuralUnitOperationShape::CallThenReturn,
        2,
        StructuralUnitLegalizationValidatorKind::AuthoredCall,
    ),
    structural_unit_form(
        StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1,
        StructuralUnitLegalizationMatcherKind::InstalledProviderCall,
        StructuralUnitOperationShape::CallThenReturn,
        2,
        StructuralUnitLegalizationValidatorKind::InstalledProviderCall,
    ),
    structural_unit_form(
        StructuralUnitLegalizationRecipe::ClaimCompletionSettlementsThenReturnUnitV1,
        StructuralUnitLegalizationMatcherKind::ClaimCompletionSettlements,
        StructuralUnitOperationShape::NonEmptySettlementPrefixThenReturn,
        1,
        StructuralUnitLegalizationValidatorKind::ClaimCompletionSettlements,
    ),
];

pub(super) fn legalization_form_for_recipe(
    recipe: StructuralUnitLegalizationRecipe,
) -> Option<&'static LegalizationFormDescriptor> {
    legalization_form_for_recipe_in(&LEGALIZATION_FORMS, recipe)
}

pub(super) fn legalization_form_for_recipe_in(
    catalog: &[LegalizationFormDescriptor],
    recipe: StructuralUnitLegalizationRecipe,
) -> Option<&LegalizationFormDescriptor> {
    let mut matches = catalog
        .iter()
        .filter(|descriptor| descriptor.recipe == recipe);
    let descriptor = matches.next()?;
    matches.next().is_none().then_some(descriptor)
}
