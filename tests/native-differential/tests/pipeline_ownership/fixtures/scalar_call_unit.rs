//! Exact attached-Unit scalar-call chain shared by selection and allocation tests.

use crate::tests::*;

pub(crate) const SCALAR_CALL_UNIT_CALLER: u64 = 21_001;
pub(crate) const SCALAR_CALL_UNIT_ATTACHMENT: u64 = 21_002;
pub(crate) const SCALAR_CALL_UNIT_ENTRY: u64 = 21_003;
pub(crate) const SCALAR_CALL_UNIT_LEFT: u64 = 21_004;
pub(crate) const SCALAR_CALL_UNIT_RIGHT: u64 = 21_005;
pub(crate) const SCALAR_CALL_UNIT_FIRST_RESULT: u64 = 21_006;
pub(crate) const SCALAR_CALL_UNIT_SECOND_RESULT: u64 = 21_007;
pub(crate) const SCALAR_CALL_UNIT_THIRD_RESULT: u64 = 21_008;
pub(crate) const SCALAR_CALL_UNIT_LEFT_OPERATION: u64 = 21_009;
pub(crate) const SCALAR_CALL_UNIT_RIGHT_OPERATION: u64 = 21_010;
pub(crate) const SCALAR_CALL_UNIT_FIRST_CALL: u64 = 21_011;
pub(crate) const SCALAR_CALL_UNIT_SECOND_CALL: u64 = 21_012;
pub(crate) const SCALAR_CALL_UNIT_THIRD_CALL: u64 = 21_013;
pub(crate) const SCALAR_CALL_UNIT_RETURN_EDGE: u64 = 21_014;
pub(crate) const SCALAR_CALL_UNIT_CALLEE_BASE: u64 = 21_100;

pub(crate) fn scalar_call_unit_artifact() -> (Vec<u8>, Vec<u8>) {
    scalar_call_unit_artifact_with(|_| {})
}

pub(crate) fn scalar_call_unit_artifact_with(
    edit: impl FnOnce(&mut TerminalModule),
) -> (Vec<u8>, Vec<u8>) {
    let caller = MachineId::new(SCALAR_CALL_UNIT_CALLER).unwrap();
    let attachment = StructuralTypeId::new(SCALAR_CALL_UNIT_ATTACHMENT).unwrap();
    let entry = BlockId::new(SCALAR_CALL_UNIT_ENTRY).unwrap();
    let left = ValueId::new(SCALAR_CALL_UNIT_LEFT).unwrap();
    let right = ValueId::new(SCALAR_CALL_UNIT_RIGHT).unwrap();
    let first_result = ValueId::new(SCALAR_CALL_UNIT_FIRST_RESULT).unwrap();
    let second_result = ValueId::new(SCALAR_CALL_UNIT_SECOND_RESULT).unwrap();
    let third_result = ValueId::new(SCALAR_CALL_UNIT_THIRD_RESULT).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let callee =
        conditional_u64_integer_equal_parameters_machine(SCALAR_CALL_UNIT_CALLEE_BASE, [1, 0]);
    let call = |id, result, arguments| Operation {
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
        id: caller,
        attachment: Some(attachment),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            id: entry,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(SCALAR_CALL_UNIT_LEFT_OPERATION).unwrap(),
                    result: OperationResult::Scalar(declaration(left)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                },
                Operation {
                    id: OperationId::new(SCALAR_CALL_UNIT_RIGHT_OPERATION).unwrap(),
                    result: OperationResult::Scalar(declaration(right)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(9),
                    },
                },
                call(SCALAR_CALL_UNIT_FIRST_CALL, first_result, vec![left, right]),
                call(
                    SCALAR_CALL_UNIT_SECOND_CALL,
                    second_result,
                    vec![left, right],
                ),
                call(
                    SCALAR_CALL_UNIT_THIRD_CALL,
                    third_result,
                    vec![first_result, second_result],
                ),
            ],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(SCALAR_CALL_UNIT_RETURN_EDGE).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(21_015).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let mut module = conditional_immediate_module(caller, vec![caller_machine, callee]);
    module.structural_types.push(StructuralTypeDeclaration {
        id: attachment,
        identity: "test::ScalarCallUnitAttachment".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    edit(&mut module);
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn staged_scalar_call_unit(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = scalar_call_unit_artifact();
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
