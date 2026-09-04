use super::*;

pub(super) fn machine() -> MachineId {
    MachineId::new(57_001).unwrap()
}

pub(super) fn structural_type_id() -> StructuralTypeId {
    StructuralTypeId::new(57_003).unwrap()
}

pub(super) fn place_id() -> PlaceId {
    PlaceId::new(57_004).unwrap()
}

pub(super) fn establishment_operation() -> OperationId {
    OperationId::new(57_005).unwrap()
}

pub(super) fn return_edge() -> EdgeId {
    EdgeId::new(57_006).unwrap()
}

pub(super) fn bytes() -> Vec<u8> {
    vec![0x00, 0x4f, 0x6d, 0x65, 0x67, 0x61, 0xff]
}

pub(super) fn structural_type() -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: structural_type_id(),
        identity: "BorrowedBytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    }
}

pub(super) fn place() -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: place_id(),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: structural_type_id(),
        },
    }
}

pub(super) fn base_plan() -> AbstractOperationPlan {
    let entry = BlockId::new(57_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x57; 32]),
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
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation: establishment_operation(),
                    place: place(),
                    structural_type: structural_type(),
                    bytes: bytes(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: return_edge(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineByteSequenceLiteralUnitReturnTranslationError {
    let mut source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_byte_sequence_literal_unit_return::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineByteSequenceLiteralUnitReturnTranslationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineByteSequenceLiteralUnitReturn,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineByteSequenceLiteralUnitReturn(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("byte-sequence literal corruption must fail at its independent validator")
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
            value: ValueId::new(57_050).unwrap(),
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
        place: PlaceId::new(57_051).unwrap(),
        structural_type: StructuralTypeId::new(57_052).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        projected_qualifications: Vec::new(),
        shape,
        placement,
    }
}
