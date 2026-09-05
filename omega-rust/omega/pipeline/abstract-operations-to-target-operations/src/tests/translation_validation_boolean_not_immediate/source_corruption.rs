use super::*;
use semantic_vocabulary::{ClaimId, ServiceId, StructuralDomainId};
use terminal_psi::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(68_100).unwrap(),
                scalar_type: ScalarType::Boolean,
            })
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(68_101).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(68_102).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(68_103).unwrap()],
                    projected_qualifications: Vec::new(),
                })
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineBooleanNotImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(68_104).unwrap(),
                input: PlaceId::new(68_105).unwrap(),
                path: Vec::new(),
            })
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(68_106).unwrap())
        }),
        StraightLineBooleanNotImmediateTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineBooleanNotImmediateTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineBooleanNotImmediateTranslationError::SourceOperationRoster
    );
}

#[test]
fn definition_and_operand_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanNot { psi_operation, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(68_003).unwrap();
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanNot { result, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *result = ValueId::new(68_004).unwrap();
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanNot { operand, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *operand = ValueId::new(68_107).unwrap();
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceBooleanNotOperand
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[2] else {
                unreachable!()
            };
            *value = ValueId::new(68_004).unwrap();
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[2] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceResultLink
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
                PlaceId::new(68_108).unwrap(),
            ));
        }),
        StraightLineBooleanNotImmediateTranslationError::SourceCleanup
    );
}
