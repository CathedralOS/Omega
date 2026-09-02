//! Sole ordered inventory of every target-legal form admitted by this stage.
//!
//! Rows contain contract data only. Producer matchers and independent replay
//! validators receive distinct dispatch kinds, so the shared inventory cannot
//! become producer-derived validation evidence.

use omega_legalized_operations::{
    LegalizationRecipe, StructuralUnitLegalizationRecipe, UnitLegalizationRecipe,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LegalizationFormRecipe {
    Scalar(LegalizationRecipe),
    Unit(UnitLegalizationRecipe),
    StructuralUnit(StructuralUnitLegalizationRecipe),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarLegalizationMatcherKind {
    Immediate,
    EntryParameter,
    ExactAddImmediate,
    ExactSubtractImmediate,
    WidenedU8ExactAddImmediate,
    WidenedU8ExactSubtractImmediate,
    ActiveResidentExactAddChain,
    ActiveResidentExactAddBridgeChain,
    ActiveResidentExactAddOriginalVictimChain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnitLegalizationMatcherKind {
    ReturnUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructuralUnitLegalizationMatcherKind {
    ReturnUnit,
    AuthoredCallThenReturnUnit,
    InstalledProviderCallThenReturnUnit,
    ClaimCompletionSettlementsThenReturnUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LegalizationProducerMatcherKind {
    Scalar(ScalarLegalizationMatcherKind),
    Unit(UnitLegalizationMatcherKind),
    StructuralUnit(StructuralUnitLegalizationMatcherKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarLegalizationValidatorKind {
    Immediate,
    EntryParameter,
    ExactAddImmediate,
    ExactSubtractImmediate,
    WidenedU8ExactAddImmediate,
    WidenedU8ExactSubtractImmediate,
    ActiveResidentExactAddChain,
    ActiveResidentExactAddBridgeChain,
    ActiveResidentExactAddOriginalVictimChain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnitLegalizationValidatorKind {
    ReturnUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructuralUnitLegalizationValidatorKind {
    ReturnUnit,
    AuthoredCallThenReturnUnit,
    InstalledProviderCallThenReturnUnit,
    ClaimCompletionSettlementsThenReturnUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LegalizationValidatorKind {
    Scalar(ScalarLegalizationValidatorKind),
    Unit(UnitLegalizationValidatorKind),
    StructuralUnit(StructuralUnitLegalizationValidatorKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarShapeConstraints {
    pub condition: ScalarConditionShape,
    pub entry_node_count: usize,
    pub block_offsets: [usize; 3],
    pub operation_count: usize,
    pub leaf_node_counts: [usize; 2],
    pub parameter_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarConditionShape {
    DirectBooleanParameter,
    IntegerEqualU64Parameters,
    IntegerLessThanU64Parameters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UnitShapeConstraints {
    pub block_count: usize,
    pub operation_count: usize,
    pub node_count: usize,
    pub scalar_parameter_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructuralUnitOperationShape {
    ReturnOnly,
    CallThenReturn,
    NonEmptySettlementPrefixThenReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StructuralUnitShapeConstraints {
    pub block_count: usize,
    pub scalar_parameter_count: usize,
    pub operations: StructuralUnitOperationShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LegalizationShapeConstraints {
    Scalar(ScalarShapeConstraints),
    Unit(UnitShapeConstraints),
    StructuralUnit(StructuralUnitShapeConstraints),
}

/// Planning metadata only. It never participates in legality or replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LegalizationStructuralCost {
    pub projected_selected_instruction_count: usize,
    pub introduced_temporary_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LegalizationFormDescriptor {
    pub recipe: LegalizationFormRecipe,
    pub producer_matcher: LegalizationProducerMatcherKind,
    pub constraints: LegalizationShapeConstraints,
    pub cost: LegalizationStructuralCost,
    pub validator: LegalizationValidatorKind,
}

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
pub(super) const LEGALIZATION_FORMS: [LegalizationFormDescriptor; 16] = [
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
