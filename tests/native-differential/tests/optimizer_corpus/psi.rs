use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IeeeFloatComparisonOperation, IeeeFloatFormat,
    IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    ScalarType, ValueId,
};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    Block, CertificateEnvelope, EvidenceRoute, MachineContract, ObligationEvidence, Operation,
    OperationKind, OperationResult, ProofSystemMarker, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::ProofBundle;

use super::{
    exact_traps::{TrapCase, TrapOperation},
    generator::LaneInput,
    ieee_compare::CompareCase,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CorpusExpected {
    Unsigned(u64),
    Boolean(bool),
}

pub(super) struct CorpusArtifact {
    pub(super) semantic: Vec<u8>,
    pub(super) proof: Vec<u8>,
    pub(super) expected: CorpusExpected,
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

pub(super) fn ieee_compare_artifact(
    ordinal: usize,
    case: &CompareCase,
    lane_base: u64,
) -> CorpusArtifact {
    build_artifact(
        ordinal,
        lane_base,
        Leaf::IeeeCompare {
            comparison: case.comparison,
            left_bits: case.left_bits,
            right_bits: case.right_bits,
            expected: case.expected,
        },
    )
}

pub(super) fn exact_trap_artifact(
    ordinal: usize,
    case: &TrapCase,
    lane_base: u64,
) -> CorpusArtifact {
    build_artifact(
        ordinal,
        lane_base,
        Leaf::ExactTrap {
            operation: case.operation,
            left: case.left,
            right: case.right,
            expected: case.expected,
        },
    )
}

#[derive(Clone, Copy)]
enum Leaf {
    WrappingAdd(LaneInput),
    Immediate(u64),
    IeeeCompare {
        comparison: IeeeFloatComparisonOperation,
        left_bits: u64,
        right_bits: u64,
        expected: bool,
    },
    ExactTrap {
        operation: TrapOperation,
        left: u64,
        right: u64,
        expected: u64,
    },
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
    let integer_scalar_type = ScalarType::Integer(integer_type);
    let declaration = |id, scalar_type| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let literal = |id, result, value: u64| Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(declaration(result, integer_scalar_type)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(value.into()),
        },
    };
    let wrapping_add = |id, result, left, right| Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(declaration(result, integer_scalar_type)),
        kind: OperationKind::WrappingIntegerAdd { left, right },
    };
    let float_scalar_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
    let ieee_literal = |id, result, bits| Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(declaration(result, float_scalar_type)),
        kind: OperationKind::IeeeFloatConstant {
            value: IeeeFloatValue::Binary64(bits),
        },
    };
    let ieee_compare = |id, result, comparison, left, right| Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(declaration(result, ScalarType::Boolean)),
        kind: OperationKind::IeeeFloatCompare {
            comparison,
            left,
            right,
        },
    };
    let narrow_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let trap_leaf = |operation: TrapOperation,
                     left,
                     right,
                     obligation,
                     left_id,
                     right_id,
                     result_id,
                     left_operation,
                     right_operation,
                     combine_operation| {
        match operation {
            TrapOperation::ExactAdd | TrapOperation::ExactSubtract | TrapOperation::ExactDivide => {
                let kind = match operation {
                    TrapOperation::ExactAdd => OperationKind::ExactIntegerAdd {
                        left: left_id,
                        right: right_id,
                        obligation,
                    },
                    TrapOperation::ExactSubtract => OperationKind::ExactIntegerSubtract {
                        left: left_id,
                        right: right_id,
                        obligation,
                    },
                    TrapOperation::ExactDivide => OperationKind::ExactIntegerDivide {
                        left: left_id,
                        right: right_id,
                        obligation,
                    },
                    TrapOperation::NarrowingCast => unreachable!("binary trap leaf match"),
                };
                vec![
                    literal(left_operation, left_id, left),
                    literal(right_operation, right_id, right),
                    Operation {
                        static_reach_binding: None,
                        id: combine_operation,
                        result: OperationResult::Scalar(declaration(
                            result_id,
                            integer_scalar_type,
                        )),
                        kind,
                    },
                ]
            }
            TrapOperation::NarrowingCast => vec![
                literal(left_operation, left_id, left),
                Operation {
                    static_reach_binding: None,
                    id: right_operation,
                    result: OperationResult::Scalar(declaration(
                        right_id,
                        ScalarType::Integer(narrow_type),
                    )),
                    kind: OperationKind::IntegerExactCast {
                        operand: left_id,
                        obligation,
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: combine_operation,
                    result: OperationResult::Scalar(declaration(result_id, integer_scalar_type)),
                    kind: OperationKind::IntegerWiden { operand: right_id },
                },
            ],
        }
    };
    let (true_operations, false_operations, add_operations, expected, machine_scalar_type) =
        match leaf {
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
                CorpusExpected::Unsigned(input.expected),
                integer_scalar_type,
            ),
            Leaf::Immediate(expected) => (
                vec![literal(true_left_operation, true_result, expected)],
                vec![literal(false_left_operation, false_result, expected)],
                Vec::new(),
                CorpusExpected::Unsigned(expected),
                integer_scalar_type,
            ),
            Leaf::IeeeCompare {
                comparison,
                left_bits,
                right_bits,
                expected,
            } => (
                vec![
                    ieee_literal(true_left_operation, true_left, left_bits),
                    ieee_literal(true_right_operation, true_right, right_bits),
                    ieee_compare(
                        true_add_operation,
                        true_result,
                        comparison,
                        true_left,
                        true_right,
                    ),
                ],
                vec![
                    ieee_literal(false_left_operation, false_left, left_bits),
                    ieee_literal(false_right_operation, false_right, right_bits),
                    ieee_compare(
                        false_add_operation,
                        false_result,
                        comparison,
                        false_left,
                        false_right,
                    ),
                ],
                Vec::new(),
                CorpusExpected::Boolean(expected),
                ScalarType::Boolean,
            ),
            Leaf::ExactTrap {
                operation,
                left,
                right,
                expected,
            } => (
                trap_leaf(
                    operation,
                    left,
                    right,
                    ObligationId::new(base + 24).unwrap(),
                    true_left,
                    true_right,
                    true_result,
                    true_left_operation,
                    true_right_operation,
                    true_add_operation,
                ),
                trap_leaf(
                    operation,
                    left,
                    right,
                    ObligationId::new(base + 25).unwrap(),
                    false_left,
                    false_right,
                    false_result,
                    false_left_operation,
                    false_right_operation,
                    false_add_operation,
                ),
                Vec::new(),
                CorpusExpected::Unsigned(expected),
                integer_scalar_type,
            ),
        };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
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
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: condition,
                scalar_type: ScalarType::Boolean,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(machine_result, machine_scalar_type)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(base + 19).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(base + 20).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
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
                    structural_parameters: Vec::new(),
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
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: if matches!(leaf, Leaf::ExactTrap { .. }) {
            canonical_integer_evidence(&module)
        } else {
            Vec::new()
        },
    };
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&proof).unwrap();
    for condition in [false, true] {
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(condition)],
        )
        .unwrap();
        let expected_value = match expected {
            CorpusExpected::Unsigned(expected) => TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(expected.into()),
            },
            CorpusExpected::Boolean(expected) => TerminalScalarValue::Boolean(expected),
        };
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(expected_value)
        );
    }
    CorpusArtifact {
        semantic,
        proof,
        expected,
        add_operations,
    }
}

