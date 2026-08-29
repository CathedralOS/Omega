use super::catalog::*;
use omega_legalized_operations::LegalizationRecipe;

#[test]
fn scalar_catalog_has_one_canonical_row_for_every_current_recipe() {
    let expected = [
        LegalizationRecipe::ReturnU64ImmediateConditionalV1,
        LegalizationRecipe::ReturnU64EntryParameterConditionalV1,
        LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1,
        LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1,
        LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1,
        LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1,
        LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1,
    ];
    assert_eq!(
        SCALAR_LEGALIZATION_FORMS.map(|descriptor| descriptor.recipe),
        expected
    );
    for recipe in expected {
        assert_eq!(
            SCALAR_LEGALIZATION_FORMS
                .iter()
                .filter(|descriptor| descriptor.recipe == recipe)
                .count(),
            1
        );
        assert_eq!(
            scalar_form_for_recipe(recipe).map(|row| row.recipe),
            Some(recipe)
        );
    }
}

#[test]
fn structural_cost_is_separate_from_legality_constraints() {
    assert_eq!(
        SCALAR_LEGALIZATION_FORMS.map(|descriptor| descriptor.cost),
        [
            ScalarStructuralCost {
                projected_selected_instruction_count: 6,
                introduced_temporary_count: 0,
            },
            ScalarStructuralCost {
                projected_selected_instruction_count: 4,
                introduced_temporary_count: 0,
            },
            ScalarStructuralCost {
                projected_selected_instruction_count: 10,
                introduced_temporary_count: 0,
            },
            ScalarStructuralCost {
                projected_selected_instruction_count: 10,
                introduced_temporary_count: 0,
            },
            ScalarStructuralCost {
                projected_selected_instruction_count: 10,
                introduced_temporary_count: 4,
            },
            ScalarStructuralCost {
                projected_selected_instruction_count: 10,
                introduced_temporary_count: 4,
            },
            ScalarStructuralCost {
                projected_selected_instruction_count: 11,
                introduced_temporary_count: 0,
            },
        ]
    );
}
