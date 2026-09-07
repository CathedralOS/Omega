//! Structural-call Unit catalog rows.
use legalized_operations::StructuralUnitLegalizationRecipe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum StructuralUnitLegalizationMatcherKind {
    ReturnOnly,
    AuthoredCall,
    InstalledProviderCall,
    ClaimCompletionSettlements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) enum StructuralUnitLegalizationValidatorKind {
    ReturnOnly,
    AuthoredCall,
    InstalledProviderCall,
    ClaimCompletionSettlements,
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
pub(in crate::legalization) struct LegalizationStructuralCost {
    pub projected_selected_instruction_count: usize,
    pub introduced_temporary_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::legalization) struct LegalizationFormDescriptor {
    pub recipe: StructuralUnitLegalizationRecipe,
    pub producer_matcher: StructuralUnitLegalizationMatcherKind,
    pub constraints: StructuralUnitShapeConstraints,
    pub cost: LegalizationStructuralCost,
    pub validator: StructuralUnitLegalizationValidatorKind,
}
