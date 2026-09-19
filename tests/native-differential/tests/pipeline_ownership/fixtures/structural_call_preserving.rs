//! Scalar-result call chain carrying a borrowed structural argument live across
//! every call, with the caller parameter live across the first call and the
//! first call's result live across the second call.

use crate::tests::{
    AdmissionProfile, BindingRelevance, Block, BlockId, ContractId, EdgeId, IntegerSign,
    IntegerType, IntegerValue, MachineContract, MachineId, NativeTarget, Operation, OperationId,
    OperationKind, OperationResult, Optimization, OptimizationSelections,
    OptimizedTargetLoweringRequest, PlaceId, ProofBundle, ScalarType,
    StagedOptimizedSelectedInstructions, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldId, StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, Terminator, ValueDeclaration,
    ValueId, conditional_immediate_module, conditional_u64_integer_equal_parameters_machine,
    lower_optimized_to_target_operations, optimize_artifact_sections, request,
    stage_optimized_instruction_selection,
};
pub(crate) const STRUCTURAL_CALL_PRESERVING_CALLER: u64 = 22_200;
pub(crate) const STRUCTURAL_CALL_PRESERVING_ENTRY: u64 = 22_201;
pub(crate) const STRUCTURAL_CALL_PRESERVING_PARAMETER: u64 = 22_202;
pub(crate) const STRUCTURAL_CALL_PRESERVING_CONSTANT: u64 = 22_203;
pub(crate) const STRUCTURAL_CALL_PRESERVING_SECOND_CONSTANT: u64 = 22_216;
pub(crate) const STRUCTURAL_CALL_PRESERVING_SECOND_CONSTANT_OPERATION: u64 = 22_217;
pub(crate) const STRUCTURAL_CALL_PRESERVING_FIRST_RESULT: u64 = 22_204;
pub(crate) const STRUCTURAL_CALL_PRESERVING_SECOND_RESULT: u64 = 22_205;
pub(crate) const STRUCTURAL_CALL_PRESERVING_THIRD_RESULT: u64 = 22_206;
pub(crate) const STRUCTURAL_CALL_PRESERVING_RESULT: u64 = 22_207;
pub(crate) const STRUCTURAL_CALL_PRESERVING_CONSTANT_OPERATION: u64 = 22_208;
pub(crate) const STRUCTURAL_CALL_PRESERVING_FIRST_CALL: u64 = 22_209;
pub(crate) const STRUCTURAL_CALL_PRESERVING_SECOND_CALL: u64 = 22_210;
pub(crate) const STRUCTURAL_CALL_PRESERVING_THIRD_CALL: u64 = 22_211;
pub(crate) const STRUCTURAL_CALL_PRESERVING_RETURN_EDGE: u64 = 22_212;
pub(crate) const STRUCTURAL_CALL_PRESERVING_CONTRACT: u64 = 22_213;
pub(crate) const STRUCTURAL_CALL_PRESERVING_EXTENT_PLACE: u64 = 22_214;
pub(crate) const STRUCTURAL_CALL_PRESERVING_CALLEE_EXTENT_PLACE: u64 = 22_215;
pub(crate) const STRUCTURAL_CALL_PRESERVING_CALLEE_BASE: u64 = 22_300;
pub(crate) const STRUCTURAL_CALL_PRESERVING_EXTENT_TYPE: u64 = 22_400;

pub(crate) fn structural_call_preserving_artifact() -> (Vec<u8>, Vec<u8>) {
    let caller = MachineId::new(STRUCTURAL_CALL_PRESERVING_CALLER).unwrap();
    let entry = BlockId::new(STRUCTURAL_CALL_PRESERVING_ENTRY).unwrap();
    let parameter = ValueId::new(STRUCTURAL_CALL_PRESERVING_PARAMETER).unwrap();
    let constant = ValueId::new(STRUCTURAL_CALL_PRESERVING_CONSTANT).unwrap();
    let second_constant = ValueId::new(STRUCTURAL_CALL_PRESERVING_SECOND_CONSTANT).unwrap();
    let first_result = ValueId::new(STRUCTURAL_CALL_PRESERVING_FIRST_RESULT).unwrap();
    let second_result = ValueId::new(STRUCTURAL_CALL_PRESERVING_SECOND_RESULT).unwrap();
    let third_result = ValueId::new(STRUCTURAL_CALL_PRESERVING_THIRD_RESULT).unwrap();
    let extent = StructuralTypeId::new(STRUCTURAL_CALL_PRESERVING_EXTENT_TYPE).unwrap();
    let caller_extent = PlaceId::new(STRUCTURAL_CALL_PRESERVING_EXTENT_PLACE).unwrap();
    let callee_extent = PlaceId::new(STRUCTURAL_CALL_PRESERVING_CALLEE_EXTENT_PLACE).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let structural_parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: extent,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
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
    let mut callee = conditional_u64_integer_equal_parameters_machine(
        STRUCTURAL_CALL_PRESERVING_CALLEE_BASE,
        [1, 0],
    );
    callee.structural_parameters = vec![structural_parameter(callee_extent)];
    callee.structural_places = vec![structural_place(callee_extent)];
    let call = |id, result, arguments| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(declaration(result)),
        kind: OperationKind::CallStructuralScalar {
            erased_arguments: Vec::new(),
            callee: callee.id,
            arguments,
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: caller_extent,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    let caller_machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: caller,
        attachment: None,
        parameters: vec![declaration(parameter)],
        structural_parameters: vec![structural_parameter(caller_extent)],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(declaration(
            ValueId::new(STRUCTURAL_CALL_PRESERVING_RESULT).unwrap(),
        )),
        structural_places: vec![structural_place(caller_extent)],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(STRUCTURAL_CALL_PRESERVING_CONSTANT_OPERATION).unwrap(),
                    result: OperationResult::Scalar(declaration(constant)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(9),
                    },
                },
                call(
                    STRUCTURAL_CALL_PRESERVING_FIRST_CALL,
                    first_result,
                    vec![parameter, constant],
                ),
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(STRUCTURAL_CALL_PRESERVING_SECOND_CONSTANT_OPERATION)
                        .unwrap(),
                    result: OperationResult::Scalar(declaration(second_constant)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                },
                call(
                    STRUCTURAL_CALL_PRESERVING_SECOND_CALL,
                    second_result,
                    vec![parameter, second_constant],
                ),
                call(
                    STRUCTURAL_CALL_PRESERVING_THIRD_CALL,
                    third_result,
                    vec![first_result, second_result],
                ),
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(STRUCTURAL_CALL_PRESERVING_RETURN_EDGE).unwrap(),
                value: third_result,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(STRUCTURAL_CALL_PRESERVING_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let mut module = conditional_immediate_module(caller, vec![caller_machine, callee]);
    module.structural_types.push(StructuralTypeDeclaration {
        id: extent,
        identity: "test::StructuralCallExtent".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "base".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                },
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: "length".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                },
            ],
        },
    });
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
    )
}

pub(crate) fn staged_structural_call_preserving(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = structural_call_preserving_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
