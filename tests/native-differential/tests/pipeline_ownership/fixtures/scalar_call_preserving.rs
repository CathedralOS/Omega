//! Ordinary scalar-returning call chain with a parameter and earlier results live across later calls.

use crate::tests::*;

pub(crate) const SCALAR_CALL_PRESERVING_CALLER: u64 = 21_200;
pub(crate) const SCALAR_CALL_PRESERVING_ENTRY: u64 = 21_201;
pub(crate) const SCALAR_CALL_PRESERVING_PARAMETER: u64 = 21_202;
pub(crate) const SCALAR_CALL_PRESERVING_CONSTANT: u64 = 21_203;
pub(crate) const SCALAR_CALL_PRESERVING_FIRST_RESULT: u64 = 21_204;
pub(crate) const SCALAR_CALL_PRESERVING_SECOND_RESULT: u64 = 21_205;
pub(crate) const SCALAR_CALL_PRESERVING_THIRD_RESULT: u64 = 21_206;
pub(crate) const SCALAR_CALL_PRESERVING_RESULT: u64 = 21_207;
pub(crate) const SCALAR_CALL_PRESERVING_CONSTANT_OPERATION: u64 = 21_208;
pub(crate) const SCALAR_CALL_PRESERVING_FIRST_CALL: u64 = 21_209;
pub(crate) const SCALAR_CALL_PRESERVING_SECOND_CALL: u64 = 21_210;
pub(crate) const SCALAR_CALL_PRESERVING_THIRD_CALL: u64 = 21_211;
pub(crate) const SCALAR_CALL_PRESERVING_RETURN_EDGE: u64 = 21_212;
pub(crate) const SCALAR_CALL_PRESERVING_CONTRACT: u64 = 21_213;
pub(crate) const SCALAR_CALL_PRESERVING_CALLEE_BASE: u64 = 21_300;

pub(crate) fn scalar_call_preserving_artifact() -> (Vec<u8>, Vec<u8>) {
    let caller = MachineId::new(SCALAR_CALL_PRESERVING_CALLER).unwrap();
    let entry = BlockId::new(SCALAR_CALL_PRESERVING_ENTRY).unwrap();
    let parameter = ValueId::new(SCALAR_CALL_PRESERVING_PARAMETER).unwrap();
    let constant = ValueId::new(SCALAR_CALL_PRESERVING_CONSTANT).unwrap();
    let first_result = ValueId::new(SCALAR_CALL_PRESERVING_FIRST_RESULT).unwrap();
    let second_result = ValueId::new(SCALAR_CALL_PRESERVING_SECOND_RESULT).unwrap();
    let third_result = ValueId::new(SCALAR_CALL_PRESERVING_THIRD_RESULT).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let callee = conditional_u64_integer_equal_parameters_machine(
        SCALAR_CALL_PRESERVING_CALLEE_BASE,
        [1, 0],
    );
    let call = |id, result, arguments| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(declaration(result)),
        kind: OperationKind::Call {
            callee: callee.id,
            arguments,
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
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(declaration(
            ValueId::new(SCALAR_CALL_PRESERVING_RESULT).unwrap(),
        )),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(SCALAR_CALL_PRESERVING_CONSTANT_OPERATION).unwrap(),
                    result: OperationResult::Scalar(declaration(constant)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(9),
                    },
                },
                call(
                    SCALAR_CALL_PRESERVING_FIRST_CALL,
                    first_result,
                    vec![parameter, constant],
                ),
                call(
                    SCALAR_CALL_PRESERVING_SECOND_CALL,
                    second_result,
                    vec![parameter, constant],
                ),
                call(
                    SCALAR_CALL_PRESERVING_THIRD_CALL,
                    third_result,
                    vec![first_result, second_result],
                ),
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(SCALAR_CALL_PRESERVING_RETURN_EDGE).unwrap(),
                value: third_result,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(SCALAR_CALL_PRESERVING_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let module = conditional_immediate_module(caller, vec![caller_machine, callee]);
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn staged_scalar_call_preserving(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = scalar_call_preserving_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
