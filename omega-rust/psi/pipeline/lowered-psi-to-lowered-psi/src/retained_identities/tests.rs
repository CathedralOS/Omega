//! Entrance-level legs for the retained-identity family through the public
//! `run_psi_optimization` coordinator: the published artifact is a legal
//! second input, publication is deterministic, a selection without
//! retained-identity consumers leaves the carriers inert, forged
//! after-modules that drop covered coordinates are rejected by the same
//! independent validators the entrance runs, and a malformed ranked carrier
//! fails closed at the module-validation gate.

use crate::{PsiOptimizationStageError, run_psi_optimization};
use lowered_psi::{LoweredPsi, LoweredSourceCallOccurrence};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ProofBundle, SuccessorEdge,
    TerminalBlockNaturalRank, TerminalMachine, TerminalMachineResult, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    CopyPropagationRewriteError, DeadScalarRewriteError, validate_copy_propagation,
    validate_dead_scalar_elimination,
};

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}

fn block_id(ordinal: u64) -> BlockId {
    BlockId::new(ordinal).unwrap()
}

fn edge(ordinal: u64) -> EdgeId {
    EdgeId::new(ordinal).unwrap()
}

fn u32_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
}

fn declaration(ordinal: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value(ordinal),
        scalar_type,
    }
}

fn boolean(ordinal: u64) -> ValueDeclaration {
    declaration(ordinal, ScalarType::Boolean)
}

fn u32(ordinal: u64) -> ValueDeclaration {
    declaration(ordinal, u32_type())
}

fn operation(ordinal: u64, result: ValueDeclaration, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        id: OperationId::new(ordinal).unwrap(),
        result: OperationResult::Scalar(result),
        kind,
    }
}

fn lowered(machines: Vec<TerminalMachine>) -> LoweredPsi {
    let entry = machines[0].id;
    LoweredPsi {
        selected_ieee_float_comparison_occurrences: Vec::new(),
        semantic_module: terminal_psi::TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry,
            scalar_qualifications: Default::default(),
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
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    }
}

fn machine(
    ordinal: u64,
    parameters: Vec<ValueDeclaration>,
    result: TerminalMachineResult,
    entry: BlockId,
    blocks: Vec<Block>,
) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(ordinal).unwrap(),
        attachment: None,
        parameters,
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks,
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(ordinal).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn block(
    ordinal: u64,
    parameters: Vec<ValueDeclaration>,
    operations: Vec<Operation>,
    terminator: Terminator,
) -> Block {
    Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: block_id(ordinal),
        parameters,
        operations,
        terminator,
    }
}

