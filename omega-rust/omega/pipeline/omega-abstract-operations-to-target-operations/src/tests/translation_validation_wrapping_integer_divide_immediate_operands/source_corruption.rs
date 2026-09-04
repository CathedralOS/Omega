use super::*;
use psi_core::{ClaimId, ServiceId, StructuralDomainId};
use psi_terminal::EntryClaim;

#[test]
fn envelope_and_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(AbstractParameter {
            value: ValueId::new(84_100).unwrap(),
            scalar_type: ScalarType::Integer(scalar_type()),
        })),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.structural_parameters.push(StructuralParameterDeclaration {
                place: PlaceId::new(84_101).unwrap(),
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(84_102).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: vec![StructuralDomainId::new(84_103).unwrap()],
                projected_qualifications: Vec::new(),
            })
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| function.entry_claims.push(EntryClaim {
            claim: ClaimId::new(84_104).unwrap(),
            input: PlaceId::new(84_105).unwrap(),
            path: Vec::new(),
        })),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| function
            .published_service_ceiling
            .push(ServiceId::new(84_106).unwrap())),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(1, 2)),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceOperationRoster
    );
}

#[test]
fn definition_type_value_obligation_and_operand_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { psi_operation, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(84_003).unwrap();
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerDivide { result, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            *result = ValueId::new(84_004).unwrap();
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap());
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceConstantType
    );
    for invalid in [
        IntegerType::new(IntegerSign::Signed, 24).unwrap(),
        IntegerType::address(64).unwrap(),
    ] {
        assert_eq!(
            leaf_error(|function| {
                for operation in &mut function.operations[0..2] {
                    let AbstractOperation::IntegerConstant {
                        scalar_type, value, ..
                    } = operation
                    else {
                        unreachable!()
                    };
                    *scalar_type = ScalarType::Integer(invalid);
                    *value = invalid.maximum_value();
                }
                let AbstractOperation::WrappingIntegerDivide { scalar_type, .. } =
                    &mut function.operations[2]
                else {
                    unreachable!()
                };
                *scalar_type = invalid;
            }),
            StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceIntegerType
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[0] else { unreachable!() };
            *value = IntegerValue::Signed(32_768);
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *value = IntegerValue::Signed(0);
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceDivideUndefined
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerDivide { left, right, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceDivideOperands
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerDivide { obligation, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(84_111).unwrap();
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *value = ValueId::new(84_004).unwrap();
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceResultLink
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
                PlaceId::new(84_107).unwrap(),
            ));
        }),
        StraightLineWrappingIntegerDivideImmediateOperandsTranslationError::SourceCleanup
    );
}
