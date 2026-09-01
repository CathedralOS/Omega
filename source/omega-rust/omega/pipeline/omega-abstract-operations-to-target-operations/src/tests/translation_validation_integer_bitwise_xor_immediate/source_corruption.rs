use super::*;
use psi_core::{ClaimId, ServiceId, StructuralDomainId};
use psi_terminal::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(79_100).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type()),
            })
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(79_101).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(79_102).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(79_103).unwrap()],
                    projected_qualifications: Vec::new(),
                })
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(79_104).unwrap(),
                input: PlaceId::new(79_105).unwrap(),
                path: Vec::new(),
            })
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(79_106).unwrap())
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(1, 2)),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceOperationRoster
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
            *psi_operation = OperationId::new(79_003).unwrap();
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseXor { result, .. } = &mut function.operations[2]
            else {
                unreachable!()
            };
            *result = ValueId::new(79_004).unwrap();
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceDefinitionRoster
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
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceConstantType
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
                let AbstractOperation::IntegerBitwiseXor { scalar_type, .. } =
                    &mut function.operations[2]
                else {
                    unreachable!()
                };
                *scalar_type = invalid;
            }),
            StraightLineIntegerBitwiseXorImmediateTranslationError::SourceIntegerType
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
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseXor { left, right, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceBitwiseXorOperands
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *value = ValueId::new(79_004).unwrap();
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                scalar_type: return_type,
                ..
            } = &mut function.operations[3]
            else {
                unreachable!()
            };
            *return_type = ScalarType::Boolean;
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceResultLink
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
                PlaceId::new(79_107).unwrap(),
            ));
        }),
        StraightLineIntegerBitwiseXorImmediateTranslationError::SourceCleanup
    );
}
