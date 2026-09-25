//! A caller passing a bounded inline byte field of its record parameter as a
//! mutably borrowed view to a unit callee. Selection stages the projected
//! field pointer with an `AddressOffset` over the referent root, then reads
//! the field's live-length word through a `Load64` and forms the
//! descriptor's data pointer with a second `AddressOffset` — two displaced
//! consumers of one producer, the exact surface
//! `SelectedAddressOffsetFoldV1` folds in the pre-allocation slice.

use crate::tests::{
    AdmissionProfile, BindingRelevance, Block, BlockId, ContractId, EdgeId,
    ExplicitOptimizationRequest, MachineContract, MachineId, NativeTarget, Operation, OperationId,
    OperationKind, OperationResult, OptimizationSelections, OptimizationWorkBudget,
    OptimizedTargetLoweringRequest, PlaceId, StagedOptimizedSelectedInstructions, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, Terminator, ValueId, conditional_immediate_module,
    lower_optimized_to_target_operations, operation_proof_bundle, optimize_artifact_sections,
    stage_optimized_instruction_selection,
};

pub(crate) const BYTE_FIELD_CALL_CALLER: u64 = 26_100;
pub(crate) const BYTE_FIELD_CALL_CALLER_ENTRY: u64 = 26_101;
pub(crate) const BYTE_FIELD_CALL_CALLER_PLACE: u64 = 26_102;
pub(crate) const BYTE_FIELD_CALL_CALLER_CONTRACT: u64 = 26_103;
pub(crate) const BYTE_FIELD_CALL_CALLER_CALL: u64 = 26_104;
pub(crate) const BYTE_FIELD_CALL_CALLER_RETURN_EDGE: u64 = 26_105;
pub(crate) const BYTE_FIELD_CALL_CALLEE: u64 = 26_200;
pub(crate) const BYTE_FIELD_CALL_CALLEE_ENTRY: u64 = 26_201;
pub(crate) const BYTE_FIELD_CALL_CALLEE_PLACE: u64 = 26_202;
pub(crate) const BYTE_FIELD_CALL_CALLEE_LENGTH: u64 = 26_203;
pub(crate) const BYTE_FIELD_CALL_CALLEE_OPERATION: u64 = 26_204;
pub(crate) const BYTE_FIELD_CALL_CALLEE_RETURN_EDGE: u64 = 26_205;
pub(crate) const BYTE_FIELD_CALL_CALLEE_CONTRACT: u64 = 26_206;
pub(crate) const BYTE_FIELD_CALL_VIEW_TYPE: u64 = 26_300;
pub(crate) const BYTE_FIELD_CALL_RECORD_TYPE: u64 = 26_301;
pub(crate) const BYTE_FIELD_CALL_FLAG_FIELD: u64 = 26_302;
pub(crate) const BYTE_FIELD_CALL_OUT_FIELD: u64 = 26_303;

