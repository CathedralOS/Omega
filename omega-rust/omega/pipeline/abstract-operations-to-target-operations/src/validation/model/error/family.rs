//! Closed error carrier for independently replayed Unit and structural families.

use super::terminal::{
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
use crate::validation::StructuralCallReturnProjectedQualificationValidationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationFamilyError {
    StraightLineUnitCallReturn(StraightLineUnitCallReturnTranslationError),
    StraightLineUnitReturn(StraightLineUnitReturnTranslationError),
    StraightLinePortWriteUnitReturn(StraightLinePortWriteUnitReturnTranslationError),
    StraightLineByteSequenceLiteralUnitReturn(
        StraightLineByteSequenceLiteralUnitReturnTranslationError,
    ),
    StraightLineIntegerLiteralUnitReturn(StraightLineIntegerLiteralUnitReturnTranslationError),
    StraightLineIntegerLiteralSequenceUnitReturn(
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError,
    ),
    StraightLineIeeeFloatLiteralUnitReturn(StraightLineIeeeFloatLiteralUnitReturnTranslationError),
    StraightLineIeeeFloatLiteralSequenceUnitReturn(
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError,
    ),
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn(
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError,
    ),
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn(
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError,
    ),
    StraightLineTrivialAffineLocalUnitReturn(
        StraightLineTrivialAffineLocalUnitReturnTranslationError,
    ),
    StructuralCallReturnCaller(StructuralCallReturnProjectedQualificationValidationError),
    StructuralParameterReturnCallee(StructuralCallReturnProjectedQualificationValidationError),
}
