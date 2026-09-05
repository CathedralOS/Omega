use super::*;
use semantic_vocabulary::{ClaimId, ServiceId, StructuralDomainId};
use terminal_psi::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(AbstractParameter {
            value: ValueId::new(66_100).unwrap(),
            scalar_type: ScalarType::Integer(source_type()),
        })),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| function.structural_parameters.push(
            StructuralParameterDeclaration {
                place: PlaceId::new(66_101).unwrap(),
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(66_102).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: vec![StructuralDomainId::new(66_103).unwrap()],
                projected_qualifications: Vec::new(),
            }
        )),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| function.entry_claims.push(EntryClaim {
            claim: ClaimId::new(66_104).unwrap(),
            input: PlaceId::new(66_105).unwrap(),
            path: Vec::new(),
        })),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| function
            .published_service_ceiling
            .push(ServiceId::new(66_106).unwrap())),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceOperationRoster
    );
}

#[test]
fn definition_constant_and_cast_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerExactCast { psi_operation, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(66_003).unwrap();
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerExactCast { result, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *result = ValueId::new(66_004).unwrap();
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceDefinitionRoster
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
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceConstantType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(65_536);
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerExactCast { operand, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *operand = ValueId::new(66_107).unwrap();
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceCastOperand
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerExactCast { target_type, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *target_type = source_type();
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceCastType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(256);
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceCastValueOutsideTarget
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[2] else {
                unreachable!()
            };
            *value = ValueId::new(66_004).unwrap();
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[2]
            else {
                unreachable!()
            };
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(66_108).unwrap(),
            ));
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::SourceCleanup
    );
}
