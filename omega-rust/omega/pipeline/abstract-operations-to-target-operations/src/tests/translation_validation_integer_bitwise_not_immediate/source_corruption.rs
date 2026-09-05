use super::*;
use semantic_vocabulary::{ClaimId, ServiceId, StructuralDomainId};
use terminal_psi::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(67_100).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type()),
            })
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(67_101).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(67_102).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(67_103).unwrap()],
                    projected_qualifications: Vec::new(),
                })
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(67_104).unwrap(),
                input: PlaceId::new(67_105).unwrap(),
                path: Vec::new(),
            })
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(67_106).unwrap())
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceOperationRoster
    );
}

#[test]
fn definition_constant_and_bitwise_not_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseNot { psi_operation, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(67_003).unwrap();
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseNot { result, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *result = ValueId::new(67_004).unwrap();
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceConstantType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(65_536);
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseNot { operand, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *operand = ValueId::new(67_107).unwrap();
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceBitwiseNotOperand
    );
    for invalid in [
        IntegerType::new(IntegerSign::Unsigned, 24).unwrap(),
        IntegerType::address(32).unwrap(),
    ] {
        assert_eq!(
            leaf_error(|function| {
                let AbstractOperation::IntegerConstant { scalar_type, .. } =
                    &mut function.operations[0]
                else {
                    unreachable!()
                };
                *scalar_type = ScalarType::Integer(invalid);
                let AbstractOperation::IntegerBitwiseNot { scalar_type, .. } =
                    &mut function.operations[1]
                else {
                    unreachable!()
                };
                *scalar_type = invalid;
                let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[2]
                else {
                    unreachable!()
                };
                *scalar_type = ScalarType::Integer(invalid);
                let AbstractFunctionResult::Scalar(result) = &mut function.result else {
                    unreachable!()
                };
                result.scalar_type = ScalarType::Integer(invalid);
            }),
            StraightLineIntegerBitwiseNotImmediateTranslationError::SourceBitwiseNotType
        );
    }
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[2] else {
                unreachable!()
            };
            *value = ValueId::new(67_004).unwrap();
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[2]
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(67_108).unwrap(),
            ));
        }),
        StraightLineIntegerBitwiseNotImmediateTranslationError::SourceCleanup
    );
}