fn successor(ordinal: u64, target: BlockId, arguments: Vec<ValueId>) -> SuccessorEdge {
    SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: edge(ordinal),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

/// Ranked-cycle fixture: a `Natural`-covered self-loop behind an acyclic
/// merge, the same shape the per-rule stage suites exercise.
///
/// ```text
/// b1 (entry): v20 = v1 + v1; v21 = v1 + v1; v22 = v1 + v1;
///             cond v2 ──e1:t──▶ b2 [v1, v22]
///                    └──e2:f──▶ b2 [v1, v22]
/// b2 (v30, v31): ──e5:[v30, v2, v21]──▶ b3
/// b3 (v10, v11, v32) [covered member]: v40 = 1u32; v42 = 1u32;
///             v41 = v10 - v40;
///             cond v11 ──e6:[v41, v11, v21]──▶ b3 (covered backedge)
///                    └──e7───────────────────▶ b4
/// b4: return
/// ```
///
/// The row names member block `b3`, rank `v10`, covered edge `e6`, and
/// successor `v41`. `v32` is a copy-shaped covered parameter, `v31` a dead
/// merge parameter, `v22` an ordinary duplicate, and `v21` a duplicate whose
/// identity covered contents still use.
fn ranked_cycle_fixture() -> LoweredPsi {
    let (b1, merge, member, exit) = (block_id(1), block_id(2), block_id(3), block_id(4));
    let (v1, v2) = (value(1), value(2));
    let (v10, v11) = (value(10), value(11));
    let (v21, v22) = (value(21), value(22));
    let v30 = value(30);
    let (v40, v41) = (value(40), value(41));
    let mut lowered = lowered(vec![machine(
        1,
        vec![u32(1), boolean(2)],
        TerminalMachineResult::Unit,
        b1,
        vec![
            block(
                1,
                Vec::new(),
                vec![
                    operation(
                        20,
                        u32(20),
                        OperationKind::WrappingIntegerAdd {
                            left: v1,
                            right: v1,
                        },
                    ),
                    operation(
                        21,
                        u32(21),
                        OperationKind::WrappingIntegerAdd {
                            left: v1,
                            right: v1,
                        },
                    ),
                    operation(
                        22,
                        u32(22),
                        OperationKind::WrappingIntegerAdd {
                            left: v1,
                            right: v1,
                        },
                    ),
                ],
                Terminator::Conditional {
                    condition: v2,
                    when_true: successor(1, merge, vec![v1, v22]),
                    when_false: successor(2, merge, vec![v1, v22]),
                },
            ),
            block(
                2,
                vec![u32(30), u32(31)],
                Vec::new(),
                Terminator::Jump {
                    edge: edge(5),
                    target: member,
                    arguments: vec![v30, v2, v21],
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            ),
            block(
                3,
                vec![u32(10), boolean(11), u32(32)],
                vec![
                    operation(
                        40,
                        u32(40),
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(1),
                        },
                    ),
                    operation(
                        42,
                        u32(42),
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(1),
                        },
                    ),
                    operation(
                        41,
                        u32(41),
                        OperationKind::WrappingIntegerSubtract {
                            left: v10,
                            right: v40,
                        },
                    ),
                ],
                Terminator::Conditional {
                    condition: v11,
                    when_true: successor(6, member, vec![v41, v11, v21]),
                    when_false: successor(7, exit, vec![]),
                },
            ),
            block(
                4,
                Vec::new(),
                Vec::new(),
                Terminator::ReturnUnit {
                    edge: edge(8),
                    trivial_affine_discards: Vec::new(),
                },
            ),
        ],
    )]);
    lowered.semantic_module.machines[0].ranked_scc =
        Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            ranks: vec![TerminalBlockNaturalRank {
                block: member,
                value: v10,
            }],
            edges: vec![TerminalNaturalRankEdge {
                edge: edge(6),
                source: member,
                target: member,
                successor_rank: v41,
                comparison: TerminalNaturalRankComparison::Strict,
            }],
        }]));
    lowered
}

fn selections(selection: &[PsiOptimization]) -> PsiOptimizationSelections {
    PsiOptimizationSelections::new(selection.iter().copied()).unwrap()
}

#[test]
fn the_published_ranked_artifact_reenters_as_a_legal_second_input() {
    // Fixed point through the real second-input contract: the validated
    // `LoweredPsi` the entrance publishes feeds back through the same
    // selection, and the second publication equals the first — not a
    // reconstructed-input equality.
    for selection in [
        PsiOptimization::CopyPropagation,
        PsiOptimization::DeadPureScalarElimination,
        PsiOptimization::GlobalValueNumbering,
    ] {
        let once = run_psi_optimization(ranked_cycle_fixture(), selections(&[selection])).unwrap();
        let twice = run_psi_optimization(once.lowered().clone(), selections(&[selection])).unwrap();
        assert_eq!(
            twice.lowered(),
            once.lowered(),
            "{selection:?} reaches a fixed point on the published artifact"
        );
    }
}

