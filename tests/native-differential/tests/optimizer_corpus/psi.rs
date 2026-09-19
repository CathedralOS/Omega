use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IeeeFloatComparisonOperation, IeeeFloatFormat,
    IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    BindingRelevance, Block, CertificateEnvelope, EvidenceRoute, MachineContract,
    ObligationEvidence, Operation, OperationKind, OperationResult, ProofSystemMarker,
    RecordFieldInitializer, RecordFieldValue, ScalarCaseField, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::ProofBundle;

use super::{
    affine_cleanup::CleanupCase,
    atomic_establishment::AtomicCase,
    exact_traps::{TrapCase, TrapOperation},
    generator::LaneInput,
    ieee_compare::CompareCase,
    placed_memory::PlacedMemoryCase,
    transition::{MAX_CARRIED_ARGUMENTS, TransitionCase, TransitionFold},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CorpusExpected {
    Unsigned(u64),
    Boolean(bool),
    /// Per-arm Boolean results: the false arm answers `when_false`, the true
    /// arm answers `when_true`.
    BooleanPerArm {
        when_false: bool,
        when_true: bool,
    },
    /// Per-arm u64 results: the false arm answers `when_false`, the true arm
    /// answers `when_true`.
    UnsignedPerArm {
        when_false: u64,
        when_true: u64,
    },
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

pub(super) fn affine_cleanup_artifact(
    ordinal: usize,
    case: &CleanupCase,
    lane_base: u64,
) -> CorpusArtifact {
    build_artifact(
        ordinal,
        lane_base,
        Leaf::AffineCleanup {
            left: case.left,
            right: case.right,
            expected: case.expected,
            true_records: case.true_cleanups,
            false_records: case.false_cleanups,
        },
    )
}

/// One atomic-establishment artifact: each conditional arm atomically
/// establishes its `established` sum case from u64 payload literals, observes
/// the discriminator through a `StructuralCaseMembership` query for the
/// Boolean result, then establishes an unobserved unrestricted fixed array.
/// The module shape diverges from `build_artifact` (two structural result
/// places per arm and a declared sum/array pair), so the artifact is built
/// here rather than through the shared leaf model.
pub(super) fn atomic_establishment_artifact(
    ordinal: usize,
    case: &AtomicCase,
    lane_base: u64,
) -> CorpusArtifact {
    let base = lane_base + u64::try_from(ordinal).unwrap() * 256;
    let machine = MachineId::new(base + 1).unwrap();
    let entry = BlockId::new(base + 2).unwrap();
    let when_true = BlockId::new(base + 3).unwrap();
    let when_false = BlockId::new(base + 4).unwrap();
    let condition = ValueId::new(base + 5).unwrap();
    let machine_result = ValueId::new(base + 9).unwrap();
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

    // Declared case `index` carries `index % 3` u64 fields. Case IDs and field
    // IDs must each be strictly increasing in the canonical encoding, so the
    // rosters are placed at dense ascending offsets.
    let sum_type = StructuralTypeId::new(base + 30).unwrap();
    let case_id = |index: u8| StructuralCaseId::new(base + 31 + u64::from(index)).unwrap();
    let field_id = |case_index: u8, field_index: u8| {
        StructuralFieldId::new(base + 40 + u64::from(case_index) * 3 + u64::from(field_index))
            .unwrap()
    };
    let element_type = StructuralTypeId::new(base + 50).unwrap();
    let array_type = StructuralTypeId::new(base + 51).unwrap();
    let cases = (0..case.case_count)
        .map(|index| StructuralCaseDeclaration {
            id: case_id(index),
            identity: format!("Case{index}"),
            fields: (0..AtomicCase::field_count(index))
                .map(|field| StructuralFieldDeclaration {
                    id: field_id(index, field),
                    identity: format!("f{field}"),
                    relevance: language_core::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(integer_scalar_type),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let queried = case_id(case.queried);

    // Per-arm operation IDs stay inside disjoint 32-wide windows.
    let arm = |op_base: u64, place_base: u64, edge: u64, established: u8| {
        let establish_op = OperationId::new(op_base + 8).unwrap();
        let member_op = OperationId::new(op_base + 9).unwrap();
        let array_op = OperationId::new(op_base + 10).unwrap();
        let result_value = ValueId::new(op_base + 11).unwrap();
        let case_place = PlaceId::new(place_base).unwrap();
        let array_place = PlaceId::new(place_base + 1).unwrap();
        let field_count = AtomicCase::field_count(established);
        let mut operations = Vec::new();
        let mut fields = Vec::new();
        for field in 0..field_count {
            let literal_op = OperationId::new(op_base + u64::from(field)).unwrap();
            let operand = ValueId::new(op_base + 12 + u64::from(field)).unwrap();
            operations.push(literal(literal_op, operand, case.payload));
            fields.push(ScalarCaseField {
                field: field_id(established, field),
                value: operand,
                range_obligation: None,
            });
        }
        operations.push(Operation {
            static_reach_binding: None,
            id: establish_op,
            result: OperationResult::Structural(StructuralOperationResult {
                place: case_place,
                structural_type: sum_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarCase {
                result_case: case_id(established),
                fields,
            },
        });
        operations.push(Operation {
            static_reach_binding: None,
            id: member_op,
            result: OperationResult::Scalar(declaration(result_value, ScalarType::Boolean)),
            kind: OperationKind::StructuralCaseMembership {
                source: case_place,
                path: Vec::new(),
                case: queried,
            },
        });
        let mut elements = Vec::new();
        for element in 0..case.array_elements {
            let element_op = OperationId::new(op_base + 16 + u64::from(element) * 2).unwrap();
            let operand = ValueId::new(op_base + 17 + u64::from(element) * 2).unwrap();
            operations.push(literal(element_op, operand, case.payload));
            elements.push(operand);
        }
        operations.push(Operation {
            static_reach_binding: None,
            id: array_op,
            result: OperationResult::Structural(StructuralOperationResult {
                place: array_place,
                structural_type: array_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarArray { elements },
        });
        let places = vec![
            StructuralPlaceDeclaration {
                id: case_place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: establish_op,
                    structural_type: sum_type,
                },
            },
            StructuralPlaceDeclaration {
                id: array_place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: array_op,
                    structural_type: array_type,
                },
            },
        ];
        (
            operations,
            places,
            Terminator::Return {
                edge: EdgeId::new(edge).unwrap(),
                value: result_value,
                cleanup_actions: Vec::new(),
            },
        )
    };
    let (true_operations, true_places, true_terminator) =
        arm(base + 60, base + 140, base + 21, case.established_true);
    let (false_operations, false_places, false_terminator) =
        arm(base + 96, base + 142, base + 22, case.established_false);

    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        // Canonical encoding orders structural types by id.
        structural_types: vec![
            StructuralTypeDeclaration {
                id: sum_type,
                identity: "omega.optimizer-corpus.atomic.Outcome".into(),
                shape: StructuralTypeShape::Sum { cases },
            },
            StructuralTypeDeclaration {
                id: element_type,
                identity: "omega.optimizer-corpus.atomic.Element".into(),
                shape: StructuralTypeShape::PrimitiveScalar(integer_scalar_type),
            },
            StructuralTypeDeclaration {
                id: array_type,
                identity: "omega.optimizer-corpus.atomic.Buffer".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: element_type,
                    length: u64::from(case.array_elements),
                },
            },
        ],
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
            parameters: vec![declaration(condition, ScalarType::Boolean)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(machine_result, ScalarType::Boolean)),
            structural_places: true_places.into_iter().chain(false_places).collect(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(base + 19).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(base + 20).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: when_true,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    operations: true_operations,
                    terminator: true_terminator,
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: when_false,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    operations: false_operations,
                    terminator: false_terminator,
                },
            ],
            contract: MachineContract {
                id: ContractId::new(base + 23).unwrap(),
                crash_routes: Vec::new(),
                erased_scalar_formals: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = terminal_codec::encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let expected = CorpusExpected::BooleanPerArm {
        when_false: case.expected_false(),
        when_true: case.expected_true(),
    };
    for (condition, arm_expected) in [(false, case.expected_false()), (true, case.expected_true())]
    {
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(condition)],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .unwrap();
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(arm_expected)),
            "atomic corpus ordinal {ordinal} diverged in the reference interpreter"
        );
    }
    CorpusArtifact {
        semantic,
        proof,
        expected,
        add_operations: Vec::new(),
    }
}

pub(super) fn placed_memory_artifact(
    ordinal: usize,
    case: &PlacedMemoryCase,
    lane_base: u64,
) -> CorpusArtifact {
    build_artifact(
        ordinal,
        lane_base,
        Leaf::PlacedMemory {
            initial: case.left,
            field: case.right,
            expected: case.expected,
            true_stores: case.true_stores,
            false_stores: case.false_stores,
        },
    )
}

/// Dense strictly-increasing identity supply for one transition artifact.
/// Every declaration kind draws from the same counter so ids stay unique and
/// reproducible case to case.
struct TransitionIds {
    next: u64,
}

impl TransitionIds {
    fn take(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn machine(&mut self) -> MachineId {
        MachineId::new(self.take()).unwrap()
    }

    fn contract(&mut self) -> ContractId {
        ContractId::new(self.take()).unwrap()
    }

    fn block(&mut self) -> BlockId {
        BlockId::new(self.take()).unwrap()
    }

    fn edge(&mut self) -> EdgeId {
        EdgeId::new(self.take()).unwrap()
    }

    fn value(&mut self) -> ValueId {
        ValueId::new(self.take()).unwrap()
    }

    fn operation(&mut self) -> OperationId {
        OperationId::new(self.take()).unwrap()
    }
}

/// One edge-transition artifact: the entry conditional transports seeded
/// literals into parameterized arm blocks, each arm folds its bound
/// parameters and `Jump`s a carried result (plus an untouched forwarded
/// parameter when the case carries two scalars) into a shared merge or
/// private tail, and the last level either returns directly, dispatches on a
/// computed `IntegerEqual` through argument-carrying leaf edges, or relays
/// the value into a shared single-parameter final block. The module shape
/// diverges from `build_artifact` (parameterized multi-block control flow
/// rather than two leaf arms), so the artifact is built here rather than
/// through the shared leaf model.
pub(super) fn transition_artifact(
    ordinal: usize,
    case: &TransitionCase,
    lane_base: u64,
) -> CorpusArtifact {
    let mut ids = TransitionIds {
        next: lane_base + u64::try_from(ordinal).unwrap() * 512,
    };
    let machine = ids.machine();
    let contract = ids.contract();
    let condition = ids.value();
    let machine_result = ids.value();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let integer_scalar_type = ScalarType::Integer(integer_type);
    let declaration = |id: ValueId| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type: integer_scalar_type,
    };
    let literal = |ids: &mut TransitionIds, value: u64| -> (Operation, ValueId) {
        let result = ids.value();
        (
            Operation {
                static_reach_binding: None,
                id: ids.operation(),
                result: OperationResult::Scalar(declaration(result)),
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(value.into()),
                },
            },
            result,
        )
    };
    let fold_operation = |ids: &mut TransitionIds,
                          fold: TransitionFold,
                          left: ValueId,
                          right: ValueId|
     -> (Operation, ValueId) {
        let result = ids.value();
        let kind = match fold {
            TransitionFold::SaturatingAdd => OperationKind::SaturatingIntegerAdd { left, right },
            TransitionFold::SaturatingSubtract => {
                OperationKind::SaturatingIntegerSubtract { left, right }
            }
            TransitionFold::BitwiseXor => OperationKind::IntegerBitwiseXor { left, right },
            TransitionFold::BitwiseAnd => OperationKind::IntegerBitwiseAnd { left, right },
        };
        (
            Operation {
                static_reach_binding: None,
                id: ids.operation(),
                result: OperationResult::Scalar(declaration(result)),
                kind,
            },
            result,
        )
    };
    let equality =
        |ids: &mut TransitionIds, left: ValueId, right: ValueId| -> (Operation, ValueId) {
            let result = ids.value();
            (
                Operation {
                    static_reach_binding: None,
                    id: ids.operation(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerEqual { left, right },
                },
                result,
            )
        };
    let successor =
        |ids: &mut TransitionIds, target: BlockId, arguments: Vec<ValueId>| SuccessorEdge {
            structural_arguments: Vec::new(),
            edge: ids.edge(),
            target,
            arguments,
            erased_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
    let jump =
        |ids: &mut TransitionIds, target: BlockId, arguments: Vec<ValueId>| Terminator::Jump {
            erased_arguments: Vec::new(),
            edge: ids.edge(),
            target,
            arguments,
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
    let scalar_return = |ids: &mut TransitionIds, value: ValueId| Terminator::Return {
        edge: ids.edge(),
        value,
        cleanup_actions: Vec::new(),
    };

    // Block ids are allocated in listing order so the declared `blocks`
    // sequence reads in strictly increasing identity order.
    let entry = ids.block();
    let arm_true = ids.block();
    let arm_false = ids.block();
    let merge = if case.converge {
        Some(ids.block())
    } else {
        None
    };
    let tail_true = merge.unwrap_or_else(|| ids.block());
    let tail_false = merge.unwrap_or_else(|| ids.block());
    let leaf_pair = |ids: &mut TransitionIds| [ids.block(), ids.block()];
    let merge_leaves = (case.converge && case.inner).then(|| leaf_pair(&mut ids));
    let tail_true_leaves = (!case.converge && case.inner).then(|| leaf_pair(&mut ids));
    let tail_false_leaves = (!case.converge && case.inner).then(|| leaf_pair(&mut ids));
    let final_block = case.extend.then(|| ids.block());
    let relay = |ids: &mut TransitionIds, value: ValueId| match final_block {
        Some(final_block) => jump(ids, final_block, vec![value]),
        None => scalar_return(ids, value),
    };

    // Entry: one literal per transported scalar, then the conditional whose
    // successor edges bind the arm parameters — reversed on the false edge
    // when the case permutes.
    let mut entry_operations = Vec::new();
    let seed_values = case.seeds[..case.edge_arguments as usize]
        .iter()
        .map(|seed| {
            let (operation, value) = literal(&mut ids, *seed);
            entry_operations.push(operation);
            value
        })
        .collect::<Vec<_>>();
    let mut false_arguments = seed_values.clone();
    if case.permute_false && case.edge_arguments >= 2 {
        false_arguments.reverse();
    }

    // Arm body: fold the bound parameters left to right, fold in the arm's
    // literal, then transport the result — and the untouched first parameter
    // when the case carries two scalars — along the Jump edge.
    let arm = |ids: &mut TransitionIds,
               block: BlockId,
               fold: TransitionFold,
               arm_literal: u64,
               target: BlockId|
     -> Block {
        let parameters = (0..case.edge_arguments)
            .map(|_| declaration(ids.value()))
            .collect::<Vec<_>>();
        let mut operations = Vec::new();
        let mut accumulator = parameters[0].id;
        for parameter in &parameters[1..] {
            let (operation, folded) = fold_operation(ids, fold, accumulator, parameter.id);
            operations.push(operation);
            accumulator = folded;
        }
        let (literal_operation, arm_value) = literal(ids, arm_literal);
        operations.push(literal_operation);
        let (fold_operation_result, result) = fold_operation(ids, fold, accumulator, arm_value);
        operations.push(fold_operation_result);
        let mut carried = vec![result];
        if case.carried == MAX_CARRIED_ARGUMENTS {
            carried.push(parameters[0].id);
        }
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block,
            parameters,
            operations,
            terminator: jump(ids, target, carried),
        }
    };

    // Second-level block (shared merge or private tail): either dispatch on a
    // computed equality through argument-carrying leaf edges, or fold the
    // carried bindings with the late literal and return or relay onward.
    let level = |ids: &mut TransitionIds,
                 block: BlockId,
                 leaves: Option<[BlockId; 2]>|
     -> (Block, Vec<Block>) {
        let parameters = (0..case.carried)
            .map(|_| declaration(ids.value()))
            .collect::<Vec<_>>();
        let mut operations = Vec::new();
        let mut leaf_blocks = Vec::new();
        let terminator = if case.inner {
            let [when_equal, when_unequal] = leaves.expect("inner dispatch declares its leaves");
            let other = if case.carried == MAX_CARRIED_ARGUMENTS {
                parameters[1].id
            } else {
                let (operation, value) = literal(ids, case.late_literal);
                operations.push(operation);
                value
            };
            let (equality_operation, equality) = equality(ids, parameters[0].id, other);
            operations.push(equality_operation);
            for (index, leaf) in [when_equal, when_unequal].into_iter().enumerate() {
                let leaf_parameter = declaration(ids.value());
                let mut leaf_operations = Vec::new();
                let (literal_operation, leaf_literal) = literal(ids, case.leaf_literals[index]);
                leaf_operations.push(literal_operation);
                let (leaf_fold, leaf_result) =
                    fold_operation(ids, case.fold_late, leaf_parameter.id, leaf_literal);
                leaf_operations.push(leaf_fold);
                let leaf_terminator = relay(ids, leaf_result);
                leaf_blocks.push(Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: leaf,
                    parameters: vec![leaf_parameter],
                    operations: leaf_operations,
                    terminator: leaf_terminator,
                });
            }
            // Each inner edge transports a different operand, so a swapped
            // binding diverges rather than agreeing by accident.
            Terminator::Conditional {
                condition: equality,
                when_true: successor(ids, when_equal, vec![parameters[0].id]),
                when_false: successor(ids, when_unequal, vec![other]),
            }
        } else {
            let combined = if case.carried == MAX_CARRIED_ARGUMENTS {
                let (operation, folded) =
                    fold_operation(ids, case.fold_late, parameters[0].id, parameters[1].id);
                operations.push(operation);
                folded
            } else {
                parameters[0].id
            };
            let (literal_operation, late) = literal(ids, case.late_literal);
            operations.push(literal_operation);
            let (result_operation, result) = fold_operation(ids, case.fold_late, combined, late);
            operations.push(result_operation);
            relay(ids, result)
        };
        (
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block,
                parameters,
                operations,
                terminator,
            },
            leaf_blocks,
        )
    };

    let mut blocks = Vec::new();
    blocks.push(Block {
        structural_parameters: Vec::new(),
        id: entry,
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        operations: entry_operations,
        terminator: Terminator::Conditional {
            condition,
            when_true: successor(&mut ids, arm_true, seed_values),
            when_false: successor(&mut ids, arm_false, false_arguments),
        },
    });
    let arm_true_target = merge.unwrap_or(tail_true);
    let arm_false_target = merge.unwrap_or(tail_false);
    // `arm_literals` follows the manifest's `[when_false, when_true]`
    // convention so `TransitionCase::expected(arm)` can index it by `arm`.
    blocks.push(arm(
        &mut ids,
        arm_true,
        case.fold_true,
        case.arm_literals[1],
        arm_true_target,
    ));
    blocks.push(arm(
        &mut ids,
        arm_false,
        case.fold_false,
        case.arm_literals[0],
        arm_false_target,
    ));
    if let Some(merge) = merge {
        let (block, mut leaf_blocks) = level(&mut ids, merge, merge_leaves);
        blocks.push(block);
        blocks.append(&mut leaf_blocks);
    } else {
        let (block, mut leaf_blocks) = level(&mut ids, tail_true, tail_true_leaves);
        blocks.push(block);
        let (block, mut tail_false_leaves) = level(&mut ids, tail_false, tail_false_leaves);
        blocks.push(block);
        blocks.append(&mut leaf_blocks);
        blocks.append(&mut tail_false_leaves);
    }
    if let Some(final_block) = final_block {
        let parameter = declaration(ids.value());
        blocks.push(Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: final_block,
            parameters: vec![parameter],
            operations: Vec::new(),
            terminator: scalar_return(&mut ids, parameter.id),
        });
    }

    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
            result: TerminalMachineResult::Scalar(declaration(machine_result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks,
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: contract,
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = terminal_codec::encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let expected = CorpusExpected::UnsignedPerArm {
        when_false: case.expected(false),
        when_true: case.expected(true),
    };
    for (condition, arm_expected) in [(false, case.expected(false)), (true, case.expected(true))] {
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(condition)],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .unwrap();
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(arm_expected.into()),
            }),
            "transition corpus ordinal {ordinal} diverged in the reference interpreter"
        );
    }
    CorpusArtifact {
        semantic,
        proof,
        expected,
        add_operations: Vec::new(),
    }
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
    AffineCleanup {
        left: u64,
        right: u64,
        expected: u64,
        true_records: u8,
        false_records: u8,
    },
    PlacedMemory {
        initial: u64,
        field: u64,
        expected: u64,
        true_stores: u8,
        false_stores: u8,
    },
}

