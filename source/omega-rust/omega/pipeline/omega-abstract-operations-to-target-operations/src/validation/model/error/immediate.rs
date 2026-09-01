//! Parameterless scalar-immediate replay errors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineBooleanImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineBooleanNotImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceBooleanNotOperand,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineBooleanEqualImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceEqualOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerEqualImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceConstantType,
    SourceIntegerType,
    SourceConstantOutsideType,
    SourceEqualOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerLessThanImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceConstantType,
    SourceIntegerType,
    SourceConstantOutsideType,
    SourceLessThanOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerLessOrEqualImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceConstantType,
    SourceIntegerType,
    SourceConstantOutsideType,
    SourceLessOrEqualOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceConstantType,
    SourceConstantOutsideType,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerWidenImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceConstantType,
    SourceConstantOutsideType,
    SourceWidenOperand,
    SourceWidenType,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerExactCastImmediateOperandTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceConstantType,
    SourceConstantOutsideType,
    SourceCastOperand,
    SourceCastType,
    SourceCastValueOutsideTarget,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerBitwiseNotImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceConstantType,
    SourceConstantOutsideType,
    SourceBitwiseNotOperand,
    SourceBitwiseNotType,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}
