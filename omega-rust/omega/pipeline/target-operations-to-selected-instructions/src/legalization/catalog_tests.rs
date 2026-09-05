use super::catalog::*;
use legalized_operations::{
    LegalizationRecipe, ScalarCallUnitLegalizationRecipe, StructuralUnitLegalizationRecipe,
    UnitLegalizationRecipe,
};

const EXPECTED_RECIPES: [LegalizationFormRecipe; 23] = [
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
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64IntegerNotEqualParametersConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(LegalizationRecipe::ReturnU64I64LessThanParametersConditionalV1),
    LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64I64LessOrEqualParametersConditionalV1,
    ),
    LegalizationFormRecipe::Scalar(LegalizationRecipe::ReturnU64EqualZeroParameterConditionalV1),
    LegalizationFormRecipe::Scalar(LegalizationRecipe::ReturnU64NotEqualZeroParameterConditionalV1),
    LegalizationFormRecipe::ScalarCallUnit(
        ScalarCallUnitLegalizationRecipe::U64EqualityConditionalThreeCallChainThenReturnUnitV1,
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

#[test]
fn not_equal_catalog_row_freezes_the_exact_nested_source_grammar() {
    let row = legalization_form_for_recipe(LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64IntegerNotEqualParametersConditionalV1,
    ))
    .expect("not-equal catalog row");
    let LegalizationShapeConstraints::Scalar(constraints) = row.constraints else {
        panic!("scalar not-equal row")
    };
    assert_eq!(
        constraints.condition,
        ScalarConditionShape::IntegerNotEqualU64Parameters
    );
    assert_eq!(constraints.entry_node_count, 3);
    assert_eq!(constraints.block_offsets, [0, 3, 5]);
    assert_eq!(constraints.operation_count, 7);
    assert_eq!(constraints.leaf_node_counts, [2, 2]);
    assert_eq!(constraints.parameter_count, 2);
}

#[test]
fn i64_less_than_catalog_row_freezes_the_exact_source_grammar() {
    let row = legalization_form_for_recipe(LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64I64LessThanParametersConditionalV1,
    ))
    .expect("I64 less-than catalog row");
    let LegalizationShapeConstraints::Scalar(constraints) = row.constraints else {
        panic!("scalar I64 less-than row")
    };
    assert_eq!(
        constraints.condition,
        ScalarConditionShape::IntegerLessThanI64Parameters
    );
    assert_eq!(constraints.entry_node_count, 2);
    assert_eq!(constraints.block_offsets, [0, 2, 4]);
    assert_eq!(constraints.operation_count, 6);
    assert_eq!(constraints.leaf_node_counts, [2, 2]);
    assert_eq!(constraints.parameter_count, 2);
    assert_ne!(
        super::legalization_validator_identity(),
        super::legalization_validator_identity_v17_legacy()
    );
}

#[test]
fn i64_less_or_equal_catalog_row_freezes_the_exact_source_grammar() {
    let row = legalization_form_for_recipe(LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64I64LessOrEqualParametersConditionalV1,
    ))
    .expect("I64 less-or-equal catalog row");
    let LegalizationShapeConstraints::Scalar(constraints) = row.constraints else {
        panic!("scalar I64 less-or-equal row")
    };
    assert_eq!(
        constraints.condition,
        ScalarConditionShape::IntegerLessOrEqualI64Parameters
    );
    assert_eq!(constraints.entry_node_count, 2);
    assert_eq!(constraints.block_offsets, [0, 2, 4]);
    assert_eq!(constraints.operation_count, 6);
    assert_eq!(constraints.leaf_node_counts, [2, 2]);
    assert_eq!(constraints.parameter_count, 2);
    assert_eq!(row.cost.projected_selected_instruction_count, 6);
    assert_eq!(row.cost.introduced_temporary_count, 0);
    assert_ne!(
        super::legalization_validator_identity(),
        super::legalization_validator_identity_v20_legacy()
    );
}

