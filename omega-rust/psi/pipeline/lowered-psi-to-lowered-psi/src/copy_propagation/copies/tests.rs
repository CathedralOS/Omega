//! Copy propagation copy tests.

//! Machine-level boundary coverage for cases a validated module cannot
//! express: cycles, ranking evidence, positional case payloads, retained
//! proof values, and declaration mismatches.
use super::{BTreeSet, BlockId, TerminalMachine, Terminator, ValueId, propagate};
use lowered_psi::LoweredSourceCallOccurrence;
use semantic_vocabulary::{
    ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId, Proposition,
    ScalarQualificationSetId, ScalarTerm, ScalarType, StructuralCaseId, StructuralFieldId,
    StructuralTypeId,
};
use terminal_psi::{
    Block, ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    MachineContract, Operation, OperationKind, OperationResult, OutcomeSpecificEnsure,
    OutcomeSpecificGuard, StructuralCaseSuccessorEdge, TerminalMachineResult, TerminalRankedScc,
    ValueDeclaration,
};

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}

fn declaration(ordinal: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(ordinal).unwrap(),
        scalar_type: ScalarType::Boolean,
    }
}

fn machine(blocks: Vec<Block>) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(1).unwrap(),
        attachment: None,
        parameters: vec![declaration(1)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(declaration(9)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks,
        contract: MachineContract {
            id: ContractId::new(1).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
            erased_scalar_formals: Vec::new(),
        },
    }
}

fn block(ordinal: u64, parameters: Vec<ValueDeclaration>, terminator: Terminator) -> Block {
    Block {
        structural_parameters: Vec::new(),
        id: BlockId::new(ordinal).unwrap(),
        parameters,
        erased_scalar_formals: Vec::new(),
        operations: Vec::new(),
        terminator,
    }
}