/// Caller `record` (`flag: u64` at byte 0, `out: bounded bytes` at byte 8)
/// passes `record.out` as a mutably borrowed view to `measure`, a unit
/// callee that reads the view's live length. The descriptor staging reads
/// the field through the projected pointer — the source-produced fold
/// candidate.
pub(crate) fn byte_field_call_artifact() -> (Vec<u8>, Vec<u8>) {
    let caller = MachineId::new(BYTE_FIELD_CALL_CALLER).unwrap();
    let callee = MachineId::new(BYTE_FIELD_CALL_CALLEE).unwrap();
    let caller_place = PlaceId::new(BYTE_FIELD_CALL_CALLER_PLACE).unwrap();
    let callee_place = PlaceId::new(BYTE_FIELD_CALL_CALLEE_PLACE).unwrap();
    let view = StructuralTypeId::new(BYTE_FIELD_CALL_VIEW_TYPE).unwrap();
    let record = StructuralTypeId::new(BYTE_FIELD_CALL_RECORD_TYPE).unwrap();
    let u64_type = crate::tests::ScalarType::Integer(
        crate::tests::IntegerType::new(crate::tests::IntegerSign::Unsigned, 64).unwrap(),
    );
    let structural_parameter = |place, structural_type, access| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let structural_place = |id| StructuralPlaceDeclaration {
        id,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    };
    let length = ValueId::new(BYTE_FIELD_CALL_CALLEE_LENGTH).unwrap();
    let callee_machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: callee,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(
            callee_place,
            view,
            StructuralAccess::MutableBorrow,
        )],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(callee_place)],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(BYTE_FIELD_CALL_CALLEE_ENTRY).unwrap(),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(BYTE_FIELD_CALL_CALLEE_ENTRY).unwrap(),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(BYTE_FIELD_CALL_CALLEE_OPERATION).unwrap(),
                result: OperationResult::Scalar(crate::tests::ValueDeclaration {
                    qualifications: Default::default(),
                    id: length,
                    scalar_type: u64_type,
                }),
                kind: OperationKind::ByteSequenceLength {
                    source: callee_place,
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(BYTE_FIELD_CALL_CALLEE_RETURN_EDGE).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: ContractId::new(BYTE_FIELD_CALL_CALLEE_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let caller_machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: caller,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(
            caller_place,
            record,
            StructuralAccess::MutableBorrow,
        )],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(caller_place)],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(BYTE_FIELD_CALL_CALLER_ENTRY).unwrap(),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(BYTE_FIELD_CALL_CALLER_ENTRY).unwrap(),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(BYTE_FIELD_CALL_CALLER_CALL).unwrap(),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    callee,
                    arguments: Vec::new(),
                    structural_arguments: vec![terminal_psi::StructuralArgument {
                        place: caller_place,
                        path: vec![terminal_psi::StructuralPathSegment::Field("out".into())],
                        access: StructuralAccess::MutableBorrow,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(BYTE_FIELD_CALL_CALLER_RETURN_EDGE).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: ContractId::new(BYTE_FIELD_CALL_CALLER_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let mut module = conditional_immediate_module(caller, vec![caller_machine, callee_machine]);
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: view,
            identity: "test::AddressFoldBytes".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access: None },
            ),
        },
        StructuralTypeDeclaration {
            id: record,
            identity: "test::AddressFoldPrinter".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(BYTE_FIELD_CALL_FLAG_FIELD).unwrap(),
                        identity: "flag".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(crate::tests::ScalarType::Integer(
                            crate::tests::IntegerType::new(crate::tests::IntegerSign::Unsigned, 64)
                                .unwrap(),
                        )),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(BYTE_FIELD_CALL_OUT_FIELD).unwrap(),
                        identity: "out".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 3 },
                        ),
                    },
                ],
            },
        },
    ];
    let proof = operation_proof_bundle(&module);
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
    )
}

/// The same call shape with the byte view passed as the caller's own root
/// parameter — no field projection. Selection stages the pointer with a
/// `CopyI64`, so every displaced consumer declines the fold: the negative
/// axis's real candidates.
pub(crate) fn byte_view_call_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, _) = byte_field_call_artifact();
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let caller_place = PlaceId::new(BYTE_FIELD_CALL_CALLER_PLACE).unwrap();
    let view = StructuralTypeId::new(BYTE_FIELD_CALL_VIEW_TYPE).unwrap();
    let caller = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    caller.structural_parameters[0].structural_type = view;
    caller.blocks[0].operations[0].kind = match &caller.blocks[0].operations[0].kind {
        OperationKind::CallUnit {
            erased_arguments,
            erased_proof_arguments,
            callee,
            arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } => OperationKind::CallUnit {
            erased_arguments: erased_arguments.clone(),
            erased_proof_arguments: erased_proof_arguments.clone(),
            callee: *callee,
            arguments: arguments.clone(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: caller_place,
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
        _ => panic!("byte-field call fixture must carry the structural unit call"),
    };
    let proof = operation_proof_bundle(&module);
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
    )
}

/// The byte-view call artifact under an authored suite, lowered and selected
/// for `target` with `budget` as the per-pass work budget.
pub(crate) fn staged_byte_view_call_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = byte_view_call_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

/// The byte-field call artifact under an authored suite, lowered and selected
/// for `target` with `budget` as the per-pass work budget.
pub(crate) fn staged_byte_field_call_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = byte_field_call_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
