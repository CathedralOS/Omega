//! Terminal Unit-return and scalar-Crash replay errors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineUnitReturnTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceCleanupActions,
    TargetFixedIntegerScalarAbi,
    TargetProvenance,
    TargetOperation,
    TargetCallPlan,
    TargetParameters,
    TargetOperationRoster,
    TargetReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLinePortWriteUnitReturnTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceCleanupActions,
    TargetFixedIntegerScalarAbi,
    TargetProvenance,
    TargetOperation,
    TargetCallPlan,
    TargetParameters,
    TargetOperationRoster,
    TargetPortWrite,
    TargetReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineUnitCallReturnTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceStructuralArguments,
    SourceClaimTransfers,
    SourceCleanupActions,
    TargetFixedIntegerScalarAbi,
    TargetProvenance,
    TargetOperation,
    TargetCallPlan,
    TargetParameters,
    TargetOperationRoster,
    TargetCall,
    TargetReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineTrivialAffineLocalUnitReturnTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourcePlace,
    SourceStructuralType,
    SourceCleanupActions,
    TargetFixedIntegerScalarAbi,
    TargetProvenance,
    TargetOperation,
    TargetCallPlan,
    TargetParameters,
    TargetOperationRoster,
    TargetEstablishment,
    TargetReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineScalarCrashTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    TargetProvenance,
    TargetOperation,
}
