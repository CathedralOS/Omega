use super::catalog::*;
use omega_legalized_operations::{
    LegalizationRecipe, StructuralUnitLegalizationRecipe, UnitLegalizationRecipe,
};

const EXPECTED_RECIPES: [LegalizationFormRecipe; 17] = [
    LegalizationFormRecipe::Scalar(LegalizationRecipe::ReturnU64ImmediateConditionalV1),
    LegalizationFormRecipe::Scalar(LegalizationRecipe::ReturnU64EntryParameterConditionalV1),
    LegalizationFormRecipe::Scalar(LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64ActiveResidentExactAddBridgeChainConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64ActiveResidentExactAddOriginalVictimChainConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64IntegerLessThanParametersConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1,
    ),
    LegalizationFormRecipe::Unit(UnitLegalizationRecipe::ReturnUnitV1),
    LegalizationFormRecipe::StructuralUnit(StructuralUnitLegalizationRecipe::ReturnUnitV1),
    LegalizationFormRecipe::StructuralUnit(
        StructuralUnitLegalizationRecipe::AuthoredCallThenReturnUnitV1,
    ),
    LegalizationFormRecipe::StructuralUnit(
        StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1,
    ),
    LegalizationFormRecipe::StructuralUnit(
        StructuralUnitLegalizationRecipe::ClaimCompletionSettlementsThenReturnUnitV1,
    ),
];

#[test]
fn catalog_has_exact_order_unique_rows_and_total_lookup() {
    assert_eq!(
        LEGALIZATION_FORMS.map(|descriptor| descriptor.recipe),
        EXPECTED_RECIPES
    );
    for recipe in EXPECTED_RECIPES {
        assert_eq!(
            LEGALIZATION_FORMS
                .iter()
                .filter(|descriptor| descriptor.recipe == recipe)
                .count(),
            1
        );
        assert_eq!(
            legalization_form_for_recipe(recipe).map(|row| row.recipe),
            Some(recipe)
        );
    }
}

#[test]
fn every_catalog_row_is_a_real_enablement_switch() {
    for disabled_recipe in EXPECTED_RECIPES {
        let enabled = LEGALIZATION_FORMS
            .iter()
            .copied()
            .filter(|row| row.recipe != disabled_recipe)
            .collect::<Vec<_>>();
        assert_eq!(
            legalization_form_for_recipe_in(&enabled, disabled_recipe),
            None,
            "an omitted row must disable its recipe"
        );
        for enabled_recipe in EXPECTED_RECIPES {
            if enabled_recipe != disabled_recipe {
                assert_eq!(
                    legalization_form_for_recipe_in(&enabled, enabled_recipe).map(|row| row.recipe),
                    Some(enabled_recipe)
                );
            }
        }
    }
}

#[test]
fn ambiguous_catalog_recipe_fails_closed() {
    for duplicated_recipe in EXPECTED_RECIPES {
        let duplicate = *legalization_form_for_recipe(duplicated_recipe).unwrap();
        let mut ambiguous = LEGALIZATION_FORMS.to_vec();
        ambiguous.push(duplicate);
        assert_eq!(
            legalization_form_for_recipe_in(&ambiguous, duplicated_recipe),
            None
        );
    }
}

#[test]
fn planning_cost_is_present_for_all_families_but_not_a_legality_key() {
    assert_eq!(LEGALIZATION_FORMS.len(), EXPECTED_RECIPES.len());
    assert!(LEGALIZATION_FORMS.iter().all(|row| {
        row.cost.projected_selected_instruction_count > 0
            && row.cost.introduced_temporary_count <= 4
    }));
}
