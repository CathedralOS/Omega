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
