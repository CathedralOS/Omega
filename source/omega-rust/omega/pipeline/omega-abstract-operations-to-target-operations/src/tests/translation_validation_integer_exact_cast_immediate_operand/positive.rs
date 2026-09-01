use super::*;

#[test]
fn validates_every_native_exact_cast_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (source_type, target_type) in legal_native_exact_casts() {
            for source_value in representable_boundary_values(source_type, target_type) {
                let source = base_plan(source_type, target_type, source_value);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerExactCastImmediateOperand(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("constant exact cast must publish only its proof-bearing immediate-operand family")
                };
                assert_eq!(row.machine(), machine());
                assert_eq!(row.constant_operation(), OperationId::new(66_003).unwrap());
                assert_eq!(row.cast_operation(), OperationId::new(66_005).unwrap());
                assert_eq!(row.obligation(), ObligationId::new(66_009).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(66_007).unwrap());
                assert_eq!(row.constant_result(), ValueId::new(66_004).unwrap());
                assert_eq!(row.cast_result(), ValueId::new(66_006).unwrap());
                assert_eq!(row.source_type(), source_type);
                assert_eq!(row.target_type(), target_type);
                assert_eq!(row.source_value(), source_value);
                assert_eq!(
                    row.cast_value(),
                    source_type
                        .exact_cast_value_to(target_type, source_value)
                        .unwrap()
                );
                assert!(matches!(
                    &target.functions[0].operation,
                    TargetOperation::ReturnIntegerExpression {
                        scalar_type,
                        expression: TargetIntegerExpression::IntegerExactCast {
                            obligation,
                            source_type: target_source_type,
                            operand,
                            ..
                        },
                        ..
                    } if *scalar_type == target_type
                        && *obligation == ObligationId::new(66_009).unwrap()
                        && *target_source_type == source_type
                        && matches!(
                            operand.as_ref(),
                            TargetIntegerExpression::Immediate { source_value: value_id, value }
                                if *value_id == ValueId::new(66_004).unwrap()
                                    && *value == source_value
                        )
                ));
            }
        }
    }
}

#[test]
fn classifier_is_disjoint_from_plain_immediate_and_parameter_exact_cast() {
    let target_profile = NativeTarget::linux_x64();
    let mut plain = default_plan();
    plain.functions[0].operations.remove(1);
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut plain.functions[0].operations[1]
    else {
        unreachable!()
    };
    *value = ValueId::new(66_004).unwrap();
    *scalar_type = ScalarType::Integer(source_type());
    let AbstractFunctionResult::Scalar(result) = &mut plain.functions[0].result else {
        unreachable!()
    };
    result.scalar_type = ScalarType::Integer(source_type());
    let target = lower_to_target_operations(&plain, target_profile).unwrap();
    let receipt = validate_abstract_to_target_translation(&plain, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(_)
        )
    ));

    assert!(
        !crate::validation::straight_line_integer_exact_cast_immediate_operand::is_candidate(
            &super::super::parameter_translation_fixture::uniform_integer_exact_cast_plan(
                source_type(),
                target_type(),
                1,
            )
            .functions[0]
        )
    );
}

fn legal_native_exact_casts() -> Vec<(IntegerType, IntegerType)> {
    let integer = |sign, bits| IntegerType::new(sign, bits).unwrap();
    let integers = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| integer(sign, bits)))
        .collect::<Vec<_>>();
    let pairs = integers
        .iter()
        .flat_map(|source| integers.iter().map(move |target| (*source, *target)))
        .filter(|(source, target)| {
            source != target && !source.can_widen_to(*target) && source.can_exact_cast_to(*target)
        })
        .collect::<Vec<_>>();
    assert_eq!(pairs.len(), 38);
    pairs
}

fn representable_boundary_values(source: IntegerType, target: IntegerType) -> Vec<IntegerValue> {
    let mut values = vec![source.minimum_value(), source.maximum_value()];
    for target_boundary in [target.minimum_value(), target.maximum_value()] {
        if let Some(value) = target.exact_cast_value_to(source, target_boundary) {
            values.push(value);
        }
    }
    values.retain(|value| source.exact_cast_value_to(target, *value).is_some());
    values.sort();
    values.dedup();
    assert!(!values.is_empty());
    values
}
