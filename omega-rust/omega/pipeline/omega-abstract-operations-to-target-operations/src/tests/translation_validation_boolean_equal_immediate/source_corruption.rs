use super::*;
use psi_core::{ClaimId, ServiceId, StructuralDomainId};
use psi_terminal::EntryClaim;

#[test]
fn envelope_and_operation_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(69_100).unwrap(),
                scalar_type: ScalarType::Boolean,
            })
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(69_101).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(69_102).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(69_103).unwrap()],
                    projected_qualifications: Vec::new(),
                })
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineBooleanEqualImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(69_104).unwrap(),
                input: PlaceId::new(69_105).unwrap(),
                path: Vec::new(),
            })
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(69_106).unwrap())
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries[0].operation_offset = 1),
        StraightLineBooleanEqualImmediateTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(1, 2)),
        StraightLineBooleanEqualImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations.pop();
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceOperationRoster
    );
}

#[test]
fn definition_and_operand_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanConstant { psi_operation, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(69_003).unwrap();
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanConstant { result, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *result = ValueId::new(69_004).unwrap();
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanEqual { result, .. } = &mut function.operations[2] else {
                unreachable!()
            };
            *result = ValueId::new(69_004).unwrap();
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractFunctionResult::Scalar(result) = &mut function.result else {
                unreachable!()
            };
            result.value = ValueId::new(69_004).unwrap();
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceDefinitionRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanEqual { left, .. } = &mut function.operations[2] else {
                unreachable!()
            };
            *left = ValueId::new(69_106).unwrap();
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceEqualOperands
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanEqual { left, right, .. } = &mut function.operations[2]
            else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceEqualOperands
    );
}

#[test]
fn return_and_cleanup_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *value = ValueId::new(69_004).unwrap();
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { result, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *result = ValueId::new(69_108).unwrap();
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[3] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceResultLink
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
                PlaceId::new(69_107).unwrap(),
            ));
        }),
        StraightLineBooleanEqualImmediateTranslationError::SourceCleanup
    );
}