fn jump(ordinal: u64, target: u64, arguments: Vec<u64>) -> Terminator {
    Terminator::Jump {
        edge: EdgeId::new(ordinal).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: arguments
            .into_iter()
            .map(|ordinal| ValueId::new(ordinal).unwrap())
            .collect(),
        erased_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn edge_arguments(block: &Block) -> Vec<ValueId> {
    match &block.terminator {
        Terminator::Jump { arguments, .. } => arguments.clone(),
        _ => panic!("expected a jump terminator"),
    }
}

fn parameter_ids(block: &Block) -> Vec<ValueId> {
    block
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

#[test]
fn uniform_copy_collapses_and_substitutes_the_return_use() {
    let mut machine = machine(vec![
        block(1, vec![], jump(1, 2, vec![1])),
        block(
            2,
            vec![declaration(2)],
            Terminator::Return {
                edge: EdgeId::new(2).unwrap(),
                value: ValueId::new(2).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ),
    ]);
    propagate(&mut machine, &[], &mut BTreeSet::new());
    assert!(machine.blocks[1].parameters.is_empty());
    assert_eq!(edge_arguments(&machine.blocks[0]), vec![]);
    let Terminator::Return { value, .. } = &machine.blocks[1].terminator else {
        panic!("b2 keeps its return")
    };
    assert_eq!(*value, ValueId::new(1).unwrap());
}

#[test]
fn divergent_edge_bindings_keep_the_parameter_and_arguments() {
    let mut machine = machine(vec![
        block(1, vec![], jump(1, 3, vec![1])),
        block(2, vec![], jump(2, 3, vec![9])),
        block(
            3,
            vec![declaration(3)],
            Terminator::Return {
                edge: EdgeId::new(3).unwrap(),
                value: ValueId::new(3).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ),
    ]);
    propagate(&mut machine, &[], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&machine.blocks[2]),
        vec![ValueId::new(3).unwrap()]
    );
    assert_eq!(
        edge_arguments(&machine.blocks[0]),
        vec![ValueId::new(1).unwrap()]
    );
    assert_eq!(
        edge_arguments(&machine.blocks[1]),
        vec![ValueId::new(9).unwrap()]
    );
}

#[test]
fn copy_chain_collapses_transitively() {
    let mut machine = machine(vec![
        block(1, vec![], jump(1, 2, vec![1])),
        block(2, vec![declaration(2)], jump(2, 3, vec![2])),
        block(
            3,
            vec![declaration(3)],
            Terminator::Return {
                edge: EdgeId::new(3).unwrap(),
                value: ValueId::new(3).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ),
    ]);
    propagate(&mut machine, &[], &mut BTreeSet::new());
    assert!(machine.blocks[1].parameters.is_empty());
    assert!(machine.blocks[2].parameters.is_empty());
    assert_eq!(edge_arguments(&machine.blocks[0]), vec![]);
    assert_eq!(edge_arguments(&machine.blocks[1]), vec![]);
    let Terminator::Return { value, .. } = &machine.blocks[2].terminator else {
        panic!("b3 keeps its return")
    };
    assert_eq!(*value, ValueId::new(1).unwrap());
}

#[test]
fn cyclic_bindings_resolve_to_one_representative() {
    // The two-parameter cycle `p2 <- p3 <- p2` resolves to p2: p3 is a
    // copy of the representative, p2 stands for itself.
    let mut machine = machine(vec![
        block(1, vec![], jump(1, 2, vec![1])),
        block(2, vec![declaration(2)], jump(2, 3, vec![2])),
        block(3, vec![declaration(3)], jump(3, 2, vec![3])),
    ]);
    propagate(&mut machine, &[], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&machine.blocks[1]),
        vec![ValueId::new(2).unwrap()]
    );
    assert!(machine.blocks[2].parameters.is_empty());
    assert_eq!(
        edge_arguments(&machine.blocks[1]),
        vec![],
        "the edge into b3 drops the removed position"
    );
    assert_eq!(
        edge_arguments(&machine.blocks[2]),
        vec![ValueId::new(2).unwrap()],
        "the surviving edge rebinds the representative"
    );
}

#[test]
fn covered_component_coordinates_stay_while_outside_copies_collapse() {
    // b1 enters the covered self-loop member b2 carrying parameters r
    // and w; w binds v1 on every incoming edge — copy-shaped — but b2's
    // table is covered and stays. The exit block's parameter x is an
    // ordinary copy and collapses, dropping the member's exit-edge
    // argument position.
    let rank_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
            .unwrap();
    let rank_declaration = |ordinal: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(ordinal).unwrap(),
        scalar_type: ScalarType::Integer(rank_type),
    };
    let mut machine = machine(vec![
        block(1, vec![], jump(1, 2, vec![1, 1])),
        block(
            2,
            vec![rank_declaration(10), declaration(11)],
            Terminator::Conditional {
                condition: ValueId::new(1).unwrap(),
                when_true: terminal_psi::SuccessorEdge {
                    edge: EdgeId::new(2).unwrap(),
                    target: BlockId::new(2).unwrap(),
                    arguments: vec![ValueId::new(12).unwrap(), ValueId::new(1).unwrap()],
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: terminal_psi::SuccessorEdge {
                    edge: EdgeId::new(3).unwrap(),
                    target: BlockId::new(3).unwrap(),
                    arguments: vec![ValueId::new(1).unwrap()],
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        ),
        block(
            3,
            vec![declaration(13)],
            Terminator::Return {
                edge: EdgeId::new(4).unwrap(),
                value: ValueId::new(13).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ),
    ]);
    machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![
        terminal_psi::TerminalNaturalCycle {
            rank_type,
            ranks: vec![terminal_psi::TerminalBlockNaturalRank {
                block: BlockId::new(2).unwrap(),
                value: ValueId::new(10).unwrap(),
            }],
            edges: vec![terminal_psi::TerminalNaturalRankEdge {
                edge: EdgeId::new(2).unwrap(),
                source: BlockId::new(2).unwrap(),
                target: BlockId::new(2).unwrap(),
                successor_rank: ValueId::new(12).unwrap(),
                comparison: terminal_psi::TerminalNaturalRankComparison::Strict,
            }],
        },
    ]));
    propagate(&mut machine, &[], &mut BTreeSet::new());
    let member = &machine.blocks[1];
    assert_eq!(
        parameter_ids(member),
        vec![ValueId::new(10).unwrap(), ValueId::new(11).unwrap()],
        "the covered parameter table stays exact even for copy-shaped w"
    );
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &member.terminator
    else {
        panic!("the member keeps its conditional")
    };
    assert_eq!(
        when_true.arguments,
        vec![ValueId::new(12).unwrap(), ValueId::new(1).unwrap()],
        "the covered backedge keeps its exact arguments"
    );
    assert_eq!(
        when_false.arguments,
        vec![],
        "the exit edge drops the collapsed parameter's position"
    );
    assert!(
        machine.blocks[2].parameters.is_empty(),
        "the uncovered exit parameter collapses to v1"
    );
    let Terminator::Return { value, .. } = &machine.blocks[2].terminator else {
        panic!("b3 keeps its return")
    };
    assert_eq!(*value, ValueId::new(1).unwrap());
}

#[test]
fn structural_case_target_keeps_its_positional_bindings() {
    // Structural-case payload bindings are positional, not listed
    // arguments: even a uniform Jump inventory cannot collapse them.
    let mut machine = machine(vec![
        block(1, vec![], jump(1, 2, vec![1])),
        block(
            2,
            vec![declaration(2)],
            Terminator::Return {
                edge: EdgeId::new(2).unwrap(),
                value: ValueId::new(2).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ),
        block(
            3,
            vec![],
            Terminator::StructuralCase {
                source: PlaceId::new(1).unwrap(),
                cases: vec![StructuralCaseSuccessorEdge {
                    edge: EdgeId::new(3).unwrap(),
                    target: BlockId::new(2).unwrap(),
                    case: StructuralCaseId::new(1).unwrap(),
                    payload_fields: vec![StructuralFieldId::new(1).unwrap()],
                    trivial_affine_discards: Vec::new(),
                }],
            },
        ),
    ]);
    propagate(&mut machine, &[], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&machine.blocks[1]),
        vec![ValueId::new(2).unwrap()],
        "a structural-case target is never a copy"
    );
    assert_eq!(
        edge_arguments(&machine.blocks[0]),
        vec![ValueId::new(1).unwrap()]
    );
}

#[test]
fn retained_and_proposition_named_values_are_never_collapsed() {
    let build = || {
        machine(vec![
            block(1, vec![], jump(1, 2, vec![1])),
            block(
                2,
                vec![declaration(2)],
                Terminator::Return {
                    edge: EdgeId::new(2).unwrap(),
                    value: ValueId::new(2).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ),
        ])
    };
    let mut retained_machine = build();
    let mut retained = BTreeSet::from([ValueId::new(2).unwrap()]);
    propagate(&mut retained_machine, &[], &mut retained);
    assert_eq!(
        parameter_ids(&retained_machine.blocks[1]),
        vec![ValueId::new(2).unwrap()]
    );

    let mut proposition_machine = build();
    proposition_machine
        .contract
        .requires
        .push(Proposition::Equal(
            semantic_vocabulary::ScalarTerm::Value {
                id: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Boolean,
            },
            semantic_vocabulary::ScalarTerm::Boolean(true),
        ));
    propagate(&mut proposition_machine, &[], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&proposition_machine.blocks[1]),
        vec![ValueId::new(2).unwrap()],
        "a proposition's value identities are uses, not removable copies"
    );
}

#[test]
fn mismatched_resolved_source_declaration_keeps_the_parameter() {
    // The uniform binding alone is not enough: a resolved source whose
    // declaration disagrees on carrier or qualifications rejects the
    // substitution.
    for parameter in [
        ValueDeclaration {
            qualifications: ScalarQualificationSetId::new(7),
            id: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                    .unwrap(),
            ),
        },
    ] {
        let mut machine = machine(vec![
            block(1, vec![], jump(1, 2, vec![1])),
            block(
                2,
                vec![parameter],
                Terminator::Return {
                    edge: EdgeId::new(2).unwrap(),
                    value: ValueId::new(2).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ),
        ]);
        propagate(&mut machine, &[], &mut BTreeSet::new());
        assert_eq!(
            parameter_ids(&machine.blocks[1]),
            vec![ValueId::new(2).unwrap()],
            "a {parameter:?} mismatch keeps the declared parameter"
        );
        assert_eq!(
            edge_arguments(&machine.blocks[0]),
            vec![ValueId::new(1).unwrap()]
        );
    }
}

#[test]
fn block_without_inventoried_incoming_edges_keeps_parameters() {
    let mut machine = machine(vec![
        block(
            1,
            vec![],
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        ),
        block(
            2,
            vec![declaration(2)],
            Terminator::Return {
                edge: EdgeId::new(2).unwrap(),
                value: ValueId::new(2).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ),
    ]);
    propagate(&mut machine, &[], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&machine.blocks[1]),
        vec![ValueId::new(2).unwrap()]
    );
}

fn equal_v2_true() -> Proposition {
    Proposition::Equal(
        ScalarTerm::Value {
            id: value(2),
            scalar_type: ScalarType::Boolean,
        },
        ScalarTerm::Boolean(true),
    )
}

/// b1 jumps to b2 binding both of b2's parameters to machine parameter v1:
/// v2 and v3 are copy-shaped siblings, so a carrier naming v2 leaves v3 to
/// collapse as the observable contrast.
fn two_copy_machine() -> TerminalMachine {
    machine(vec![
        block(1, vec![], jump(1, 2, vec![1, 1])),
        block(
            2,
            vec![declaration(2), declaration(3)],
            Terminator::Return {
                edge: EdgeId::new(2).unwrap(),
                value: ValueId::new(3).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ),
    ])
}

fn collapse_isolates_named_value(mut machine: TerminalMachine) {
    // After propagation the named v2 stays a parameter of b2 while its
    // unnamed sibling v3 collapses and its uses substitute v1.
    propagate(&mut machine, &[], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&machine.blocks[1]),
        vec![value(2)],
        "the carrier-named copy keeps its identity"
    );
    let Terminator::Return { value: result, .. } = &machine.blocks[1].terminator else {
        panic!("the block still returns");
    };
    assert_eq!(*result, value(1), "the collapsed sibling substitutes v1");
    let Terminator::Jump { arguments, .. } = &machine.blocks[0].terminator else {
        panic!("the entry still jumps");
    };
    assert_eq!(*arguments, vec![value(1)], "the dropped position leaves");
}

#[test]
fn ensures_and_outcome_propositions_retain_named_copies() {
    // Positive: ensures clauses and outcome-specific guarantees are contract
    // proof terms like requires — the copy-shaped parameter each names keeps
    // its identity while an unnamed sibling still collapses.
    let mut ensures_machine = two_copy_machine();
    ensures_machine.contract.ensures.push(ContractClause {
        obligation: ObligationId::new(1).unwrap(),
        proposition: equal_v2_true(),
    });
    collapse_isolates_named_value(ensures_machine);

    let mut outcome_machine = two_copy_machine();
    outcome_machine
        .contract
        .outcome_specific_ensures
        .push(OutcomeSpecificEnsure {
            guard: OutcomeSpecificGuard {
                result_type: StructuralTypeId::new(1).unwrap(),
                result_case: StructuralCaseId::new(1).unwrap(),
            },
            position: 0,
            obligation: ObligationId::new(2).unwrap(),
            proposition: equal_v2_true(),
            evidence: None,
        });
    collapse_isolates_named_value(outcome_machine);
}

#[test]
fn crash_route_predicates_retain_named_copies_while_truth_does_not() {
    // Positive: machine crash-route predicates are proof terms naming exact
    // values. Boundary: the canonical `Truth` bucket carries no identity, so
    // both copies collapse.
    let mut route_machine = two_copy_machine();
    route_machine.contract.crash_routes.push(CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            equal_v2_true(),
        ))],
    });
    collapse_isolates_named_value(route_machine);

    let mut truth_machine = two_copy_machine();
    truth_machine.contract.crash_routes.push(CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    });
    propagate(&mut truth_machine, &[], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&truth_machine.blocks[1]),
        Vec::<ValueId>::new(),
        "a Truth alternative retains nothing"
    );
}

