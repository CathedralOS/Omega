use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance = TerminalPsiProvenance::default()),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetProvenance
    );
}

#[test]
fn target_outer_operation_and_obligation_corruption_fail_closed() {
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { psi_edge, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(85_120).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { source_value, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *source_value = ValueId::new(85_121).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { scalar_type, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *scalar_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::SaturatingDivide { psi_operation, .. } = expression else {
                unreachable!()
            };
            *psi_operation = OperationId::new(85_122).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::SaturatingDivide { obligation, .. } = expression else {
                unreachable!()
            };
            *obligation = ObligationId::new(85_123).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].operation =
            TargetOperation::ReturnIntegerImmediate {
                psi_edge: EdgeId::new(85_009).unwrap(),
                source_value: ValueId::new(85_008).unwrap(),
                scalar_type: scalar_type(),
                value: IntegerValue::Signed(32_767),
            }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
}

#[test]
fn target_immediate_children_and_order_corruption_fail_closed() {
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::SaturatingDivide { left, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Immediate { source_value, .. } = left.as_mut() else {
                unreachable!()
            };
            *source_value = ValueId::new(85_124).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::SaturatingDivide { left, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Immediate { value, .. } = left.as_mut() else {
                unreachable!()
            };
            *value = IntegerValue::Signed(1);
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::SaturatingDivide { right, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Immediate { source_value, .. } = right.as_mut() else {
                unreachable!()
            };
            *source_value = ValueId::new(85_125).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::SaturatingDivide { right, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Immediate { value, .. } = right.as_mut() else {
                unreachable!()
            };
            *value = IntegerValue::Signed(1);
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::SaturatingDivide { left, right, .. } = expression else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
}

#[test]
fn every_adjacent_division_and_remainder_expression_fails_closed() {
    let children = || {
        (
            Box::new(TargetIntegerExpression::Immediate {
                source_value: ValueId::new(85_004).unwrap(),
                value: IntegerValue::Signed(-32_768),
            }),
            Box::new(TargetIntegerExpression::Immediate {
                source_value: ValueId::new(85_006).unwrap(),
                value: IntegerValue::Signed(-1),
            }),
        )
    };
    let operation = OperationId::new(85_007).unwrap();
    let obligation = ObligationId::new(85_011).unwrap();
    let (left, right) = children();
    let exact_divide = TargetIntegerExpression::ExactDivide {
        psi_operation: operation,
        obligation,
        left,
        right,
    };
    let (left, right) = children();
    let wrapping_divide = TargetIntegerExpression::WrappingDivide {
        psi_operation: operation,
        obligation,
        left,
        right,
    };
    let (left, right) = children();
    let wrapping_remainder = TargetIntegerExpression::WrappingRemainder {
        psi_operation: operation,
        obligation,
        left,
        right,
    };
    let (left, right) = children();
    let exact_remainder = TargetIntegerExpression::ExactRemainder {
        psi_operation: operation,
        obligation,
        left,
        right,
    };
    let (left, right) = children();
    let saturating_remainder = TargetIntegerExpression::SaturatingRemainder {
        psi_operation: operation,
        obligation,
        left,
        right,
    };
    for replacement in [
        exact_divide,
        wrapping_divide,
        wrapping_remainder,
        exact_remainder,
        saturating_remainder,
    ] {
        assert_eq!(
            candidate_error(|target| {
                let TargetOperation::ReturnIntegerExpression { expression, .. } =
                    &mut target.functions[0].operation
                else {
                    unreachable!()
                };
                *expression = replacement;
            }),
            StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
        );
    }
}
