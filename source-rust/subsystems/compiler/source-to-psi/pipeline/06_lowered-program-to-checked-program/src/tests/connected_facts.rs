// `decide` first gives the prover and kernel only the facts that share a
// value with the goal, directly or through other facts, plus facts that name
// no value (`crate::connected`). These tests pin which facts that keeps.

use super::{id, number, u32_type};
use crate::{Hypotheses, connected};
use arena::IdTable;
use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
use terminal_check_tables::KnowledgeId;

fn value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(id::<ValueId>(raw), u32_type())
}

fn at_most(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    Proposition::LessOrEqual(left, right)
}

/// Rows for `assumptions` then `axioms`, in that order.
fn hypotheses(assumptions: Vec<Proposition>, axioms: Vec<Proposition>) -> Hypotheses {
    let mut rows: IdTable<KnowledgeId, ()> = IdTable::default();
    Hypotheses {
        assumption_rows: assumptions.iter().map(|_| rows.push(())).collect(),
        axiom_rows: axioms.iter().map(|_| rows.push(())).collect(),
        assumptions,
        axioms,
    }
}

#[test]
fn a_fact_about_unrelated_values_is_left_out_and_a_chain_is_kept() {
    // Goal `v1 <= 10`. `v1 <= v2` and `v2 <= 10` reach it through `v2`;
    // `1 <= 2` names no value; `v3 <= 5` and `v4 <= v3` share nothing with it.
    let goal = at_most(value(1), number(10));
    let known = hypotheses(
        vec![
            at_most(value(1), value(2)),
            at_most(value(3), number(5)),
            at_most(number(1), number(2)),
        ],
        vec![at_most(value(4), value(3)), at_most(value(2), number(10))],
    );
    let narrowed = connected(&goal, &known).expect("unrelated facts are dropped");
    assert_eq!(
        narrowed.assumptions,
        [known.assumptions[0].clone(), known.assumptions[2].clone()]
    );
    assert_eq!(
        narrowed.assumption_rows,
        [known.assumption_rows[0], known.assumption_rows[2]]
    );
    assert_eq!(narrowed.axioms, [known.axioms[1].clone()]);
    assert_eq!(narrowed.axiom_rows, [known.axiom_rows[1]]);
}

#[test]
fn when_every_fact_is_connected_the_full_set_is_used() {
    let goal = at_most(value(1), number(10));
    let known = hypotheses(
        vec![at_most(value(1), value(2))],
        vec![at_most(value(2), value(3))],
    );
    assert!(connected(&goal, &known).is_none());
}

