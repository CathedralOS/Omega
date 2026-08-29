//! Shared and exact parameter-derived replay errors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerParameterTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineBooleanParameterTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineBooleanNotParameterTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceNotResultRoster,
    SourceOperandLink,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineBooleanEqualParametersTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceEqualResultRoster,
    SourceLeftOperandLink,
    SourceRightOperandLink,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerEqualParametersTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceEqualResultRoster,
    SourceLeftOperandLink,
    SourceRightOperandLink,
    SourceOperandTypeMismatch,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerLessThanParametersTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceLessThanResultRoster,
    SourceLeftOperandLink,
    SourceRightOperandLink,
    SourceOperandTypeMismatch,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerLessOrEqualParametersTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceLessOrEqualResultRoster,
    SourceLeftOperandLink,
    SourceRightOperandLink,
    SourceOperandTypeMismatch,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::validation) enum StraightLineParameterReconstructionError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
}

macro_rules! map_parameter_reconstruction_error {
    ($family:ty) => {
        impl From<StraightLineParameterReconstructionError> for $family {
            fn from(error: StraightLineParameterReconstructionError) -> Self {
                match error {
                    StraightLineParameterReconstructionError::SourceParameters => {
                        Self::SourceParameters
                    }
                    StraightLineParameterReconstructionError::SourceStructuralParameters => {
                        Self::SourceStructuralParameters
                    }
                    StraightLineParameterReconstructionError::SourceResult => Self::SourceResult,
                    StraightLineParameterReconstructionError::SourceEntryClaims => {
                        Self::SourceEntryClaims
                    }
                    StraightLineParameterReconstructionError::SourcePublishedServices => {
                        Self::SourcePublishedServices
                    }
                    StraightLineParameterReconstructionError::SourceBlockRoster => {
                        Self::SourceBlockRoster
                    }
                    StraightLineParameterReconstructionError::SourceOperationRoster => {
                        Self::SourceOperationRoster
                    }
                    StraightLineParameterReconstructionError::SourceParameterRoster => {
                        Self::SourceParameterRoster
                    }
                    StraightLineParameterReconstructionError::SourceParameterShape => {
                        Self::SourceParameterShape
                    }
                    StraightLineParameterReconstructionError::SourceReturnLink => {
                        Self::SourceReturnLink
                    }
                    StraightLineParameterReconstructionError::SourceCleanup => Self::SourceCleanup,
                    StraightLineParameterReconstructionError::AbiPlan => Self::AbiPlan,
                    StraightLineParameterReconstructionError::AbiParameterCount => {
                        Self::AbiParameterCount
                    }
                    StraightLineParameterReconstructionError::AbiParameterPlacement => {
                        Self::AbiParameterPlacement
                    }
                    StraightLineParameterReconstructionError::TargetProvenance => {
                        Self::TargetProvenance
                    }
                }
            }
        }
    };
}

map_parameter_reconstruction_error!(StraightLineIntegerParameterTranslationError);
map_parameter_reconstruction_error!(StraightLineBooleanParameterTranslationError);
map_parameter_reconstruction_error!(StraightLineBooleanNotParameterTranslationError);
map_parameter_reconstruction_error!(StraightLineBooleanEqualParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerEqualParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerLessThanParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerLessOrEqualParametersTranslationError);
