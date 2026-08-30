//! Optimizer module role: executable entrance. Translation error taxonomy and exact family-to-error join.

mod immediate;
mod parameter;
mod terminal;

pub use immediate::{
    StraightLineBooleanImmediateTranslationError, StraightLineIntegerImmediateTranslationError,
};
pub(in crate::validation) use parameter::StraightLineParameterReconstructionError;
pub use parameter::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanNotParameterTranslationError, StraightLineBooleanParameterTranslationError,
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
};
pub use terminal::StraightLineScalarCrashTranslationError;

use psi_core::MachineId;

use super::AbstractToTargetTranslationFamily;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationValidationError {
    PsiMismatch,
    TargetMismatch,
    EntryMismatch,
    FunctionCountMismatch,
    FunctionMachineMismatch {
        position: usize,
    },
    FunctionAttachmentMismatch {
        machine: MachineId,
    },
    AmbiguousFunctionFamily {
        machine: MachineId,
        first: AbstractToTargetTranslationFamily,
        second: AbstractToTargetTranslationFamily,
    },
    FunctionFamily {
        machine: MachineId,
        family: AbstractToTargetTranslationFamily,
        error: AbstractToTargetTranslationFamilyError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationFamilyError {
    StraightLineIntegerImmediate(StraightLineIntegerImmediateTranslationError),
    StraightLineBooleanImmediate(StraightLineBooleanImmediateTranslationError),
    StraightLineScalarCrash(StraightLineScalarCrashTranslationError),
    StraightLineIntegerParameter(StraightLineIntegerParameterTranslationError),
    StraightLineBooleanParameter(StraightLineBooleanParameterTranslationError),
    StraightLineBooleanNotParameter(StraightLineBooleanNotParameterTranslationError),
    StraightLineIntegerBitwiseNotParameter(StraightLineIntegerBitwiseNotParameterTranslationError),
    StraightLineBooleanEqualParameters(StraightLineBooleanEqualParametersTranslationError),
    StraightLineIntegerEqualParameters(StraightLineIntegerEqualParametersTranslationError),
    StraightLineIntegerLessThanParameters(StraightLineIntegerLessThanParametersTranslationError),
    StraightLineIntegerLessOrEqualParameters(
        StraightLineIntegerLessOrEqualParametersTranslationError,
    ),
    StraightLineIntegerWidenParameter(StraightLineIntegerWidenParameterTranslationError),
    StraightLineIntegerExactCastParameter(StraightLineIntegerExactCastParameterTranslationError),
    StraightLineIntegerBitwiseAndParameters(
        StraightLineIntegerBitwiseAndParametersTranslationError,
    ),
    StraightLineIntegerBitwiseOrParameters(StraightLineIntegerBitwiseOrParametersTranslationError),
    StraightLineIntegerBitwiseXorParameters(
        StraightLineIntegerBitwiseXorParametersTranslationError,
    ),
}

impl std::fmt::Display for AbstractToTargetTranslationValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "abstract-to-target translation validation failed: {self:?}"
        )
    }
}

impl std::error::Error for AbstractToTargetTranslationValidationError {}