#[test]
fn knowledge_index_matches_hypotheses_and_connected_across_straight_line_and_edges() {
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerValue, MachineId, ObligationId, OperationId,
    };
    use terminal_check_tables::edges::{Destination, EdgeRow};
    use terminal_check_tables::values::{Justification, KnowledgeRow, ValueOrigin, ValueRow};
    use terminal_check_tables::{Construct, MachineTables, ProgramPoint};
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        Terminator,
    };

    let mut tables = MachineTables::default();
    for pos in 0..6 {
        tables.values.push(ValueRow {
            scalar_type: u32_type(),
            origin: ValueOrigin::MachineParameter { position: pos },
            rigid: None,
        });
    }
    let op = |raw| {
        Operation::unsuspended(
            id::<OperationId>(raw),
            OperationResult::Unit,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        )
    };
    // b1 -> b2 (edge 1, sole pred), b1 -> b3 (edge 2), b2 -> b3 (edge 3)
    for (src, dst) in [(1, 2), (1, 3), (2, 3)] {
        tables.edges.push(EdgeRow {
            source: id::<BlockId>(src),
            destination: Destination::Block(id::<BlockId>(dst)),
            dispositions: Vec::new(),
        });
    }
    let mut block1 = Block::plain(
        id::<BlockId>(1),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: id::<EdgeId>(1),
            cleanup_actions: Vec::new(),
        },
    );
    block1.operations = vec![op(1), op(2), op(3)];
    let mut block2 = Block::plain(
        id::<BlockId>(2),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: id::<EdgeId>(3),
            cleanup_actions: Vec::new(),
        },
    );
    block2.operations = vec![op(4)];
    let mut block3 = Block::plain(
        id::<BlockId>(3),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: id::<EdgeId>(3),
            cleanup_actions: Vec::new(),
        },
    );
    block3.operations = vec![op(5)];
    let lowered = TerminalMachine {
        call_formations: Vec::new(),
        assembly_target_requirement: Default::default(),
        id: id::<MachineId>(1),
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id::<BlockId>(1),
        blocks: vec![block1, block2, block3],
        contract: MachineContract::empty(id::<ContractId>(1)),
    };
    let mut add_fact =
        |prop: Proposition, available_from: ProgramPoint, justification: Justification| {
            let proposition = tables.propositions.push(prop);
            tables.knowledge.push(KnowledgeRow {
                proposition,
                available_from,
                justification,
                premises: Vec::new(),
            });
        };
    // Chain for v1 -> v2 -> v3, plus unrelated facts & value-free fact across blocks/edges.
    add_fact(
        at_most(value(1), value(2)),
        ProgramPoint::BlockEntry(id::<BlockId>(1)),
        Justification::EntryRequirement { clause: 0 },
    );
    add_fact(
        at_most(value(4), value(5)),
        ProgramPoint::AfterOperation(id::<OperationId>(1)),
        Justification::OperationSemantics(id::<OperationId>(1)),
    );
    add_fact(
        at_most(value(2), value(3)),
        ProgramPoint::AfterOperation(id::<OperationId>(2)),
        Justification::OperationSemantics(id::<OperationId>(2)),
    );
    add_fact(
        at_most(number(1), number(2)),
        ProgramPoint::AfterOperation(id::<OperationId>(3)),
        Justification::FormalType,
    );
    add_fact(
        at_most(value(3), value(6)),
        ProgramPoint::Edge(id::<EdgeId>(1)),
        Justification::BranchCondition {
            edge: id::<EdgeId>(1),
            holds: true,
        },
    );
    add_fact(
        at_most(value(5), value(6)),
        ProgramPoint::Edge(id::<EdgeId>(2)),
        Justification::BranchCondition {
            edge: id::<EdgeId>(2),
            holds: false,
        },
    );

    let graph = crate::graph::Graph::new(lowered.id, &lowered, &tables).unwrap();
    let mut knowledge = crate::KnowledgeIndex::new(&graph, &tables, lowered.id).unwrap();
    let goal = at_most(value(1), number(10));
    let points = [
        ProgramPoint::BlockEntry(id::<BlockId>(1)),
        ProgramPoint::AfterOperation(id::<OperationId>(2)),
        ProgramPoint::AfterOperation(id::<OperationId>(3)),
        ProgramPoint::Edge(id::<EdgeId>(1)),
        ProgramPoint::AfterOperation(id::<OperationId>(4)),
        ProgramPoint::Edge(id::<EdgeId>(2)),
        ProgramPoint::AfterOperation(id::<OperationId>(5)),
    ];
    for point in points {
        let expected_full =
            crate::hypotheses(&graph, &tables, lowered.id, id::<ObligationId>(1), point).unwrap();
        let actual_full = knowledge
            .full(
                &graph,
                &tables,
                lowered.id,
                Construct::Obligation(id::<ObligationId>(1)),
                point,
            )
            .unwrap();
        assert_eq!(actual_full.assumption_rows, expected_full.assumption_rows);
        assert_eq!(actual_full.axiom_rows, expected_full.axiom_rows);
        assert_eq!(actual_full.assumptions, expected_full.assumptions);
        assert_eq!(actual_full.axioms, expected_full.axioms);

        let expected_narrow = crate::connected(&goal, &expected_full);
        match (
            knowledge
                .narrowed_or_full(
                    &graph,
                    &tables,
                    lowered.id,
                    Construct::Obligation(id::<ObligationId>(1)),
                    point,
                    &goal,
                )
                .unwrap(),
            expected_narrow,
        ) {
            (crate::NarrowedKnowledge::Narrowed(actual), Some(expected)) => {
                assert_eq!(actual.assumption_rows, expected.assumption_rows);
                assert_eq!(actual.axiom_rows, expected.axiom_rows);
                assert_eq!(actual.assumptions, expected.assumptions);
                assert_eq!(actual.axioms, expected.axioms);
            }
            (crate::NarrowedKnowledge::All(actual), None) => {
                assert_eq!(actual.assumption_rows, expected_full.assumption_rows);
                assert_eq!(actual.axiom_rows, expected_full.axiom_rows);
                assert_eq!(actual.assumptions, expected_full.assumptions);
                assert_eq!(actual.axioms, expected_full.axioms);
            }
            _ => panic!("mismatch between KnowledgeIndex and hypotheses+connected at {point:?}"),
        }
    }
}
