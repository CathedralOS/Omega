use super::*;
use semantic_vocabulary::{ClaimId, ServiceId, StructuralDomainId};
use terminal_psi::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(AbstractParameter {
            value: ValueId::new(64_100).unwrap(),
            scalar_type: ScalarType::Integer(source_type()),
        })),
        StraightLineIntegerWidenImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| function.structural_parameters.push(
            StructuralParameterDeclaration {
                place: PlaceId::new(64_101).unwrap(),
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(64_102).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: vec![StructuralDomainId::new(64_103).unwrap()],
                projected_qualifications: Vec::new(),
            }
        )),
        StraightLineIntegerWidenImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineIntegerWidenImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| function.entry_claims.push(EntryClaim {
            claim: ClaimId::new(64_104).unwrap(),
            input: PlaceId::new(64_105).unwrap(),
            path: Vec::new(),
        })),
        StraightLineIntegerWidenImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| function
            .published_service_ceiling
            .push(ServiceId::new(64_106).unwrap())),
        StraightLineIntegerWidenImmediateTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineIntegerWidenImmediateTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerWidenImmediateTranslationError::SourceOperationRoster
    );
}

#[test]
fn definition_constant_and_widen_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerWiden { psi_operation, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(64_003).unwrap();
        }),
        StraightLineIntegerWidenImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerWiden { result, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *result = ValueId::new(64_004).unwrap();
        }),
        StraightLineIntegerWidenImmediateTranslationError::SourceDefinitionRoster
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
        StraightLineIntegerWidenImmediateTranslationError::SourceConstantType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(65_536);
        }),
        StraightLineIntegerWidenImmediateTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerWiden { operand, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *operand = ValueId::new(64_107).unwrap();
        }),
        StraightLineIntegerWidenImmediateTranslationError::SourceWidenOperand
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerWiden { target_type, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *target_type = source_type();
        }),
        StraightLineIntegerWidenImmediateTranslationError::SourceWidenType
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[2] else {
                unreachable!()
            };
            *value = ValueId::new(64_004).unwrap();
        }),
        StraightLineIntegerWidenImmediateTranslationError::SourceResultLink
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
                PlaceId::new(64_108).unwrap(),
            ));
        }),
        StraightLineIntegerWidenImmediateTranslationError::SourceCleanup
    );
}
