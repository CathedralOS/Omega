//! Catalog selection canaries for the ordered mixed literal family.

use super::*;

fn pair() -> (AbstractFunction, TargetFunction) {
    let (mut source, mut target) = ieee_float_literal_unit_return_pair();
    let operation = OperationId::new(59_008).unwrap();
    let result = ValueId::new(59_009).unwrap();
    let scalar_type = IntegerType::new(IntegerSign::Signed, 24).unwrap();
    let value = IntegerValue::Signed(-8_388_608);
    source.operations.insert(
        1,
        AbstractOperation::IntegerConstant {
            psi_operation: operation,
            result,
            scalar_type: ScalarType::Integer(scalar_type),
            value,
        },
    );
    target.provenance.operations.push(operation);
    let TargetOperation::UnitBody(body) = &mut target.operation else {
        unreachable!()
    };
    body.operations.insert(
        1,
        TargetUnitOperation::IntegerConstant {
            psi_operation: operation,
            result,
            scalar_type,
            value,
        },
    );
    (source, target)
}

#[test]
fn omission_and_duplicate_fail_closed() {
    let (source, target) = pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let family = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family
                == AbstractToTargetTranslationFamily::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn
        })
        .copied()
        .unwrap();
    assert!(matches!(
        selection::validate(
            &source,
            NativeTarget::linux_x64(),
            &target,
            &[family, family]
        ),
        Err(
            AbstractToTargetTranslationValidationError::AmbiguousFunctionFamily {
                first: AbstractToTargetTranslationFamily::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn,
                second: AbstractToTargetTranslationFamily::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn,
                ..
            }
        )
    ));
}