/// Structural declarations and exact return cleanup schedules carried beside
/// the leaf operations. The affine-cleanup lane fills the action schedules;
/// the placed-memory lane fills only types and places. Every other lane
/// leaves this empty, preserving the corpus module's scalar-only shape.
#[derive(Default)]
struct StructuralPlan {
    structural_types: Vec<StructuralTypeDeclaration>,
    structural_places: Vec<StructuralPlaceDeclaration>,
    true_actions: Vec<TerminalAffineCleanupAction>,
    false_actions: Vec<TerminalAffineCleanupAction>,
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
    // `run_machine` legalizes only the admitted scalar instruction set; the
    // wrapping-add leaf is exercised through Psi/SCCP but is not admitted into
    // selected-machine lowering. Saturating u64 add is admitted and keeps the
    // cleanup lane's observation a pure two-operand scalar deterministic fold.
    let saturating_add = |id, result, left, right| Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(declaration(result, integer_scalar_type)),
        kind: OperationKind::SaturatingIntegerAdd { left, right },
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
    let (true_operations, false_operations, add_operations, expected, machine_scalar_type, cleanup) =
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
                StructuralPlan::default(),
            ),
            Leaf::Immediate(expected) => (
                vec![literal(true_left_operation, true_result, expected)],
                vec![literal(false_left_operation, false_result, expected)],
                Vec::new(),
                CorpusExpected::Unsigned(expected),
                integer_scalar_type,
                StructuralPlan::default(),
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
                StructuralPlan::default(),
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
                StructuralPlan::default(),
            ),
            Leaf::AffineCleanup {
                left,
                right,
                expected,
                true_records,
                false_records,
            } => {
                // One shared claim-free empty-record type per artifact. Each
                // arm establishes its own affine results and the selected
                // return edge disposes them in reverse producer order — the
                // exact schedule the verifier reconstructs independently.
                let record_type = StructuralTypeId::new(base + 30).unwrap();
                let record_type_declaration = StructuralTypeDeclaration {
                    id: record_type,
                    identity: "omega.optimizer-corpus.affine_cleanup.Cell".into(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                };
                let records = |place_base: u64, operation_base: u64, count: u8| {
                    (0..count)
                        .map(|index| {
                            (
                                PlaceId::new(place_base + u64::from(index)).unwrap(),
                                OperationId::new(operation_base + u64::from(index)).unwrap(),
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let true_records = records(base + 31, base + 41, true_records);
                let false_records = records(base + 51, base + 61, false_records);
                let establish = |(place, producer): (PlaceId, OperationId)| Operation {
                    static_reach_binding: None,
                    id: producer,
                    result: OperationResult::Structural(StructuralOperationResult {
                        place,
                        structural_type: record_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::EstablishRecord { fields: Vec::new() },
                };
                let mut cleanup = StructuralPlan {
                    structural_types: vec![record_type_declaration],
                    ..StructuralPlan::default()
                };
                for (place, producer) in true_records.iter().chain(&false_records) {
                    cleanup.structural_places.push(StructuralPlaceDeclaration {
                        id: *place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: *producer,
                            structural_type: record_type,
                        },
                    });
                }
                let schedule = |records: &[(PlaceId, OperationId)]| {
                    let mut ordered = records.to_vec();
                    ordered.sort_by_key(|(_, producer)| std::cmp::Reverse(*producer));
                    ordered
                        .into_iter()
                        .map(|(place, _)| TerminalAffineCleanupAction::DiscardRoot(place))
                        .collect::<Vec<_>>()
                };
                cleanup.true_actions = schedule(&true_records);
                cleanup.false_actions = schedule(&false_records);
                (
                    [
                        literal(true_left_operation, true_left, left),
                        literal(true_right_operation, true_right, right),
                        saturating_add(true_add_operation, true_result, true_left, true_right),
                    ]
                    .into_iter()
                    .chain(true_records.iter().copied().map(&establish))
                    .collect(),
                    [
                        literal(false_left_operation, false_left, left),
                        literal(false_right_operation, false_right, right),
                        saturating_add(false_add_operation, false_result, false_left, false_right),
                    ]
                    .into_iter()
                    .chain(false_records.iter().copied().map(&establish))
                    .collect(),
                    Vec::new(),
                    CorpusExpected::Unsigned(expected),
                    integer_scalar_type,
                    cleanup,
                )
            }
            Leaf::PlacedMemory {
                initial,
                field,
                expected,
                true_stores,
                false_stores,
            } => {
                // One primitive-local storage type and one single-field record
                // type per artifact; each arm establishes and observes its own
                // places. Both multiplicities stay unrestricted, so the return
                // edge owes no cleanup schedule — placed-memory observations
                // are ordinary reads and writes.
                let local_type = StructuralTypeId::new(base + 26).unwrap();
                let record_type = StructuralTypeId::new(base + 27).unwrap();
                let record_field = StructuralFieldId::new(base + 28).unwrap();
                let mut plan = StructuralPlan {
                    structural_types: vec![
                        StructuralTypeDeclaration {
                            id: local_type,
                            identity: "omega.optimizer-corpus.placed_memory.Cell".into(),
                            shape: StructuralTypeShape::PrimitiveScalar(integer_scalar_type),
                        },
                        StructuralTypeDeclaration {
                            id: record_type,
                            identity: "omega.optimizer-corpus.placed_memory.Holder".into(),
                            shape: StructuralTypeShape::Record {
                                fields: vec![StructuralFieldDeclaration {
                                    id: record_field,
                                    identity: "value".into(),
                                    relevance: BindingRelevance::Relevant,
                                    field_type: StructuralFieldType::Scalar(integer_scalar_type),
                                }],
                            },
                        },
                    ],
                    ..StructuralPlan::default()
                };
                let mut placed_arm = |arm_base: u64,
                                      stores: u8,
                                      left_operation: OperationId,
                                      left_value: ValueId,
                                      right_operation: OperationId,
                                      right_value: ValueId,
                                      add_operation: OperationId,
                                      result_value: ValueId|
                 -> Vec<Operation> {
                    let local_place = PlaceId::new(arm_base).unwrap();
                    let establish_local = OperationId::new(arm_base + 1).unwrap();
                    let loaded = ValueId::new(arm_base + 2).unwrap();
                    let read = OperationId::new(arm_base + 3).unwrap();
                    let record_place = PlaceId::new(arm_base + 4).unwrap();
                    let establish_record = OperationId::new(arm_base + 5).unwrap();
                    let field_value = ValueId::new(arm_base + 6).unwrap();
                    let field_read = OperationId::new(arm_base + 7).unwrap();
                    let mut operations = vec![
                        literal(left_operation, left_value, initial),
                        Operation {
                            static_reach_binding: None,
                            id: establish_local,
                            result: OperationResult::Structural(StructuralOperationResult {
                                place: local_place,
                                structural_type: local_type,
                                multiplicity: StructuralMultiplicity::Unrestricted,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                claims: Vec::new(),
                            }),
                            kind: OperationKind::EstablishPrimitiveLocal { value: left_value },
                        },
                    ];
                    // Intermediate writes deposit distinct values and the final
                    // write restores the initializer, so both arms return the
                    // same saturating sum while a mistaken store schedule or a
                    // stale read still diverges from the interpreter.
                    for index in 0..stores {
                        let written = if index + 1 == stores {
                            initial
                        } else {
                            initial.wrapping_add(u64::from(index) + 1)
                        };
                        let written_value = ValueId::new(arm_base + 8 + u64::from(index)).unwrap();
                        operations.push(literal(
                            OperationId::new(arm_base + 12 + u64::from(index)).unwrap(),
                            written_value,
                            written,
                        ));
                        operations.push(Operation {
                            static_reach_binding: None,
                            id: OperationId::new(arm_base + 16 + u64::from(index)).unwrap(),
                            result: OperationResult::Unit,
                            kind: OperationKind::WriteOnlyPrimitiveStore {
                                destination: local_place,
                                path: Vec::new(),
                                value: written_value,
                            },
                        });
                    }
                    operations.extend([
                        Operation {
                            static_reach_binding: None,
                            id: read,
                            result: OperationResult::Scalar(declaration(
                                loaded,
                                integer_scalar_type,
                            )),
                            kind: OperationKind::PrimitiveScalarRead {
                                source: local_place,
                                path: Vec::new(),
                            },
                        },
                        literal(right_operation, right_value, field),
                        Operation {
                            static_reach_binding: None,
                            id: establish_record,
                            result: OperationResult::Structural(StructuralOperationResult {
                                place: record_place,
                                structural_type: record_type,
                                multiplicity: StructuralMultiplicity::Unrestricted,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                claims: Vec::new(),
                            }),
                            kind: OperationKind::EstablishRecord {
                                fields: vec![RecordFieldInitializer {
                                    field: record_field,
                                    value: RecordFieldValue::Scalar {
                                        value: right_value,
                                        range_obligation: None,
                                    },
                                }],
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: field_read,
                            result: OperationResult::Scalar(declaration(
                                field_value,
                                integer_scalar_type,
                            )),
                            kind: OperationKind::IntegerStructuralField {
                                source: record_place,
                                path: Vec::new(),
                                field: record_field,
                            },
                        },
                        saturating_add(add_operation, result_value, loaded, field_value),
                    ]);
                    plan.structural_places.extend([
                        StructuralPlaceDeclaration {
                            id: local_place,
                            kind: StructuralPlaceKind::OperationResult {
                                producer: establish_local,
                                structural_type: local_type,
                            },
                        },
                        StructuralPlaceDeclaration {
                            id: record_place,
                            kind: StructuralPlaceKind::OperationResult {
                                producer: establish_record,
                                structural_type: record_type,
                            },
                        },
                    ]);
                    operations
                };
                (
                    placed_arm(
                        base + 30,
                        true_stores,
                        true_left_operation,
                        true_left,
                        true_right_operation,
                        true_right,
                        true_add_operation,
                        true_result,
                    ),
                    placed_arm(
                        base + 60,
                        false_stores,
                        false_left_operation,
                        false_left,
                        false_right_operation,
                        false_right,
                        false_add_operation,
                        false_result,
                    ),
                    Vec::new(),
                    CorpusExpected::Unsigned(expected),
                    integer_scalar_type,
                    plan,
                )
            }
        };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: cleanup.structural_types,
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
            structural_places: cleanup.structural_places,
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(base + 19).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(base + 20).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: when_true,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    operations: true_operations,
                    terminator: Terminator::Return {
                        edge: EdgeId::new(base + 21).unwrap(),
                        value: true_result,
                        cleanup_actions: cleanup.true_actions,
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: when_false,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    operations: false_operations,
                    terminator: Terminator::Return {
                        edge: EdgeId::new(base + 22).unwrap(),
                        value: false_result,
                        cleanup_actions: cleanup.false_actions,
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(base + 23).unwrap(),
                crash_routes: Vec::new(),
                erased_scalar_formals: Vec::new(),
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
    let proof = terminal_codec::encode_proof_section(&module, &proof).unwrap();
    for condition in [false, true] {
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(condition)],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .unwrap();
        let expected_value = match expected {
            CorpusExpected::Unsigned(expected) => TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(expected.into()),
            },
            CorpusExpected::Boolean(expected) => TerminalScalarValue::Boolean(expected),
            CorpusExpected::BooleanPerArm { .. } | CorpusExpected::UnsignedPerArm { .. } => {
                unreachable!("shared leaf artifacts never carry per-arm results")
            }
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
