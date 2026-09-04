use super::*;
use psi_core::{ClaimId, ServiceId, StructuralDomainId};
use psi_terminal::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(AbstractParameter {
            value: ValueId::new(83_100).unwrap(),
            scalar_type: ScalarType::Integer(value_type()),
        })),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(83_101).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(83_102).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(83_103).unwrap()],
                    projected_qualifications: Vec::new(),
                })
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| function.entry_claims.push(EntryClaim {
            claim: ClaimId::new(83_104).unwrap(),
            input: PlaceId::new(83_105).unwrap(),
            path: Vec::new(),
        })),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(83_106).unwrap())
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(1, 2)),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceOperationRoster
    );
}

#[test]
fn definition_type_value_and_operand_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { psi_operation, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(83_003).unwrap();
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerShiftLeft { result, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            *result = ValueId::new(83_004).unwrap();
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceDefinitionRoster
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
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceValueConstantType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap());
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceCountConstantType
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
                let AbstractOperation::WrappingIntegerShiftLeft { value_type, .. } =
                    &mut function.operations[2]
                else {
                    unreachable!()
                };
                *value_type = invalid;
            }),
            StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceValueType
        );
        assert_eq!(
            leaf_error(|function| {
                let AbstractOperation::IntegerConstant { scalar_type, .. } =
                    &mut function.operations[1]
                else {
                    unreachable!()
                };
                *scalar_type = ScalarType::Integer(invalid);
                let AbstractOperation::WrappingIntegerShiftLeft { count_type, .. } =
                    &mut function.operations[2]
                else {
                    unreachable!()
                };
                *count_type = invalid;
            }),
            StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceCountType
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(65_536);
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceValueOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(256);
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceCountOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerShiftLeft { value, count, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            std::mem::swap(value, count);
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceWrappingShiftOperands
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *value = ValueId::new(83_004).unwrap();
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[3]
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(83_107).unwrap(),
            ));
        }),
        StraightLineWrappingIntegerShiftLeftImmediateTranslationError::SourceCleanup
    );
}
