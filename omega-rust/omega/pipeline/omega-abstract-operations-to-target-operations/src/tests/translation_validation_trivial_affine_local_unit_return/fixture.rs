use super::*;

pub(super) fn machine() -> MachineId {
    MachineId::new(56_001).unwrap()
}

pub(super) fn structural_type_id() -> StructuralTypeId {
    StructuralTypeId::new(56_003).unwrap()
}

pub(super) fn place_id() -> PlaceId {
    PlaceId::new(56_004).unwrap()
}

pub(super) fn establishment_operation() -> OperationId {
    OperationId::new(56_005).unwrap()
}

pub(super) fn return_edge() -> EdgeId {
    EdgeId::new(56_006).unwrap()
}

pub(super) fn structural_type() -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: structural_type_id(),
        identity: "TrivialAffineToken".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    }
}

pub(super) fn place() -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: place_id(),
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type: structural_type_id(),
            construction: None,
        },
    }
}

pub(super) fn base_plan() -> AbstractOperationPlan {
    let entry = BlockId::new(56_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x56; 32]),
        },
        entry: machine(),
        structural_types: vec![structural_type()],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine: machine(),
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::EstablishTrivialAffineLocal {
                    psi_operation: establishment_operation(),
                    place: place(),
                    structural_type: structural_type(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: return_edge(),
                    cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id())],
                },
            ],
        }],
    }
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineTrivialAffineLocalUnitReturnTranslationError {
    let mut source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_trivial_affine_local_unit_return::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineTrivialAffineLocalUnitReturnTranslationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineTrivialAffineLocalUnitReturn,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineTrivialAffineLocalUnitReturn(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("trivial affine-local corruption must fail at its independent validator")
    };
    error
}

pub(super) fn fixed_integer_scalar_abi(target: NativeTarget) -> FixedIntegerScalarFunctionAbi {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .unwrap();
    FixedIntegerScalarFunctionAbi {
        result: FixedIntegerScalarAbiValue {
            value: ValueId::new(56_050).unwrap(),
            scalar_type,
            placement: call_plan.result.clone().unwrap(),
        },
        parameters: Vec::new(),
        call_plan,
    }
}

pub(super) fn target_structural_parameter() -> TargetStructuralParameter {
    let shape = ValueShape::integer(4, 4);
    let placement = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap()
    .parameters
    .remove(0);
    TargetStructuralParameter {
        place: PlaceId::new(56_051).unwrap(),
        structural_type: StructuralTypeId::new(56_052).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        projected_qualifications: Vec::new(),
        shape,
        placement,
    }
}
