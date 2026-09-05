use super::*;
use semantic_vocabulary::{ClaimId, ServiceId, StructuralDomainId};
use terminal_psi::EntryClaim;

#[test]
fn envelope_and_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(AbstractParameter {
            value: ValueId::new(85_100).unwrap(),
            scalar_type: ScalarType::Integer(scalar_type()),
        })),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.structural_parameters.push(StructuralParameterDeclaration {
                place: PlaceId::new(85_101).unwrap(),
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(85_102).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: vec![StructuralDomainId::new(85_103).unwrap()],
                projected_qualifications: Vec::new(),
            })
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| function.entry_claims.push(EntryClaim {
            claim: ClaimId::new(85_104).unwrap(),
            input: PlaceId::new(85_105).unwrap(),
            path: Vec::new(),
        })),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| function.published_service_ceiling.push(ServiceId::new(85_106).unwrap())),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(1, 2)),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceOperationRoster
    );
}

#[test]
fn definition_type_value_obligation_and_operand_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { psi_operation, .. } = &mut function.operations[1] else { unreachable!() };
            *psi_operation = OperationId::new(85_003).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::SaturatingIntegerDivide { result, .. } = &mut function.operations[2] else { unreachable!() };
            *result = ValueId::new(85_004).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceDefinitionRoster
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
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceConstantType
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
                let AbstractOperation::SaturatingIntegerDivide { scalar_type, .. } =
                    &mut function.operations[2]
                else {
                    unreachable!()
                };
                *scalar_type = invalid;
            }),
            StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceIntegerType
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[0] else { unreachable!() };
            *value = IntegerValue::Signed(32_768);
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *value = IntegerValue::Signed(0);
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceDivideUndefined
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::SaturatingIntegerDivide { left, right, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceDivideOperands
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::SaturatingIntegerDivide { obligation, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(85_111).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::TargetOperation
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *value = ValueId::new(85_004).unwrap();
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceResultLink
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
                PlaceId::new(85_107).unwrap(),
            ));
        }),
        StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError::SourceCleanup
    );
}
