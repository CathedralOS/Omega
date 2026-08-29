//! Ordered inventory of the scalar legalization forms admitted by this stage.
//!
//! This is contract data, not executable validation. Producer matchers and
//! independent replay validators use distinct dispatch kinds so sharing the
//! inventory cannot turn producer recognition into validation evidence.

use omega_legalized_operations::LegalizationRecipe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarLegalizationMatcherKind {
    Immediate,
    EntryParameter,
    ExactAddImmediate,
    ExactSubtractImmediate,
    WidenedU8ExactAddImmediate,
    WidenedU8ExactSubtractImmediate,
    ActiveResidentExactAddChain,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarShapeConstraints {
    pub block_offsets: [usize; 3],
    pub operation_count: usize,
    pub leaf_node_counts: [usize; 2],
    pub parameter_count: usize,
}

/// Planning metadata only. These values never participate in legality or
/// independent replay and therefore cannot authorize a transformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarStructuralCost {
    pub projected_selected_instruction_count: usize,
    pub introduced_temporary_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarLegalizationFormDescriptor {
    pub recipe: LegalizationRecipe,
    pub producer_matcher: ScalarLegalizationMatcherKind,
    pub constraints: ScalarShapeConstraints,
    pub cost: ScalarStructuralCost,
    pub validator: ScalarLegalizationValidatorKind,
}

const fn form(
    recipe: LegalizationRecipe,
    producer_matcher: ScalarLegalizationMatcherKind,
    block_offsets: [usize; 3],
    operation_count: usize,
    leaf_node_counts: [usize; 2],
    parameter_count: usize,
    projected_selected_instruction_count: usize,
    introduced_temporary_count: usize,
    validator: ScalarLegalizationValidatorKind,
) -> ScalarLegalizationFormDescriptor {
    ScalarLegalizationFormDescriptor {
        recipe,
        producer_matcher,
        constraints: ScalarShapeConstraints {
            block_offsets,
            operation_count,
            leaf_node_counts,
            parameter_count,
        },
        cost: ScalarStructuralCost {
            projected_selected_instruction_count,
            introduced_temporary_count,
        },
        validator,
    }
}

/// The sole precedence, shape, and planning inventory for scalar forms.
pub(super) const SCALAR_LEGALIZATION_FORMS: [ScalarLegalizationFormDescriptor; 7] = [
    form(
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
    form(
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
    form(
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
    form(
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
    form(
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
    form(
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
    form(
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
];

pub(super) fn scalar_form_for_recipe(
    recipe: LegalizationRecipe,
) -> Option<&'static ScalarLegalizationFormDescriptor> {
    SCALAR_LEGALIZATION_FORMS
        .iter()
        .find(|descriptor| descriptor.recipe == recipe)
}
