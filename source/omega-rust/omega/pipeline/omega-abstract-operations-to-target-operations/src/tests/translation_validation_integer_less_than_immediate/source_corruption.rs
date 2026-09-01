use super::*;
use psi_core::{ClaimId, ServiceId, StructuralDomainId};
use psi_terminal::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(71_100).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type()),
            })
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(71_101).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(71_102).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(71_103).unwrap()],
                    projected_qualifications: Vec::new(),
                })
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineIntegerLessThanImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(71_104).unwrap(),
                input: PlaceId::new(71_105).unwrap(),
                path: Vec::new(),
            })
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(71_106).unwrap())
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineIntegerLessThanImmediateTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(1, 2)),
        StraightLineIntegerLessThanImmediateTranslationError::SourceOperationRoster
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
            *psi_operation = OperationId::new(71_003).unwrap();
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerLessThan { result, .. } = &mut function.operations[2]
            else {
                unreachable!()
            };
            *result = ValueId::new(71_004).unwrap();
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceDefinitionRoster
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
        StraightLineIntegerLessThanImmediateTranslationError::SourceConstantType
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
            }),
            StraightLineIntegerLessThanImmediateTranslationError::SourceIntegerType
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
        StraightLineIntegerLessThanImmediateTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerLessThan { left, right, .. } =
                &mut function.operations[2]
            else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceLessThanOperands
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *value = ValueId::new(71_004).unwrap();
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceResultLink
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
            *return_type = ScalarType::Integer(scalar_type());
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceResultLink
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
                PlaceId::new(71_107).unwrap(),
            ));
        }),
        StraightLineIntegerLessThanImmediateTranslationError::SourceCleanup
    );
}
