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
            AbstractToTargetTranslationFamily::StructuralCallReturnCaller,
            AbstractToTargetTranslationFamily::StructuralParameterReturnCallee,
        ]
    );
    let identities = ordered.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(identities.len(), ENABLED_TRANSLATION_FAMILIES.len());

    let (source, target) = unit_call_pair();
    let disposition = validate_function(&source, NativeTarget::linux_x64(), &target, &[]).unwrap();
    assert!(matches!(
        disposition,
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitCallReturn(_)
        )
    ));
}
