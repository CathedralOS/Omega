//! Shared parameter reconstruction errors and exact family maps.

mod comparison;
mod direct;
mod unary;

pub use comparison::*;
pub use direct::*;
pub use unary::*;

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
map_parameter_reconstruction_error!(StraightLineBooleanEqualParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerEqualParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerLessThanParametersTranslationError);
map_parameter_reconstruction_error!(StraightLineIntegerLessOrEqualParametersTranslationError);
