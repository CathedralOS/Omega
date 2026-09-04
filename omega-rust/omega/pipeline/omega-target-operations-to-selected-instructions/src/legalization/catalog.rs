//! Sole ordered inventory of every target-legal form admitted by this stage.
//!
//! Contract-only rows give producers and replay validators distinct dispatch kinds, so inventory cannot
//! become producer-derived validation evidence.

mod model;

pub(super) use model::*;
use omega_legalized_operations::{
    LegalizationRecipe, ScalarCallUnitLegalizationRecipe, StructuralUnitLegalizationRecipe,
    UnitLegalizationRecipe,
};

const fn scalar_form(
    recipe: LegalizationRecipe,
    producer_matcher: ScalarLegalizationMatcherKind,
    block_offsets: [usize; 3],
    operation_count: usize,
    leaf_node_counts: [usize; 2],
    parameter_count: usize,
    projected_selected_instruction_count: usize,
    introduced_temporary_count: usize,
    validator: ScalarLegalizationValidatorKind,
) -> LegalizationFormDescriptor {
    LegalizationFormDescriptor {
        recipe: LegalizationFormRecipe::Scalar(recipe),
        producer_matcher: LegalizationProducerMatcherKind::Scalar(producer_matcher),
        constraints: LegalizationShapeConstraints::Scalar(ScalarShapeConstraints {
            condition: ScalarConditionShape::DirectBooleanParameter,
            entry_node_count: 1,
            block_offsets,
            operation_count,
            leaf_node_counts,
            parameter_count,
        }),
        cost: LegalizationStructuralCost {
            projected_selected_instruction_count,
            introduced_temporary_count,
        },
        validator: LegalizationValidatorKind::Scalar(validator),
    }
}

const fn integer_comparison_scalar_form(
    recipe: LegalizationRecipe,
    condition: ScalarConditionShape,
    producer_matcher: ScalarLegalizationMatcherKind,
    block_offsets: [usize; 3],
    operation_count: usize,
    leaf_node_counts: [usize; 2],
    parameter_count: usize,
    projected_selected_instruction_count: usize,
    introduced_temporary_count: usize,
    validator: ScalarLegalizationValidatorKind,
) -> LegalizationFormDescriptor {
    let mut descriptor = scalar_form(
        recipe,
        producer_matcher,
        block_offsets,
        operation_count,
        leaf_node_counts,
        parameter_count,
        projected_selected_instruction_count,
        introduced_temporary_count,
        validator,
    );
    descriptor.constraints = LegalizationShapeConstraints::Scalar(ScalarShapeConstraints {
        condition,
        entry_node_count: 2,
        block_offsets,
        operation_count,
        leaf_node_counts,
        parameter_count,
    });
    descriptor
}

const fn three_node_integer_comparison_scalar_form(
    recipe: LegalizationRecipe,
    condition: ScalarConditionShape,
    producer_matcher: ScalarLegalizationMatcherKind,
    block_offsets: [usize; 3],
    operation_count: usize,
    leaf_node_counts: [usize; 2],
    parameter_count: usize,
    projected_selected_instruction_count: usize,
    introduced_temporary_count: usize,
    validator: ScalarLegalizationValidatorKind,
) -> LegalizationFormDescriptor {
    let mut descriptor = integer_comparison_scalar_form(
        recipe,
        condition,
        producer_matcher,
        block_offsets,
        operation_count,
        leaf_node_counts,
        parameter_count,
        projected_selected_instruction_count,
        introduced_temporary_count,
        validator,
    );
    let LegalizationShapeConstraints::Scalar(mut constraints) = descriptor.constraints else {
        return descriptor;
    };
    constraints.entry_node_count = 3;
    descriptor.constraints = LegalizationShapeConstraints::Scalar(constraints);
    descriptor
}

const fn four_node_integer_comparison_scalar_form(
    recipe: LegalizationRecipe,
    condition: ScalarConditionShape,
    producer_matcher: ScalarLegalizationMatcherKind,
    block_offsets: [usize; 3],
    operation_count: usize,
    leaf_node_counts: [usize; 2],
    parameter_count: usize,
    projected_selected_instruction_count: usize,
    introduced_temporary_count: usize,
    validator: ScalarLegalizationValidatorKind,
) -> LegalizationFormDescriptor {
    let mut descriptor = three_node_integer_comparison_scalar_form(
        recipe,
        condition,
        producer_matcher,
        block_offsets,
        operation_count,
        leaf_node_counts,
        parameter_count,
        projected_selected_instruction_count,
        introduced_temporary_count,
        validator,
    );
    let LegalizationShapeConstraints::Scalar(mut constraints) = descriptor.constraints else {
        return descriptor;
    };
    constraints.entry_node_count = 4;
    descriptor.constraints = LegalizationShapeConstraints::Scalar(constraints);
    descriptor
}

const fn unit_form() -> LegalizationFormDescriptor {
    LegalizationFormDescriptor {
        recipe: LegalizationFormRecipe::Unit(UnitLegalizationRecipe::ReturnUnitV1),
        producer_matcher: LegalizationProducerMatcherKind::Unit(
            UnitLegalizationMatcherKind::ReturnUnit,
        ),
        constraints: LegalizationShapeConstraints::Unit(UnitShapeConstraints {
            block_count: 1,
            operation_count: 1,
            node_count: 1,
            scalar_parameter_count: 0,
        }),
        cost: LegalizationStructuralCost {
            projected_selected_instruction_count: 1,
            introduced_temporary_count: 0,
        },
        validator: LegalizationValidatorKind::Unit(UnitLegalizationValidatorKind::ReturnUnit),
    }
}

const fn scalar_call_unit_form() -> LegalizationFormDescriptor {
    LegalizationFormDescriptor {
        recipe: LegalizationFormRecipe::ScalarCallUnit(
            ScalarCallUnitLegalizationRecipe::U64EqualityConditionalThreeCallChainThenReturnUnitV1,
        ),
        producer_matcher: LegalizationProducerMatcherKind::ScalarCallUnit(
            ScalarCallUnitLegalizationMatcherKind::U64EqualityConditionalThreeCallChain,
        ),
        constraints: LegalizationShapeConstraints::ScalarCallUnit(ScalarCallUnitShapeConstraints {
            block_count: 1,
            operation_count: 6,
            node_count: 6,
            scalar_parameter_count: 0,
        }),
        cost: LegalizationStructuralCost {
            projected_selected_instruction_count: 10,
            introduced_temporary_count: 0,
        },
        validator: LegalizationValidatorKind::ScalarCallUnit(
            ScalarCallUnitLegalizationValidatorKind::U64EqualityConditionalThreeCallChain,
        ),
    }
}

const fn structural_unit_form(
    recipe: StructuralUnitLegalizationRecipe,
    producer_matcher: StructuralUnitLegalizationMatcherKind,
    operations: StructuralUnitOperationShape,
    projected_selected_instruction_count: usize,
    validator: StructuralUnitLegalizationValidatorKind,
) -> LegalizationFormDescriptor {
    LegalizationFormDescriptor {
        recipe: LegalizationFormRecipe::StructuralUnit(recipe),
        producer_matcher: LegalizationProducerMatcherKind::StructuralUnit(producer_matcher),
        constraints: LegalizationShapeConstraints::StructuralUnit(StructuralUnitShapeConstraints {
            block_count: 1,
            scalar_parameter_count: 0,
            operations,
        }),
        cost: LegalizationStructuralCost {
            projected_selected_instruction_count,
            introduced_temporary_count: 0,
        },
        validator: LegalizationValidatorKind::StructuralUnit(validator),
    }
}

/// The sole precedence, shape, and planning inventory for all current forms.
pub(super) const LEGALIZATION_FORMS: [LegalizationFormDescriptor; 23] = [
    scalar_form(
        LegalizationRecipe::ReturnU64ImmediateConditionalV1,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 1, 3],
        5,
        [2, 2],
        1,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    scalar_form(
        LegalizationRecipe::ReturnU64EntryParameterConditionalV1,
        ScalarLegalizationMatcherKind::EntryParameter,
        [0, 1, 2],
        3,
        [1, 1],
        2,
        4,
        0,
        ScalarLegalizationValidatorKind::EntryParameter,
    ),
    scalar_form(
        LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1,
        ScalarLegalizationMatcherKind::ExactAddImmediate,
        [0, 1, 5],
        9,
        [4, 4],
        1,
        10,
        0,
        ScalarLegalizationValidatorKind::ExactAddImmediate,
    ),
    scalar_form(
        LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1,
        ScalarLegalizationMatcherKind::ExactSubtractImmediate,
        [0, 1, 5],
        9,
        [4, 4],
        1,
        10,
        0,
        ScalarLegalizationValidatorKind::ExactSubtractImmediate,
    ),
    scalar_form(
        LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1,
        ScalarLegalizationMatcherKind::WidenedU8ExactAddImmediate,
        [0, 1, 6],
        11,
        [5, 5],
        1,
        10,
        4,
        ScalarLegalizationValidatorKind::WidenedU8ExactAddImmediate,
    ),
    scalar_form(
        LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1,
        ScalarLegalizationMatcherKind::WidenedU8ExactSubtractImmediate,
        [0, 1, 6],
        11,
        [5, 5],
        1,
        10,
        4,
        ScalarLegalizationValidatorKind::WidenedU8ExactSubtractImmediate,
    ),
    scalar_form(
        LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1,
        ScalarLegalizationMatcherKind::ActiveResidentExactAddChain,
        [0, 1, 8],
        10,
        [7, 2],
        1,
        11,
        0,
        ScalarLegalizationValidatorKind::ActiveResidentExactAddChain,
    ),
    scalar_form(
        LegalizationRecipe::ReturnU64ActiveResidentExactAddBridgeChainConditionalV1,
        ScalarLegalizationMatcherKind::ActiveResidentExactAddBridgeChain,
        [0, 1, 9],
        11,
        [8, 2],
        1,
        12,
        0,
        ScalarLegalizationValidatorKind::ActiveResidentExactAddBridgeChain,
    ),
    scalar_form(
        LegalizationRecipe::ReturnU64ActiveResidentExactAddOriginalVictimChainConditionalV1,
        ScalarLegalizationMatcherKind::ActiveResidentExactAddOriginalVictimChain,
        [0, 1, 10],
        12,
        [9, 2],
        1,
        13,
        0,
        ScalarLegalizationValidatorKind::ActiveResidentExactAddOriginalVictimChain,
    ),
    integer_comparison_scalar_form(
        LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1,
        ScalarConditionShape::IntegerEqualU64Parameters,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 2, 4],
        6,
        [2, 2],
        2,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    integer_comparison_scalar_form(
        LegalizationRecipe::ReturnU64IntegerLessThanParametersConditionalV1,
        ScalarConditionShape::IntegerLessThanU64Parameters,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 2, 4],
        6,
        [2, 2],
        2,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    integer_comparison_scalar_form(
        LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1,
        ScalarConditionShape::IntegerLessOrEqualU64Parameters,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 2, 4],
        6,
        [2, 2],
        2,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    three_node_integer_comparison_scalar_form(
        LegalizationRecipe::ReturnU64IntegerNotEqualParametersConditionalV1,
        ScalarConditionShape::IntegerNotEqualU64Parameters,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 3, 5],
        7,
        [2, 2],
        2,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    integer_comparison_scalar_form(
        LegalizationRecipe::ReturnU64I64LessThanParametersConditionalV1,
        ScalarConditionShape::IntegerLessThanI64Parameters,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 2, 4],
        6,
        [2, 2],
        2,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    integer_comparison_scalar_form(
        LegalizationRecipe::ReturnU64I64LessOrEqualParametersConditionalV1,
        ScalarConditionShape::IntegerLessOrEqualI64Parameters,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 2, 4],
        6,
        [2, 2],
        2,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    three_node_integer_comparison_scalar_form(
        LegalizationRecipe::ReturnU64EqualZeroParameterConditionalV1,
        ScalarConditionShape::U64EqualZeroParameter,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 3, 5],
        7,
        [2, 2],
        1,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    four_node_integer_comparison_scalar_form(
        LegalizationRecipe::ReturnU64NotEqualZeroParameterConditionalV1,
        ScalarConditionShape::U64NotEqualZeroParameter,
        ScalarLegalizationMatcherKind::Immediate,
        [0, 4, 6],
        8,
        [2, 2],
        1,
        6,
        0,
        ScalarLegalizationValidatorKind::Immediate,
    ),
    scalar_call_unit_form(),
    unit_form(),
    structural_unit_form(
        StructuralUnitLegalizationRecipe::ReturnUnitV1,
        StructuralUnitLegalizationMatcherKind::ReturnUnit,
        StructuralUnitOperationShape::ReturnOnly,
        1,
        StructuralUnitLegalizationValidatorKind::ReturnUnit,
    ),
    structural_unit_form(
        StructuralUnitLegalizationRecipe::AuthoredCallThenReturnUnitV1,
        StructuralUnitLegalizationMatcherKind::AuthoredCallThenReturnUnit,
        StructuralUnitOperationShape::CallThenReturn,
        2,
        StructuralUnitLegalizationValidatorKind::AuthoredCallThenReturnUnit,
    ),
    structural_unit_form(
        StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1,
        StructuralUnitLegalizationMatcherKind::InstalledProviderCallThenReturnUnit,
        StructuralUnitOperationShape::CallThenReturn,
        2,
        StructuralUnitLegalizationValidatorKind::InstalledProviderCallThenReturnUnit,
    ),
    structural_unit_form(
        StructuralUnitLegalizationRecipe::ClaimCompletionSettlementsThenReturnUnitV1,
        StructuralUnitLegalizationMatcherKind::ClaimCompletionSettlementsThenReturnUnit,
        StructuralUnitOperationShape::NonEmptySettlementPrefixThenReturn,
        1,
        StructuralUnitLegalizationValidatorKind::ClaimCompletionSettlementsThenReturnUnit,
    ),
];

pub(super) fn legalization_form_for_recipe(
    recipe: LegalizationFormRecipe,
) -> Option<&'static LegalizationFormDescriptor> {
    legalization_form_for_recipe_in(&LEGALIZATION_FORMS, recipe)
}

pub(super) fn legalization_form_for_recipe_in(
    catalog: &[LegalizationFormDescriptor],
    recipe: LegalizationFormRecipe,
) -> Option<&LegalizationFormDescriptor> {
    let mut matches = catalog
        .iter()
        .filter(|descriptor| descriptor.recipe == recipe);
    let descriptor = matches.next()?;
    matches.next().is_none().then_some(descriptor)
}
