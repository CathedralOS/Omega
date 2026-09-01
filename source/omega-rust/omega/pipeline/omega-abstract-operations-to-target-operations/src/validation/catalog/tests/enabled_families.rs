//! Closed ordered catalog identity and typed-dispatch custody.

use std::collections::BTreeSet;

use super::*;

#[test]
fn enabled_family_identities_are_unique_and_dispatch_is_typed() {
    let ordered = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .map(|descriptor| descriptor.family)
        .collect::<Vec<_>>();
    assert_eq!(
        ordered,
        vec![
            AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
            AbstractToTargetTranslationFamily::StraightLineIntegerWidenImmediate,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseAndImmediate,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerAddImmediate,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotImmediate,
            AbstractToTargetTranslationFamily::StraightLineIntegerExactCastImmediateOperand,
            AbstractToTargetTranslationFamily::StraightLineIntegerEqualImmediate,
            AbstractToTargetTranslationFamily::StraightLineIntegerLessThanImmediate,
            AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualImmediate,
            AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
            AbstractToTargetTranslationFamily::StraightLineBooleanNotImmediate,
            AbstractToTargetTranslationFamily::StraightLineBooleanEqualImmediate,
            AbstractToTargetTranslationFamily::StraightLineUnitReturn,
            AbstractToTargetTranslationFamily::StraightLinePortWriteUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineUnitCallReturn,
            AbstractToTargetTranslationFamily::StraightLineByteSequenceLiteralUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineIntegerLiteralUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineIntegerLiteralSequenceUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralSequenceUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineTrivialAffineLocalUnitReturn,
            AbstractToTargetTranslationFamily::StraightLineScalarCrash,
            AbstractToTargetTranslationFamily::StraightLineIntegerParameter,
            AbstractToTargetTranslationFamily::StraightLineBooleanParameter,
            AbstractToTargetTranslationFamily::StraightLineBooleanNotParameter,
            AbstractToTargetTranslationFamily::StraightLineBooleanEqualParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerEqualParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerLessThanParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotParameter,
            AbstractToTargetTranslationFamily::StraightLineIntegerWidenParameter,
            AbstractToTargetTranslationFamily::StraightLineIntegerExactCastParameter,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseAndParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseOrParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseXorParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftLeftParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftRightParameters,
            AbstractToTargetTranslationFamily::StraightLineExactIntegerShiftLeftParameters,
            AbstractToTargetTranslationFamily::StraightLineExactIntegerShiftRightParameters,
            AbstractToTargetTranslationFamily::StraightLineExactIntegerAddParameters,
            AbstractToTargetTranslationFamily::StraightLineExactIntegerSubtractParameters,
            AbstractToTargetTranslationFamily::StraightLineExactIntegerMultiplyParameters,
            AbstractToTargetTranslationFamily::StraightLineExactIntegerDivideParameters,
            AbstractToTargetTranslationFamily::StraightLineExactIntegerRemainderParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerDivideParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerRemainderParameters,
            AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerDivideParameters,
            AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerRemainderParameters,
            AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerAddParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerAddParameters,
            AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerSubtractParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerSubtractParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerMultiplyParameters,
            AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerMultiplyParameters,
            AbstractToTargetTranslationFamily::StructuralCallReturnCaller,
            AbstractToTargetTranslationFamily::StructuralParameterReturnCallee,
        ]
    );
    let identities = ordered.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(identities.len(), ENABLED_TRANSLATION_FAMILIES.len());

    let (source, target) = boolean_literal_pair();
    let disposition = validate_function(&source, NativeTarget::linux_x64(), &target, &[]).unwrap();
    assert!(matches!(
        disposition,
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate(_)
        )
    ));
}
