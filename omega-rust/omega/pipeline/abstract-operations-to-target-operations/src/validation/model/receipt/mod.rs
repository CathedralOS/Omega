//! Optimizer module role: executable entrance. Translation receipt taxonomy and exact family-to-receipt join.

mod family;
mod function_translation;
mod roster;
mod terminal;

pub use self::terminal::{
    IeeeFloatFusedMultiplyAddOperandReceipt, IeeeFloatLiteralSequenceMember,
    IntegerIeeeFloatLiteralSequenceMember, IntegerLiteralSequenceMember,
    StraightLineByteSequenceLiteralUnitReturnTranslationReceipt,
    StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIeeeFloatLiteralUnitReturnTranslationReceipt,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIntegerLiteralUnitReturnTranslationReceipt,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt,
    StraightLinePortWriteUnitReturnTranslationReceipt,
    StraightLineTrivialAffineLocalUnitReturnTranslationReceipt,
    StraightLineUnitCallReturnTranslationReceipt, StraightLineUnitReturnTranslationReceipt,
};
pub use function_translation::AbstractToTargetFunctionTranslationReceipt;
pub use roster::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationReceipt,
};