#[test]
fn retained_identity_publication_is_deterministic() {
    // Determinism: two runs over the same ranked carrier publish identical
    // artifacts, including the retained coordinates each rule preserved.
    let first = run_psi_optimization(
        ranked_cycle_fixture(),
        selections(&[
            PsiOptimization::CopyPropagation,
            PsiOptimization::GlobalValueNumbering,
            PsiOptimization::DeadPureScalarElimination,
        ]),
    )
    .unwrap();
    let second = run_psi_optimization(
        ranked_cycle_fixture(),
        selections(&[
            PsiOptimization::CopyPropagation,
            PsiOptimization::GlobalValueNumbering,
            PsiOptimization::DeadPureScalarElimination,
        ]),
    )
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn covered_coordinates_survive_publication_while_uncovered_rules_apply() {
    // Positive at the entrance: ordinary rewrites still apply outside the
    // covered component while member contents — parameters, operations, and
    // the covered edge's exact arguments — publish unchanged.
    let copy_result = run_psi_optimization(
        ranked_cycle_fixture(),
        selections(&[PsiOptimization::CopyPropagation]),
    )
    .unwrap();
    let module = &copy_result.lowered().semantic_module.machines[0];
    assert!(
        module.blocks[1].parameters.is_empty(),
        "the uncovered merge parameters still collapse"
    );
    let member = &module.blocks[2];
    assert_eq!(
        member
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>(),
        vec![value(10), value(11), value(32)],
        "the covered member keeps every parameter, including the copy-shaped one"
    );
    let Terminator::Conditional { when_true, .. } = &member.terminator else {
        panic!("the member still branches")
    };
    assert_eq!(
        when_true.arguments,
        vec![value(41), value(11), value(21)],
        "the covered edge keeps its exact arguments"
    );

    let dead_result = run_psi_optimization(
        ranked_cycle_fixture(),
        selections(&[PsiOptimization::DeadPureScalarElimination]),
    )
    .unwrap();
    let module = &dead_result.lowered().semantic_module.machines[0];
    let member = &module.blocks[2];
    assert_eq!(
        member
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![OperationId::new(40).unwrap(), OperationId::new(41).unwrap()],
        "coverage pins parameter tables and named values — the dead member \
         operation still dies under the ordinary rule"
    );
    assert_eq!(
        member
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>(),
        vec![value(10), value(11), value(32)],
        "the covered member keeps its dead parameter"
    );
}

#[test]
fn a_consumer_free_selection_leaves_the_carriers_inert() {
    // Disabled analog: `retained_identities` is not a selection member, so
    // its "disabled" leg is a selection whose rules never consume it.
    // Proof-check elision and sparse conditional constant propagation leave
    // ranked machines and their proof carriers exactly as published.
    for selection in [
        PsiOptimization::ProofCheckElision,
        PsiOptimization::SparseConditionalConstantPropagation,
    ] {
        let input = ranked_cycle_fixture();
        let result = run_psi_optimization(input.clone(), selections(&[selection])).unwrap();
        assert_eq!(
            result.lowered(),
            &input,
            "{selection:?} publishes the carrier unchanged"
        );
    }
}

#[test]
fn a_forged_after_corrupting_the_retained_rank_row_is_rejected() {
    // Corruption of the retained evidence itself: take the artifact the
    // entrance genuinely publishes, leave every block untouched, and mutate
    // only the `Natural` row so its successor rank is not the arriving rank
    // argument. The same validator the entrance runs refuses the carrier on
    // the output side before the rewrite relation is examined.
    let before = ranked_cycle_fixture();
    let genuine = run_psi_optimization(
        before.clone(),
        selections(&[PsiOptimization::CopyPropagation]),
    )
    .unwrap();
    let mut forged = genuine.lowered().semantic_module.clone();
    let TerminalRankedScc::Natural(components) = forged.machines[0].ranked_scc.as_mut().unwrap();
    components[0].edges[0].successor_rank = value(10);
    let result = validate_copy_propagation(&before.semantic_module, &forged);
    assert!(
        matches!(result, Err(CopyPropagationRewriteError::InvalidModule(_))),
        "a corrupted retained row fails module validation: {result:?}"
    );
}

#[test]
fn a_forged_after_rewiring_a_member_used_value_is_rejected() {
    // Corruption under the dead-scalar relation: a forged after removes the
    // producer of v21 — the duplicate whose identity the covered backedge
    // still passes at the rank-adjacent position — and rewires those member
    // uses to the equal computation v20. The independent validator restores
    // removals and compares: a rewired edge is not a removal subsequence, and
    // a covered member's exact arguments are not substitutable.
    let before = ranked_cycle_fixture().semantic_module;
    let mut forged = before.clone();
    let machine = &mut forged.machines[0];
    machine.blocks[0]
        .operations
        .retain(|operation| operation.id != OperationId::new(21).unwrap());
    for block in &mut machine.blocks {
        match &mut block.terminator {
            Terminator::Jump { arguments, .. } => {
                for argument in arguments.iter_mut() {
                    if *argument == value(21) {
                        *argument = value(20);
                    }
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    for argument in successor.arguments.iter_mut() {
                        if *argument == value(21) {
                            *argument = value(20);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let result = validate_dead_scalar_elimination(&before, &forged);
    assert!(
        matches!(result, Err(DeadScalarRewriteError::ChangedEdgeArguments(_))),
        "rewiring a member-used identity is rejected: {result:?}"
    );
}

#[test]
fn a_malformed_ranked_carrier_fails_closed_at_the_gate() {
    // Corruption on the way in: a successor-rank row naming a value that is
    // not the arriving rank argument makes the carrier invalid, so the
    // entrance refuses before any rule runs — even with an empty selection.
    let mut malformed = ranked_cycle_fixture();
    let TerminalRankedScc::Natural(components) = malformed.semantic_module.machines[0]
        .ranked_scc
        .as_mut()
        .unwrap();
    components[0].edges[0].successor_rank = value(10);
    let result = run_psi_optimization(malformed, selections(&[]));
    assert!(
        matches!(result, Err(PsiOptimizationStageError::InvalidModule(_))),
        "the malformed carrier fails closed: {result:?}"
    );
}

#[test]
fn a_recorded_call_join_retains_its_environment_through_publication() {
    // Source-call custody through the entrance: m1 carries a CallUnit to m2
    // and a dead total constant v10 the recorded join captures; the sibling
    // v12 is the contrast that dies. Reentry is legal and stable because the
    // occurrence rows ride the published carrier.
    let mut lowered = lowered(vec![
        machine(
            1,
            Vec::new(),
            TerminalMachineResult::Scalar(declaration(9, ScalarType::Boolean)),
            block_id(1),
            vec![block(
                1,
                Vec::new(),
                vec![
                    operation(
                        10,
                        boolean(10),
                        OperationKind::BooleanConstant { value: true },
                    ),
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(11).unwrap(),
                        result: OperationResult::Unit,
                        kind: OperationKind::CallUnit {
                            erased_arguments: Vec::new(),
                            callee: MachineId::new(2).unwrap(),
                            arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    },
                    operation(
                        12,
                        boolean(12),
                        OperationKind::BooleanConstant { value: false },
                    ),
                    operation(
                        13,
                        boolean(13),
                        OperationKind::BooleanConstant { value: true },
                    ),
                ],
                Terminator::Return {
                    edge: edge(1),
                    value: value(13),
                    cleanup_actions: Vec::new(),
                },
            )],
        ),
        machine(
            2,
            Vec::new(),
            TerminalMachineResult::Unit,
            block_id(21),
            vec![block(
                21,
                Vec::new(),
                Vec::new(),
                Terminator::ReturnUnit {
                    edge: edge(21),
                    trivial_affine_discards: Vec::new(),
                },
            )],
        ),
    ]);
    let occurrence = |operation: u64| LoweredSourceCallOccurrence {
        source_site: None,
        source_state: symbols::SymbolHandle::from_arena_index(1),
        statement_index: 0,
        call_ordinal: 0,
        terminal_operation: OperationId::new(operation).unwrap(),
        source_target: symbols::SymbolHandle::from_arena_index(2),
        source_values_before_call: vec![boolean(10)],
    };
    lowered.source_call_occurrences = vec![occurrence(11)];

    let joined = run_psi_optimization(
        lowered.clone(),
        selections(&[PsiOptimization::DeadPureScalarElimination]),
    )
    .unwrap();
    let remaining = joined.lowered().semantic_module.machines[0].blocks[0]
        .operations
        .iter()
        .filter_map(|operation| operation.result.scalar().map(|result| result.id))
        .collect::<Vec<_>>();
    assert!(
        remaining.contains(&value(10)) && !remaining.contains(&value(12)),
        "the joined capture keeps v10; the unnamed v12 dies: {remaining:?}"
    );

    // The published artifact with its occurrences intact is a legal second
    // input and already at the fixed point.
    let reentry = run_psi_optimization(
        joined.lowered().clone(),
        selections(&[PsiOptimization::DeadPureScalarElimination]),
    )
    .unwrap();
    assert_eq!(reentry.lowered(), joined.lowered());

    // Boundary: a join naming an operation absent from the module retains
    // nothing, so the captured candidate dies with its sibling.
    let mut absent = lowered.clone();
    absent.source_call_occurrences = vec![occurrence(99)];
    let absent_result = run_psi_optimization(
        absent,
        selections(&[PsiOptimization::DeadPureScalarElimination]),
    )
    .unwrap();
    let absent_remaining = absent_result.lowered().semantic_module.machines[0].blocks[0]
        .operations
        .iter()
        .filter_map(|operation| operation.result.scalar().map(|result| result.id))
        .collect::<Vec<_>>();
    assert_eq!(
        absent_remaining,
        vec![value(13)],
        "a join naming no operation retains nothing"
    );
}
