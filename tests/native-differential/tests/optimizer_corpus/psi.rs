use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_interpreter::{
    interpret_terminal_artifact_measured, TerminalExecutionResult, TerminalScalarValue,
};
use psi_terminal_verifier::ProofBundle;

use super::generator::LaneInput;

pub(super) struct CorpusArtifact {
    pub(super) semantic: Vec<u8>,
    pub(super) proof: Vec<u8>,
    pub(super) expected: u64,
    pub(super) add_operations: Vec<OperationId>,
}

pub(super) fn wrapping_add_artifact(
    ordinal: usize,
    input: LaneInput,
    lane_base: u64,
) -> CorpusArtifact {
    build_artifact(ordinal, lane_base, Leaf::WrappingAdd(input))
}

pub(super) fn immediate_artifact(ordinal: usize, expected: u64, lane_base: u64) -> CorpusArtifact {
    build_artifact(ordinal, lane_base, Leaf::Immediate(expected))
}

#[derive(Clone, Copy)]
enum Leaf {
    WrappingAdd(LaneInput),
    Immediate(u64),
}

fn build_artifact(ordinal: usize, lane_base: u64, leaf: Leaf) -> CorpusArtifact {
    let base = lane_base + u64::try_from(ordinal).unwrap() * 32;
    let machine = MachineId::new(base + 1).unwrap();
    let entry = BlockId::new(base + 2).unwrap();
    let when_true = BlockId::new(base + 3).unwrap();
    let when_false = BlockId::new(base + 4).unwrap();
    let condition = ValueId::new(base + 5).unwrap();
    let true_left = ValueId::new(base + 6).unwrap();
    let true_right = ValueId::new(base + 7).unwrap();
    let true_result = ValueId::new(base + 8).unwrap();
    let false_left = ValueId::new(base + 9).unwrap();
    let false_right = ValueId::new(base + 10).unwrap();
    let false_result = ValueId::new(base + 11).unwrap();
    let machine_result = ValueId::new(base + 12).unwrap();
    let true_left_operation = OperationId::new(base + 13).unwrap();
    let true_right_operation = OperationId::new(base + 14).unwrap();
    let true_add_operation = OperationId::new(base + 15).unwrap();
    let false_left_operation = OperationId::new(base + 16).unwrap();
    let false_right_operation = OperationId::new(base + 17).unwrap();
    let false_add_operation = OperationId::new(base + 18).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let literal = |id, result, value: u64| Operation {
        id,
        result: OperationResult::Scalar(declaration(result)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(value.into()),
        },
    };
    let wrapping_add = |id, result, left, right| Operation {
        id,
        result: OperationResult::Scalar(declaration(result)),
        kind: OperationKind::WrappingIntegerAdd { left, right },
    };
    let (true_operations, false_operations, add_operations, expected) = match leaf {
        Leaf::WrappingAdd(input) => (
            vec![
                literal(true_left_operation, true_left, input.left),
                literal(true_right_operation, true_right, input.right),
                wrapping_add(true_add_operation, true_result, true_left, true_right),
            ],
            vec![
                literal(false_left_operation, false_left, input.left),
                literal(false_right_operation, false_right, input.right),
                wrapping_add(false_add_operation, false_result, false_left, false_right),
            ],
            vec![true_add_operation, false_add_operation],
            input.expected,
        ),
        Leaf::Immediate(expected) => (
            vec![literal(true_left_operation, true_result, expected)],
            vec![literal(false_left_operation, false_result, expected)],
            Vec::new(),
            expected,
        ),
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: condition,
                scalar_type: ScalarType::Boolean,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(machine_result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(base + 19).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(base + 20).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: when_true,
                    parameters: Vec::new(),
                    operations: true_operations,
                    terminator: Terminator::Return {
                        edge: EdgeId::new(base + 21).unwrap(),
                        value: true_result,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: when_false,
                    parameters: Vec::new(),
                    operations: false_operations,
                    terminator: Terminator::Return {
                        edge: EdgeId::new(base + 22).unwrap(),
                        value: false_result,
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(base + 23).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    for condition in [false, true] {
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(condition)],
        )
        .unwrap();
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(expected.into()),
            })
        );
    }
    CorpusArtifact {
        semantic,
        proof,
        expected,
        add_operations,
    }
}
