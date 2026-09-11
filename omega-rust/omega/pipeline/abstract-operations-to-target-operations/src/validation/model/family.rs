//! Stable identities for independently replayed abstract-to-target families.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractToTargetTranslationFamily {
    StraightLineUnitReturn,
    StraightLinePortWriteUnitReturn,
    StraightLineUnitCallReturn,
    StraightLineByteSequenceLiteralUnitReturn,
    StraightLineIntegerLiteralUnitReturn,
    StraightLineIntegerLiteralSequenceUnitReturn,
    StraightLineIeeeFloatLiteralUnitReturn,
    StraightLineIeeeFloatLiteralSequenceUnitReturn,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn,
    StraightLineTrivialAffineLocalUnitReturn,
    StructuralCallReturnCaller,
    StructuralParameterReturnCallee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractToTargetPlanTranslationFamily {
    StructuralCallReturnProjectedQualifications,
}