/// Builds canonical-certificate evidence for every reconstructed operation
/// obligation in the corpus machine. Exact integer leaves admit canonical
/// certificate goals, so each obligation carries a checked proof rather than
/// the trivially-trusted kernel route used by obligation-free lanes.
fn canonical_integer_evidence(module: &TerminalModule) -> Vec<ObligationEvidence> {
    let validated = terminal_verifier::validate_module(module).unwrap();
    let questions = terminal_verifier::reconstruct_operation_obligations(module).unwrap();
    assert_eq!(questions.len(), 2, "each trap arm must own one obligation");
    let mut evidence = questions
        .iter()
        .map(|question| {
            assert!(
                question.canonical_certificate,
                "trap corpus obligation must admit a canonical certificate"
            );
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == question.owner.machine())
                .expect("reconstructed operation owner belongs to the corpus module");
            let context = validated.value_context(machine).unwrap();
            let machine_parameter_values = machine
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect();
            let proof = checked_trees_to_lowered_psi::produce_checked_canonical_integer_proof(
                &context,
                &question.obligation.proposition,
                &machine.contract.requires,
                &question.semantic_axioms,
                &machine_parameter_values,
            )
            .unwrap_or_else(|| {
                panic!("trap corpus obligation must prove a canonical integer certificate")
            });
            ObligationEvidence {
                obligation: question.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(question.obligation.id.get()).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof,
                }),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(|evidence| evidence.obligation);
    evidence
}