#[test]
fn u64_equal_zero_catalog_row_freezes_the_exact_source_grammar() {
    let row = legalization_form_for_recipe(LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64EqualZeroParameterConditionalV1,
    ))
    .expect("U64 parameter-equals-zero catalog row");
    let LegalizationShapeConstraints::Scalar(constraints) = row.constraints else {
        panic!("scalar U64 parameter-equals-zero row")
    };
    assert_eq!(
        constraints.condition,
        ScalarConditionShape::U64EqualZeroParameter
    );
    assert_eq!(constraints.entry_node_count, 3);
    assert_eq!(constraints.block_offsets, [0, 3, 5]);
    assert_eq!(constraints.operation_count, 7);
    assert_eq!(constraints.leaf_node_counts, [2, 2]);
    assert_eq!(constraints.parameter_count, 1);
    assert_eq!(
        row.producer_matcher,
        LegalizationProducerMatcherKind::Scalar(ScalarLegalizationMatcherKind::Immediate)
    );
    assert_eq!(
        row.validator,
        LegalizationValidatorKind::Scalar(ScalarLegalizationValidatorKind::Immediate)
    );
    assert_eq!(row.cost.projected_selected_instruction_count, 6);
    assert_eq!(row.cost.introduced_temporary_count, 0);

    let without_equal_zero = LEGALIZATION_FORMS
        .iter()
        .copied()
        .filter(|candidate| candidate.recipe != row.recipe)
        .collect::<Vec<_>>();
    assert_eq!(
        legalization_form_for_recipe_in(&without_equal_zero, row.recipe),
        None
    );
    let mut overlapping = LEGALIZATION_FORMS.to_vec();
    overlapping.push(*row);
    assert_eq!(
        legalization_form_for_recipe_in(&overlapping, row.recipe),
        None
    );
    assert_ne!(
        super::legalization_validator_identity(),
        super::legalization_validator_identity_v18_legacy()
    );
    assert_ne!(
        super::legalization_validator_identity_v18_legacy(),
        super::legalization_validator_identity_v17_legacy()
    );
}

#[test]
fn u64_not_equal_zero_catalog_row_freezes_the_exact_source_grammar() {
    let row = legalization_form_for_recipe(LegalizationFormRecipe::Scalar(
        LegalizationRecipe::ReturnU64NotEqualZeroParameterConditionalV1,
    ))
    .expect("U64 parameter-not-equals-zero catalog row");
    let LegalizationShapeConstraints::Scalar(constraints) = row.constraints else {
        panic!("scalar U64 parameter-not-equals-zero row")
    };
    assert_eq!(
        constraints.condition,
        ScalarConditionShape::U64NotEqualZeroParameter
    );
    assert_eq!(constraints.entry_node_count, 4);
    assert_eq!(constraints.block_offsets, [0, 4, 6]);
    assert_eq!(constraints.operation_count, 8);
    assert_eq!(constraints.leaf_node_counts, [2, 2]);
    assert_eq!(constraints.parameter_count, 1);
    assert_eq!(
        row.producer_matcher,
        LegalizationProducerMatcherKind::Scalar(ScalarLegalizationMatcherKind::Immediate)
    );
    assert_eq!(
        row.validator,
        LegalizationValidatorKind::Scalar(ScalarLegalizationValidatorKind::Immediate)
    );
    assert_eq!(row.cost.projected_selected_instruction_count, 6);
    assert_eq!(row.cost.introduced_temporary_count, 0);

    let without_not_equal_zero = LEGALIZATION_FORMS
        .iter()
        .copied()
        .filter(|candidate| candidate.recipe != row.recipe)
        .collect::<Vec<_>>();
    assert_eq!(
        legalization_form_for_recipe_in(&without_not_equal_zero, row.recipe),
        None
    );
    let mut overlapping = LEGALIZATION_FORMS.to_vec();
    overlapping.push(*row);
    assert_eq!(
        legalization_form_for_recipe_in(&overlapping, row.recipe),
        None
    );
    assert_ne!(
        super::legalization_validator_identity(),
        super::legalization_validator_identity_v19_legacy()
    );
    assert_ne!(
        super::legalization_validator_identity_v19_legacy(),
        super::legalization_validator_identity_v18_legacy()
    );
}

#[test]
fn scalar_call_unit_catalog_row_freezes_the_exact_chain_grammar() {
    let row = legalization_form_for_recipe(LegalizationFormRecipe::ScalarCallUnit(
        ScalarCallUnitLegalizationRecipe::U64EqualityConditionalThreeCallChainThenReturnUnitV1,
    ))
    .expect("scalar-call Unit catalog row");
    assert_eq!(
        row.producer_matcher,
        LegalizationProducerMatcherKind::ScalarCallUnit(
            ScalarCallUnitLegalizationMatcherKind::U64EqualityConditionalThreeCallChain,
        )
    );
    assert_eq!(
        row.validator,
        LegalizationValidatorKind::ScalarCallUnit(
            ScalarCallUnitLegalizationValidatorKind::U64EqualityConditionalThreeCallChain,
        )
    );
    let LegalizationShapeConstraints::ScalarCallUnit(constraints) = row.constraints else {
        panic!("scalar-call Unit constraints")
    };
    assert_eq!(constraints.block_count, 1);
    assert_eq!(constraints.operation_count, 6);
    assert_eq!(constraints.node_count, 6);
    assert_eq!(constraints.scalar_parameter_count, 0);
}
