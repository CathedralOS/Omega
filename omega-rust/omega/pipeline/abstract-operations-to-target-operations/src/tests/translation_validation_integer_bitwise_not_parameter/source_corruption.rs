use super::*;

#[test]
fn integer_bitwise_not_source_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::IntegerBitwiseNot { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceBitwiseNotResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseNot { operand, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *operand = ValueId::new(42_999).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractFunctionResult::Scalar(result) = &mut function.result else {
                unreachable!()
            };
            result.scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(42_998).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 24)),
            });
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceParameterShape
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
                PlaceId::new(42_997).unwrap(),
            ));
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceCleanup
    );
}

#[test]
fn integer_bitwise_not_shared_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(42_910).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(42_911).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(42_912).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(42_913).unwrap(),
                input: PlaceId::new(42_914).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(42_915).unwrap());
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries.clear()),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.parameters.push(function.parameters[0])),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceParameterRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperationRoster
    );
}
