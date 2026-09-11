//! Exact receipt-to-family identity projection.

use super::AbstractToTargetFunctionTranslationReceipt;
use crate::validation::model::AbstractToTargetTranslationFamily;

impl AbstractToTargetFunctionTranslationReceipt {
    pub const fn family(&self) -> AbstractToTargetTranslationFamily {
        match self {
            Self::StraightLineUnitCallReturn(_) => AbstractToTargetTranslationFamily::StraightLineUnitCallReturn,
            Self::StraightLineUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineUnitReturn,
            Self::StraightLinePortWriteUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLinePortWriteUnitReturn,
            Self::StraightLineByteSequenceLiteralUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineByteSequenceLiteralUnitReturn,
            Self::StraightLineIntegerLiteralUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineIntegerLiteralUnitReturn,
            Self::StraightLineIntegerLiteralSequenceUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineIntegerLiteralSequenceUnitReturn,
            Self::StraightLineIeeeFloatLiteralUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralUnitReturn,
            Self::StraightLineIeeeFloatLiteralSequenceUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralSequenceUnitReturn,
            Self::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn,
            Self::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn,
            Self::StraightLineTrivialAffineLocalUnitReturn(_) => AbstractToTargetTranslationFamily::StraightLineTrivialAffineLocalUnitReturn,
            Self::StructuralCallReturnCaller(_) => AbstractToTargetTranslationFamily::StructuralCallReturnCaller,
            Self::StructuralParameterReturnCallee(_) => AbstractToTargetTranslationFamily::StructuralParameterReturnCallee,
        }
    }
}
