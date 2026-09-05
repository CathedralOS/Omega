use super::*;
use semantic_vocabulary::{ClaimId, ServiceId, StructuralDomainId};
use terminal_psi::EntryClaim;

#[test]
fn integer_widen_source_links_and_type_relation_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::IntegerWiden { result, .. } = &mut function.operations[0] else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceWidenResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerWiden { operand, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *operand = ValueId::new(44_000).unwrap();
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 16));
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| function.parameters[0].scalar_type = ScalarType::Boolean),
        StraightLineIntegerWidenParameterTranslationError::SourceOperandLink
    );
    for invalid_target in [
        integer_type(IntegerSign::Signed, 16),
        integer_type(IntegerSign::Signed, 8),
        integer_type(IntegerSign::Unsigned, 32),
        integer_type(IntegerSign::Signed, 24),
        IntegerType::address(64).unwrap(),
    ] {
        assert_eq!(
            leaf_error(|function| set_target_type(function, invalid_target)),
            StraightLineIntegerWidenParameterTranslationError::SourceWidenTypeMismatch
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let nonnative = integer_type(IntegerSign::Signed, 24);
            function.parameters[0].scalar_type = ScalarType::Integer(nonnative);
            let AbstractOperation::IntegerWiden { source_type, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *source_type = nonnative;
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceWidenTypeMismatch
    );
}

#[test]
fn integer_widen_return_and_roster_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractFunctionResult::Scalar(result) = &mut function.result else {
                unreachable!()
            };
            result.scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(44_001).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 24)),
            });
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceParameterShape
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(44_002).unwrap(),
            ));
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceCleanup
    );
}

#[test]
fn integer_widen_shared_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineIntegerWidenParameterTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(44_010).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(44_011).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(44_012).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(44_013).unwrap(),
                input: PlaceId::new(44_014).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineIntegerWidenParameterTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(44_015).unwrap());
        }),
        StraightLineIntegerWidenParameterTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries.clear()),
        StraightLineIntegerWidenParameterTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.parameters.push(function.parameters[0])),
        StraightLineIntegerWidenParameterTranslationError::SourceParameterRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerWidenParameterTranslationError::SourceOperationRoster
    );
}

fn set_target_type(function: &mut AbstractFunction, target_type: IntegerType) {
    let AbstractOperation::IntegerWiden {
        target_type: declared,
        ..
    } = &mut function.operations[0]
    else {
        unreachable!()
    };
    *declared = target_type;
    let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(target_type);
    let AbstractFunctionResult::Scalar(result) = &mut function.result else {
        unreachable!()
    };
    result.scalar_type = ScalarType::Integer(target_type);
}
