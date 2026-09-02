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
pub enum StraightLineIntegerBitwiseAndImmediateTranslationError {
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
    SourceBitwiseAndOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerBitwiseOrImmediateTranslationError {
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
    SourceBitwiseOrOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerBitwiseXorImmediateTranslationError {
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
    SourceBitwiseXorOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineWrappingIntegerAddImmediateTranslationError {
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
    SourceWrappingAddOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineSaturatingIntegerAddImmediateTranslationError {
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
    SourceSaturatingAddOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineSaturatingIntegerSubtractImmediateTranslationError {
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
    SourceSaturatingSubtractOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineSaturatingIntegerMultiplyImmediateTranslationError {
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
    SourceSaturatingMultiplyOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineWrappingIntegerShiftLeftImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceValueConstantType,
    SourceCountConstantType,
    SourceValueType,
    SourceCountType,
    SourceValueOutsideType,
    SourceCountOutsideType,
    SourceWrappingShiftOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineWrappingIntegerShiftRightImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceDefinitionRoster,
    SourceValueConstantType,
    SourceCountConstantType,
    SourceValueType,
    SourceCountType,
    SourceValueOutsideType,
    SourceCountOutsideType,
    SourceWrappingShiftOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineWrappingIntegerSubtractImmediateTranslationError {
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
    SourceWrappingSubtractOperands,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineWrappingIntegerMultiplyImmediateTranslationError {
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
    SourceWrappingMultiplyOperands,
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
