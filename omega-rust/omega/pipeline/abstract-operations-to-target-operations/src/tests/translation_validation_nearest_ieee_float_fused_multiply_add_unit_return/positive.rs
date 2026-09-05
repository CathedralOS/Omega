use super::*;

#[test]
fn validates_raw_bit_fma_custody_on_every_applicable_native_target_and_format() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
    ] {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let source = base_plan(format);
            let plan = provider_plan(target, format);
            let admitted = settlement(target, format, &plan);
            let lowered = lowered(&source, target, &[admitted]);
            let receipt = validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
                &source,
                target,
                &lowered,
                &[admitted],
            )
            .unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("the exact FMA grammar must select only its named family")
            };
            assert_eq!(row.machine(), machine());
            assert_eq!(row.fma_operation(), fma_operation());
            assert_eq!(row.fma_result(), fma_result());
            assert_eq!(row.format(), format);
            assert_eq!(row.slot(), slot(format));
            assert_eq!(row.provider(), admitted.provider);
            assert_eq!(
                row.provider_plan_report_identity(),
                plan.report_fingerprint()
            );
            assert_eq!(
                row.provider_plan_digest(),
                *plan.identity_digest().as_bytes()
            );
            assert_eq!(row.return_edge(), return_edge());
            for (position, expected) in values(format).into_iter().enumerate() {
                assert_eq!(
                    row.literals()[position].operation(),
                    literal_operation(position)
                );
                assert_eq!(row.literals()[position].result(), literal_result(position));
                assert_eq!(row.literals()[position].value(), expected);
                assert_eq!(
                    row.operands()[position].defining_operation(),
                    literal_operation(position)
                );
                assert_eq!(
                    row.operands()[position].source_value(),
                    literal_result(position)
                );
                assert_eq!(row.operands()[position].value(), expected);
            }
        }
    }
}

#[test]
fn whole_plan_settlement_roster_is_exact() {
    let target = NativeTarget::linux_x64();
    let format = IeeeFloatFormat::Binary32;
    let source = base_plan(format);
    let plan = provider_plan(target, format);
    let admitted = settlement(target, format, &plan);
    let lowered = lowered(&source, target, &[admitted]);
    assert_eq!(
        validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
            &source,
            target,
            &lowered,
            &[],
        ),
        Err(
            AbstractToTargetTranslationValidationError::MissingIeeeFloatFmaSettlement(
                fma_operation(),
            )
        )
    );
    assert_eq!(
        validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
            &source,
            target,
            &lowered,
            &[admitted, admitted],
        ),
        Err(
            AbstractToTargetTranslationValidationError::DuplicateIeeeFloatFmaSettlement(
                fma_operation(),
            )
        )
    );
    let unknown = AdmittedIeeeFloatFmaSettlement {
        terminal_operation: OperationId::new(61_090).unwrap(),
        ..admitted
    };
    assert_eq!(
        validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
            &source,
            target,
            &lowered,
            &[admitted, unknown],
        ),
        Err(
            AbstractToTargetTranslationValidationError::UnknownIeeeFloatFmaSettlement(
                unknown.terminal_operation,
            )
        )
    );
}

#[test]
fn both_arm_targets_are_typed_family_applicability_failures() {
    let format = IeeeFloatFormat::Binary32;
    let x86 = NativeTarget::linux_x64();
    let plan = provider_plan(x86, format);
    let admitted = settlement(x86, format, &plan);
    let source = base_plan(format);
    let lowered = lowered(&source, x86, &[admitted]);
    for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
        assert_eq!(
            crate::validation::straight_line_nearest_ieee_float_fused_multiply_add_unit_return::validate(
                &source.functions[0],
                target,
                &lowered.functions[0],
                &[admitted],
            ),
            Err(StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetArchitecture)
        );
    }
}
