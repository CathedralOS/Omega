//! Canonical controls for one relation and every edge of actual CFG cycles.
use super::*;
use crate::reverify;
use terminal_check_tables::obligations::ObligationRow;
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc, Terminator,
};

fn ranked(signed: bool, tuple: bool, stalls: bool) -> typed_trees_to_terminal_psi::Elaboration {
    let scalar = if signed {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    } else {
        u32_type()
    };
    let literal = |number: i128| {
        super::literal(
            scalar,
            if signed {
                IntegerValue::Signed(number)
            } else {
                IntegerValue::Unsigned(number as u128)
            },
        )
    };
    let compare = |limit| {
        expression(
            ExpressionKind::Strict {
                operation: Strict::Greater,
                left: Box::new(slot(0, scalar)),
                right: Box::new(literal(limit)),
            },
            Some(ScalarType::Boolean),
        )
    };
    let mut actuals = vec![strict(Strict::Subtract, slot(0, scalar), literal(1))];
    if tuple {
        actuals.push(literal(0));
    }
    let mut recurse = call(1, actuals);
    recurse.result = Some(ValueType::plain(scalar));
    let mut statements = vec![guarded(
        compare(if signed {
            -5
        } else if stalls {
            1
        } else {
            0
        }),
        recurse,
    )];
    if stalls {
        let mut recurse = call(1, vec![slot(0, scalar)]);
        recurse.result = Some(ValueType::plain(scalar));
        statements.push(guarded(compare(0), recurse));
    }
    statements.push(returns(literal(0)));
    let mut body = machine(vec![scalar; if tuple { 2 } else { 1 }], statements);
    body.result = scalar;
    let mut elaboration = lower(vec![body]).unwrap();
    let machine = &mut elaboration.module.machines[0];
    let members = terminal_semantics::control_graph::control_cycle_members(machine).unwrap();
    assert_eq!(members.len(), 1);
    let members = &members[0];
    let header = machine.block(&members[0]).unwrap();
    let header_id = header.id;
    let ScalarType::Integer(integer_type) = scalar else {
        unreachable!()
    };
    let ranks = header
        .parameters
        .iter()
        .map(|parameter| TerminalIntegerRankValue {
            integer_type,
            value: parameter.id,
        })
        .collect::<Vec<_>>();
    assert_eq!(ranks.len(), if tuple { 2 } else { 1 });
    let mut cycle = TerminalNaturalCycle {
        observations: Default::default(),
        ranks: Vec::new(),
        edges: Vec::new(),
    };
    for block in members {
        let values = cycle.observations.insert_many(ranks.clone());
        cycle.ranks.push(TerminalBlockNaturalRank {
            block: *block,
            values,
        });
    }
    for block in machine
        .blocks
        .iter()
        .filter(|block| members.contains(&block.id))
    {
        for (edge, target) in block
            .terminator
            .successor_edges()
            .into_iter()
            .filter(|(_, target)| members.contains(target))
        {
            let successors = if target == header_id {
                let arguments = match &block.terminator {
                    Terminator::Jump { arguments, .. } => arguments,
                    _ => panic!("tail jump"),
                };
                arguments
                    .iter()
                    .map(|value| TerminalIntegerRankValue {
                        integer_type,
                        value: *value,
                    })
                    .collect::<Vec<_>>()
            } else {
                ranks.clone()
            };
            let comparison = if target == header_id && successors[0].value != ranks[0].value {
                TerminalNaturalRankComparison::Strict
            } else {
                TerminalNaturalRankComparison::Preserving
            };
            let successor_ranks = cycle.observations.insert_many(successors);
            cycle.edges.push(TerminalNaturalRankEdge {
                edge,
                source: block.id,
                target,
                successor_ranks,
                comparison,
            });
        }
    }
    let component = terminal_semantics::control_graph::control_cycle_identity(machine, &cycle);
    let tables = elaboration.tables.machines.get_mut(machine.id).unwrap();
    tables.obligations.push(ObligationRow {
        kind: ObligationKind::WellFoundedness { component },
        created_at: ProgramPoint::BlockEntry(members[0]),
    });
    for edge in &cycle.edges {
        tables.obligations.push(ObligationRow {
            kind: ObligationKind::RankComparison {
                component,
                edge: edge.edge,
            },
            created_at: ProgramPoint::Edge(edge.edge),
        });
    }
    machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![cycle]));
    machine.contract.termination = semantic_vocabulary::Termination::Terminates;
    elaboration
}

#[test]
fn finite_integer_rankings_use_real_kernel_checked_edge_goals() {
    for (signed, tuple) in [(false, false), (true, false), (false, true)] {
        let mut elaboration = ranked(signed, tuple, false);
        let discharged = discharge(&mut elaboration).unwrap();
        let tables = elaboration.tables.machines.get(id(1)).unwrap();
        let verdicts = &discharged.machines.get(id(1)).unwrap().verdicts;
        for (obligation, row) in tables.obligations.iter() {
            match row.kind {
                ObligationKind::WellFoundedness { component } => assert!(
                    matches!(verdicts.get(obligation), Some(Ok(Discharged::FiniteIntegerRanking(relation))) if relation.component == component && relation.carriers.len() == if tuple { 2 } else { 1 })
                ),
                ObligationKind::RankComparison { .. } => {
                    let Some(Ok(Discharged::Certificate(evidence))) = verdicts.get(obligation)
                    else {
                        panic!("rank edge has no certificate")
                    };
                    assert_kernel_checked(&elaboration, 1, obligation, evidence);
                    reverify(
                        &mut elaboration.clone(),
                        id(1),
                        obligation,
                        &evidence.certificate,
                    )
                    .unwrap();
                }
                _ => {}
            }
        }
    }
}

