use super::*;
use psi_core::{ClaimId, ServiceId, StructuralDomainId};
use psi_terminal::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(82_100).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type()),
            })
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(82_101).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(82_102).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(82_103).unwrap()],
                    projected_qualifications: Vec::new(),
                })
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(82_104).unwrap(),
                input: PlaceId::new(82_105).unwrap(),
                path: Vec::new(),
            })
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(82_106).unwrap())
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(1, 2)),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceOperationRoster
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
            *psi_operation = OperationId::new(82_003).unwrap();
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::SaturatingIntegerMultiply { result, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            *result = ValueId::new(82_004).unwrap();
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceConstantType
    );
    for invalid in [
        IntegerType::new(IntegerSign::Unsigned, 24).unwrap(),
        IntegerType::address(32).unwrap(),
    ] {
        assert_eq!(
            leaf_error(|function| {
                for operation in &mut function.operations[..2] {
                    let AbstractOperation::IntegerConstant { scalar_type, .. } = operation else {
                        unreachable!()
                    };
                    *scalar_type = ScalarType::Integer(invalid);
                }
                let AbstractOperation::SaturatingIntegerMultiply { scalar_type, .. } =
                    &mut function.operations[2]
                else {
                    unreachable!()
                };
                *scalar_type = invalid;
            }),
            StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceIntegerType
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
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::SaturatingIntegerMultiply { left, right, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceSaturatingMultiplyOperands
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *value = ValueId::new(82_004).unwrap();
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceResultLink
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
                PlaceId::new(82_107).unwrap(),
            ));
        }),
        StraightLineSaturatingIntegerMultiplyImmediateTranslationError::SourceCleanup
    );
}
