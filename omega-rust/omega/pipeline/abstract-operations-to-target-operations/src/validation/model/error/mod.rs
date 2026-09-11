//! Optimizer module role: executable entrance. Translation error taxonomy and exact family-to-error join.

mod family;
mod terminal;
mod validation;

pub use self::terminal::{
    StraightLineByteSequenceLiteralUnitReturnTranslationError,
    StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError,
    StraightLineIeeeFloatLiteralUnitReturnTranslationError,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError,
    StraightLineIntegerLiteralSequenceUnitReturnTranslationError,
    StraightLineIntegerLiteralUnitReturnTranslationError,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError,
    StraightLinePortWriteUnitReturnTranslationError,
    StraightLineTrivialAffineLocalUnitReturnTranslationError,
    StraightLineUnitCallReturnTranslationError, StraightLineUnitReturnTranslationError,
};
pub use family::AbstractToTargetTranslationFamilyError;
pub use validation::AbstractToTargetTranslationValidationError;