#[test]
fn exact_ranking_inventory_rejects_missing_surplus_swapped_and_stale_claims() {
    for fault in 0..8 {
        let mut elaboration = ranked(false, false, false);
        let machine = &mut elaboration.module.machines[0];
        let Some(TerminalRankedScc::Natural(cycles)) = &mut machine.ranked_scc else {
            unreachable!()
        };
        let cycle = &mut cycles[0];
        match fault {
            0 => {
                cycle.ranks.pop();
            }
            1 => {
                cycle.edges.pop();
            }
            2 => cycle.ranks.swap(0, 1),
            3 => cycle.edges.reverse(),
            4 => {
                cycle.edges[0].source = cycle.edges[0].target;
            }
            5 => {
                let tables = elaboration.tables.machines.get_mut(machine.id).unwrap();
                let duplicate = *tables
                    .obligations
                    .iter()
                    .find(|(_, row)| matches!(row.kind, ObligationKind::WellFoundedness { .. }))
                    .unwrap()
                    .1;
                tables.obligations.push(duplicate);
            }
            6 => {
                let tables = elaboration.tables.machines.get_mut(machine.id).unwrap();
                let last = tables.obligations.len();
                tables.obligations = IdTable::from_rows(
                    tables
                        .obligations
                        .iter()
                        .take(last - 1)
                        .map(|(id, row)| (id, *row)),
                )
                .expect("the retained obligation prefix remains dense");
            }
            7 => {
                let value = ranks_value(cycle);
                let wrong = cycle
                    .observations
                    .insert_many(vec![TerminalIntegerRankValue {
                        integer_type: IntegerType::supported_address_representation(),
                        value,
                    }]);
                cycle.ranks[0].values = wrong;
            }
            _ => unreachable!(),
        }
        assert!(
            discharge(&mut elaboration).is_err(),
            "fault {fault} accepted"
        );
    }
}

#[test]
fn an_authored_countdown_measure_must_decrease_on_the_recursive_route() {
    let source = |next: &str| {
        format!(
            r#"
machine countdown(n: u32) -> u32
terminates by n -> Nat::Descending
crashes none
{{
    transition n > 0 {{
        true -> countdown({next})
        false -> 37
    }}
}}
machine entry() -> u32 terminates crashes none {{ countdown(4) }}
"#
        )
    };
    let verdicts = |next| {
        super::boundary_calls::compilation_results_of(&super::boundary_calls::typed(&source(next)))
    };
    let positive = verdicts("n - 1");
    assert!(
        positive
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "the authored countdown must retain and prove its measure"
    );
    for next in ["n", "n + 1"] {
        let verdicts = verdicts(next);
        assert!(
            verdicts.iter().any(|verdict| matches!(
                verdict, Ok(RouteVerdict::Rejected { unproved, .. })
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::RankComparison { .. }))
            ) || matches!(
                verdict, Ok(RouteVerdict::FallBack(refusal))
                    | Err(crate::CompilationRefusal::Elaboration(refusal))
                if refusal.detail.contains("removing strict rank edges")
            )),
            "the {next} recursive route must fail its progress obligation"
        );
    }
}

fn ranks_value(cycle: &TerminalNaturalCycle) -> semantic_vocabulary::ValueId {
    cycle.rank_values(cycle.ranks[0].values).unwrap()[0].value
}

#[test]
fn every_complete_cycle_must_cross_a_strict_edge() {
    let mut elaboration = ranked(false, false, true);
    let refusal = discharge(&mut elaboration).unwrap_err();
    assert!(
        refusal.detail.contains("removing strict rank edges"),
        "{}",
        refusal.detail
    );
}

#[test]
fn a_forged_successor_cannot_substitute_a_smaller_unrelated_value() {
    let mut elaboration = ranked(false, false, false);
    let machine = &mut elaboration.module.machines[0];
    let zero = machine
        .operations()
        .find_map(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0)
                }
            )
            .then(|| operation.result.scalar_ref().unwrap().id)
        })
        .unwrap();
    let Some(TerminalRankedScc::Natural(cycles)) = &mut machine.ranked_scc else {
        unreachable!()
    };
    let cycle = &mut cycles[0];
    let last = cycle
        .edges
        .iter()
        .position(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
        .unwrap();
    let old = cycle.edges[last].successor_ranks;
    cycle.observations.get_mut(old.start()).value = zero;
    let refusal = discharge(&mut elaboration).unwrap_err();
    assert!(
        refusal.detail.contains("successor rank") || refusal.detail.contains("actual scope"),
        "{}",
        refusal.detail
    );
}
