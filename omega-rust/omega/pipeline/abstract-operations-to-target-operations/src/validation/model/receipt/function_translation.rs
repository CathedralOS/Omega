//! Closed tagged carrier for independently replayed Unit and structural families.

use super::terminal::{
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
use crate::validation::{
    StructuralCallReturnCallerTranslationReceipt, StructuralParameterReturnCalleeTranslationReceipt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetFunctionTranslationReceipt {
    StraightLineUnitCallReturn(StraightLineUnitCallReturnTranslationReceipt),
    StraightLineUnitReturn(StraightLineUnitReturnTranslationReceipt),
    StraightLinePortWriteUnitReturn(StraightLinePortWriteUnitReturnTranslationReceipt),
    StraightLineByteSequenceLiteralUnitReturn(
        StraightLineByteSequenceLiteralUnitReturnTranslationReceipt,
    ),
    StraightLineIntegerLiteralUnitReturn(StraightLineIntegerLiteralUnitReturnTranslationReceipt),
    StraightLineIntegerLiteralSequenceUnitReturn(
        StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt,
    ),
    StraightLineIeeeFloatLiteralUnitReturn(
        StraightLineIeeeFloatLiteralUnitReturnTranslationReceipt,
    ),
    StraightLineIeeeFloatLiteralSequenceUnitReturn(
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    ),
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn(
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    ),
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn(
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt,
    ),
    StraightLineTrivialAffineLocalUnitReturn(
        StraightLineTrivialAffineLocalUnitReturnTranslationReceipt,
    ),
    StructuralCallReturnCaller(StructuralCallReturnCallerTranslationReceipt),
    StructuralParameterReturnCallee(StructuralParameterReturnCalleeTranslationReceipt),
}