#[test]
fn call_continuations_site_guards_and_call_joins_retain_named_copies() {
    // Positive: the remaining proposition carriers — operation crash
    // continuations and crash-terminator site guards — and the recorded
    // source-call join each keep the copy-shaped parameter they name.
    // Boundary: a join naming an absent operation demands nothing.
    let mut continuation_machine = two_copy_machine();
    continuation_machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(10).unwrap(),
        result: OperationResult::Scalar(declaration(50)),
        kind: OperationKind::Call {
            erased_arguments: Vec::new(),
            callee: MachineId::new(9).unwrap(),
            arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: vec![CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                    equal_v2_true(),
                ))],
            }],
        },
    });
    collapse_isolates_named_value(continuation_machine);

    let mut guard_machine = two_copy_machine();
    guard_machine.blocks.push(block(
        3,
        vec![],
        Terminator::Crash {
            edge: EdgeId::new(3).unwrap(),
            cause: CrashCause::Trap,
            site_guard: vec![CrashPredicateTerm::new(equal_v2_true())],
            frontier_lower_bound: Vec::new(),
        },
    ));
    collapse_isolates_named_value(guard_machine);

    let mut join_machine = two_copy_machine();
    let occurrence = |operation: u64| LoweredSourceCallOccurrence {
        source_site: None,
        source_state: symbols::SymbolHandle::from_arena_index(1),
        statement_index: 0,
        call_ordinal: 0,
        terminal_operation: OperationId::new(operation).unwrap(),
        source_target: symbols::SymbolHandle::from_arena_index(2),
        source_values_before_call: vec![declaration(2)],
    };
    join_machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(11).unwrap(),
        result: OperationResult::Scalar(declaration(51)),
        kind: OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
        },
    });
    propagate(&mut join_machine, &[occurrence(11)], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&join_machine.blocks[1]),
        vec![value(2)],
        "a join's captured environment keeps its copy-shaped parameters"
    );

    let mut absent_machine = two_copy_machine();
    propagate(&mut absent_machine, &[occurrence(99)], &mut BTreeSet::new());
    assert_eq!(
        parameter_ids(&absent_machine.blocks[1]),
        Vec::<ValueId>::new(),
        "a join naming no operation in this machine retains nothing"
    );
}
