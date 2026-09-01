//! Shared parameter-envelope replay failures projected into exact family vocabularies.

use super::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanNotParameterTranslationError, StraightLineBooleanParameterTranslationError,
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineExactIntegerMultiplyParametersTranslationError,
    StraightLineExactIntegerSubtractParametersTranslationError,
    StraightLineIntegerBitwiseAndParametersTranslationError,
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerBitwiseOrParametersTranslationError,
    StraightLineIntegerBitwiseXorParametersTranslationError,
    StraightLineIntegerEqualParametersTranslationError,
    StraightLineIntegerExactCastParameterTranslationError,
    StraightLineIntegerLessOrEqualParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationError,
    StraightLineIntegerParameterTranslationError,
    StraightLineIntegerWidenParameterTranslationError,
    StraightLineSaturatingIntegerAddParametersTranslationError,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};

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
map_parameter_reconstruction_error!(StraightLineIntegerBitwiseNotParameterTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerWidenParameterTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerExactCastParameterTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerBitwiseAndParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerBitwiseOrParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerBitwiseXorParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineExactIntegerAddParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineExactIntegerMultiplyParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineExactIntegerSubtractParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineSaturatingIntegerAddParametersTranslationError);
map_parameter_reconstruction_error!(
    StraightLineSaturatingIntegerMultiplyParametersTranslationError
);
map_parameter_reconstruction_error!(
    StraightLineSaturatingIntegerSubtractParametersTranslationError
);
map_parameter_reconstruction_error!(StraightLineWrappingIntegerAddParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineWrappingIntegerSubtractParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineWrappingIntegerMultiplyParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineBooleanEqualParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerEqualParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerLessThanParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerLessOrEqualParametersTranslationError);
