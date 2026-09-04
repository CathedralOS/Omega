//! Typed rows consumed by the sole ordered legalization catalog.

use omega_legalized_operations::{
    LegalizationRecipe, ScalarCallUnitLegalizationRecipe, StructuralUnitLegalizationRecipe,
    UnitLegalizationRecipe,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum LegalizationFormRecipe {
    Scalar(LegalizationRecipe),
    Unit(UnitLegalizationRecipe),
    ScalarCallUnit(ScalarCallUnitLegalizationRecipe),
    StructuralUnit(StructuralUnitLegalizationRecipe),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum ScalarLegalizationMatcherKind {
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
pub(in crate::legalization) enum UnitLegalizationMatcherKind {
    ReturnUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum ScalarCallUnitLegalizationMatcherKind {
    U64EqualityConditionalThreeCallChain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // Rows retain the exact legalization shape they recognize.
pub(in crate::legalization) enum StructuralUnitLegalizationMatcherKind {
    ReturnUnit,
    AuthoredCallThenReturnUnit,
    InstalledProviderCallThenReturnUnit,
    ClaimCompletionSettlementsThenReturnUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum LegalizationProducerMatcherKind {
    Scalar(ScalarLegalizationMatcherKind),
    Unit(UnitLegalizationMatcherKind),
    ScalarCallUnit(ScalarCallUnitLegalizationMatcherKind),
    StructuralUnit(StructuralUnitLegalizationMatcherKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum ScalarLegalizationValidatorKind {
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
pub(in crate::legalization) enum UnitLegalizationValidatorKind {
    ReturnUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum ScalarCallUnitLegalizationValidatorKind {
    U64EqualityConditionalThreeCallChain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // Rows retain the exact legalization shape they validate.
pub(in crate::legalization) enum StructuralUnitLegalizationValidatorKind {
    ReturnUnit,
    AuthoredCallThenReturnUnit,
    InstalledProviderCallThenReturnUnit,
    ClaimCompletionSettlementsThenReturnUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum LegalizationValidatorKind {
    Scalar(ScalarLegalizationValidatorKind),
    Unit(UnitLegalizationValidatorKind),
    ScalarCallUnit(ScalarCallUnitLegalizationValidatorKind),
    StructuralUnit(StructuralUnitLegalizationValidatorKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) struct ScalarShapeConstraints {
    pub condition: ScalarConditionShape,
    pub entry_node_count: usize,
    pub block_offsets: [usize; 3],
    pub operation_count: usize,
    pub leaf_node_counts: [usize; 2],
    pub parameter_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum ScalarConditionShape {
    DirectBooleanParameter,
    IntegerEqualU64Parameters,
    IntegerLessThanU64Parameters,
    IntegerLessOrEqualU64Parameters,
    IntegerNotEqualU64Parameters,
    IntegerLessThanI64Parameters,
    IntegerLessOrEqualI64Parameters,
    U64EqualZeroParameter,
    U64NotEqualZeroParameter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) struct UnitShapeConstraints {
    pub block_count: usize,
    pub operation_count: usize,
    pub node_count: usize,
    pub scalar_parameter_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) struct ScalarCallUnitShapeConstraints {
    pub block_count: usize,
    pub operation_count: usize,
    pub node_count: usize,
    pub scalar_parameter_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum StructuralUnitOperationShape {
    ReturnOnly,
    CallThenReturn,
    NonEmptySettlementPrefixThenReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) struct StructuralUnitShapeConstraints {
    pub block_count: usize,
    pub scalar_parameter_count: usize,
    pub operations: StructuralUnitOperationShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum LegalizationShapeConstraints {
    Scalar(ScalarShapeConstraints),
    Unit(UnitShapeConstraints),
    ScalarCallUnit(ScalarCallUnitShapeConstraints),
    StructuralUnit(StructuralUnitShapeConstraints),
}

/// Planning metadata only. It never participates in legality or replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) struct LegalizationStructuralCost {
    pub projected_selected_instruction_count: usize,
    pub introduced_temporary_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) struct LegalizationFormDescriptor {
    pub recipe: LegalizationFormRecipe,
    pub producer_matcher: LegalizationProducerMatcherKind,
    pub constraints: LegalizationShapeConstraints,
    pub cost: LegalizationStructuralCost,
    pub validator: LegalizationValidatorKind,
}
