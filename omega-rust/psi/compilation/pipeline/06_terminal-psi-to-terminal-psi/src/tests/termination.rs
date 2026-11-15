//! A loop that writes `terminates by` is accepted only when its measure is
//! shown to go down on every recursive call. Each accepted program has a
//! twin that differs only in the measure or in the recursive argument and
//! is rejected for that comparison; the forms of the clause the route does
//! not rank yet are refused by name, never accepted unjudged.
//!
//! The source programs are subjects without entries, so every machine of
//! one is judged.

use super::*;
use terminal_psi::{TerminalNaturalRankComparison, TerminalRankedScc};

fn ranked(kind: &ObligationKind) -> bool {
    matches!(
        kind,
        ObligationKind::WellFoundedness { .. } | ObligationKind::RankComparison { .. }
    )
}

/// `machine count_down(remaining: u64) -> u64 terminates by <measure>`,
/// which calls itself with `<next>` while `remaining > 0`.
fn count_down(measure: &str, next: &str) -> String {
    format!(
        "machine count_down(remaining: u64, spare: u64) -> u64\n\
         terminates by {measure} {{\n    \
             transition remaining > 0 {{\n        \
                 true -> count_down({next}, spare)\n        \
                 false -> spare\n    \
             }}\n\
         }}\n"
    )
}

fn accepted(text: &str) -> Box<crate::AcceptedClosure> {
    match judged(text) {
        RouteVerdict::Accepted(accepted) => accepted,
        verdict => panic!("the program must be accepted: {verdict:?}\n{text}"),
    }
}

fn unproved(text: &str) -> Vec<UnprovedObligation> {
    match judged(text) {
        RouteVerdict::Rejected { unproved, .. } => unproved,
        verdict => panic!("the program must be rejected: {verdict:?}\n{text}"),
    }
}

fn refused(text: &str) -> String {
    match judged(text) {
        RouteVerdict::FallBack(refusal) => {
            assert_eq!(refusal.reason, RefusalReason::Unsupported);
            refusal.detail
        }
        verdict => panic!("the program must be refused: {verdict:?}\n{text}"),
    }
}

fn judged(text: &str) -> RouteVerdict {
    super::entryless::judged_typed(&super::rankings::typed_with_ranking_catalog(text), false)
}

#[test]
fn a_measure_that_decreases_on_the_recursive_call_is_accepted() {
    // The fixture's loop (`tests/fixtures/pass/build/
    // activation_yields_without_replay`, `count_down`): under the arm's
    // guard `remaining > 0` the subtraction is safe and its result is
    // strictly below `remaining`.
    let accepted = accepted(&count_down("remaining", "remaining - 1"));
    let lowered = accepted
        .elaboration
        .module
        .machines
        .iter()
        .find(|machine| machine.ranked_scc.is_some())
        .expect("the subject retains its ranked machine");
    // The witness supplies local progress evidence. It does not replace the
    // published promise retained from the header or selected requirement.
    assert_eq!(
        lowered.contract.termination,
        semantic_vocabulary::Termination::NoGuarantee
    );
    let Some(TerminalRankedScc::Natural(cycles)) = &lowered.ranked_scc else {
        panic!("the loop keeps its ranking");
    };
    let strict = cycles[0]
        .edges
        .iter()
        .filter(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
        .count();
    assert_eq!(strict, 1, "the jump back claims the descent");
}

#[test]
fn every_rank_obligation_of_an_accepted_loop_has_checked_evidence() {
    // machine countdown(remaining: u32) -> u32 terminates by remaining
    //   { remaining > 0 -> countdown(remaining - 1); 0 }
    let mut elaboration = lower_ranked(less_one(), witnessed(0));
    let result = discharge(&mut elaboration).unwrap();
    let rows = verdicts(&elaboration, &result, 1, ranked);
    // One shared well-foundedness question and one comparison on each of
    // the cycle's two edges: the edge beneath the head and the jump back.
    assert_eq!(rows.len(), 3);
    for (obligation, verdict) in rows {
        match verdict {
            Ok(Discharged::FiniteIntegerRanking(relation)) => {
                assert_eq!(
                    relation.carriers,
                    vec![
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            32,
                        )
                        .unwrap()
                    ]
                );
                assert!(matches!(elaboration.tables.machines.get(id(1)).unwrap()
                    .obligations.get(obligation).unwrap().kind,
                    ObligationKind::WellFoundedness { component } if component == relation.component));
            }
            _ => {
                assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
            }
        }
    }
    assert!(matches!(
        judge(Ok(lower_ranked(less_one(), witnessed(0)))).unwrap(),
        RouteVerdict::Accepted(_)
    ));
}

fn witnessed(subject: u32) -> Option<Vec<Expression>> {
    Some(vec![slot(subject as usize, u32_type())])
}

fn less_one() -> Expression {
    strict(Strict::Subtract, slot(0, u32_type()), unsigned(1))
}

/// `machine countdown(remaining: u32, spare: u32) -> u32 <termination>
///  { remaining > 0 -> countdown(<next>, spare); 0 }`
fn lower_ranked(
    next: Expression,
    ranking: Option<Vec<Expression>>,
) -> typed_trees_to_terminal_psi::Elaboration {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    let parameters: Vec<ValueType> = vec![u32_type().into(), u32_type().into()];
    let positive = expression(
        ExpressionKind::Strict {
            operation: Strict::Greater,
            policy: None,
            left: Box::new(slot(0, u32_type())),
            right: Box::new(unsigned(0)),
        },
        Some(ScalarType::Boolean),
    );
    bodies.push(Body {
        slots: parameters.clone(),
        statements: vec![
            Statement::Arm {
                guard: Some(positive),
                target: Target::Value(call(1, vec![next, slot(1, u32_type())])),
                source: nowhere(),
            },
            returns(unsigned(0)),
        ],
    });
    signatures.push(Signature {
        operational: Default::default(),
        parameters,
        erased: Vec::new(),
        borrowed: Vec::new(),
        result: Some(u32_type().into()),
        requires: Vec::new(),
        ensures: Vec::new(),
        reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
        invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
        crashes: CrashCeiling::Authored(Vec::new()),
        termination: if ranking.is_some() {
            Termination::Promised
        } else {
            Termination::Silent
        },
        ranking,
    });
    typed_trees_to_terminal_psi::lower_bodies(id(1), &bodies, &signatures, &IdTable::default())
        .unwrap()
}

/// The one unproved obligation of a loop whose measure is not shown to go
/// down: the comparison on the jump back, stated as what the call owes.
fn assert_no_descent(unproved: &[UnprovedObligation]) {
    let [row] = unproved else {
        panic!("one unproved obligation: {unproved:?}");
    };
    assert!(
        matches!(row.kind, ObligationKind::RankComparison { .. }),
        "{row:?}"
    );
    assert_eq!(row.cause, UnprovedCause::NoDerivation);
    assert!(
        row.statement
            .contains("cannot prove the `terminates by` measure decreases at this call"),
        "{}",
        row.statement
    );
}

fn assert_unchanged_measure_refuses(text: &str) {
    let typed = super::rankings::typed_with_ranking_catalog(text);
    match super::entryless::judged_typed_result(&typed, false) {
        Ok(RouteVerdict::Rejected { unproved, .. }) => assert_no_descent(&unproved),
        Err(crate::CompilationRefusal::Elaboration(refusal)) => {
            assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
            assert_eq!(
                refusal.detail,
                "removing strict rank edges leaves a control cycle"
            );
        }
        result => panic!("the unchanged measure must fail its progress check: {result:?}"),
    }
}

#[test]
fn a_recursive_call_that_keeps_the_measure_is_rejected() {
    // The twin differs in the recursive argument alone: `remaining` is
    // passed on unchanged, so `remaining < remaining` is owed and has no
    // derivation.
    assert_unchanged_measure_refuses(&count_down("remaining", "remaining"));
}

#[test]
fn a_measure_the_recursive_call_does_not_decrease_is_rejected() {
    // The twin differs in the measure alone: `spare` is passed on
    // unchanged by the same call that decreases `remaining`.
    accepted(&count_down("remaining", "remaining - 1"));
    assert_unchanged_measure_refuses(&count_down("spare", "remaining - 1"));
}

#[test]
fn a_measure_that_grows_is_rejected() {
    // `climb` calls itself with `level + 1` while `level < 9`: the addition
    // is safe and the arrival keeps `level <= 9`, so the descent is the one
    // thing the call does not establish.
    assert_no_descent(&unproved(
        "machine climb(level: u64) -> u64\n\
         requires level <= 9\n\
         terminates by level {\n    \
             transition level < 9 {\n        \
                 true -> climb(level + 1)\n        \
                 false -> 0\n    \
             }\n\
         }\n",
    ));
}

#[test]
fn a_promise_holds_over_a_ranked_loop_and_not_over_an_unranked_one() {
    // `caller` promises to end and calls the loop. With the loop's witness
    // the promise is judged with it; without, nothing ranks the cycle the
    // promise depends on.
    let program = |clause: &str| {
        format!(
            "machine count_down(remaining: u64) -> u64{clause} {{\n    \
                 transition remaining > 0 {{\n        \
                     true -> count_down(remaining - 1)\n        \
                     false -> 0\n    \
                 }}\n\
             }}\n\
             machine caller() -> u64 terminates {{ count_down(3) }}\n"
        )
    };
    accepted(&program("\nterminates by remaining"));
    assert_eq!(
        refused(&program("")),
        "contract termination promise over a call cycle"
    );
}

#[test]
fn a_witness_form_the_route_does_not_rank_is_refused_by_name() {
    let looping = |header: &str, measure: &str| {
        format!(
            "machine spend({header}) -> u64\n\
             terminates by {measure} {{\n    \
                 transition remaining > 0 {{\n        \
                     true -> spend(remaining - 1, limit)\n        \
                     false -> 0\n    \
                 }}\n\
             }}\n"
        )
    };
    let scalars = "remaining: u64, limit: u64";
    for (measure, detail) in [
        // A rank range: its lower bound would be the floor.
        (
            "remaining -> Nat::Descending in 0..=16",
            "contract ranking view arguments and bounded ranges",
        ),
        // The selected natural view takes one subject.
        (
            "(remaining, limit)",
            "contract ranking view has no normalized integer relation",
        ),
    ] {
        assert_eq!(
            refused(&looping(scalars, measure)),
            format!("{detail} while reading the body of `spend`"),
            "{measure}"
        );
    }
    // A signed subject: `Nat::Descending` is declared for unsigned values.
    assert_eq!(
        refused(
            "machine sink(depth: i32) -> i32\n\
             requires depth >= 0\n\
             terminates by depth -> Nat::Descending {\n    \
                 transition depth > 0 {\n        \
                     true -> sink(depth - 1)\n        \
                     false -> 0\n    \
                 }\n\
             }\n"
        ),
        "contract natural ranking needs an unsigned integer subject while reading the body of `sink`"
    );
}

#[test]
fn a_joint_measure_checks_both_members_of_a_mutual_call_cycle() {
    let accepted = accepted(
        "machine even(n: u64) -> u64 terminates by n {\n    \
                 transition n > 0 { true -> odd(n - 1) false -> 0 }\n\
             }\n\
             machine odd(n: u64) -> u64 terminates by n {\n    \
                 transition n > 0 { true -> even(n - 1) false -> 1 }\n\
             }\n",
    );
    let mut relations = 0;
    let mut comparisons = 0;
    for (machine, tables) in accepted.elaboration.tables.machines.iter() {
        let discharged = accepted.discharge.machines.get(machine).unwrap();
        for (obligation, row) in tables.obligations.iter() {
            match row.kind {
                ObligationKind::WellFoundedness { component } => {
                    assert!(matches!(discharged.verdicts.get(obligation),
                        Some(Ok(Discharged::FiniteIntegerRanking(relation)))
                        if relation.component == component && relation.carriers.len() == 1));
                    relations += 1;
                }
                ObligationKind::RankComparison { .. } => {
                    assert_kernel_checked(
                        &accepted.elaboration,
                        machine.get(),
                        obligation,
                        certificate(discharged.verdicts.get(obligation).unwrap()),
                    );
                    comparisons += 1;
                }
                _ => {}
            }
        }
    }
    assert!(
        relations > 0 && comparisons >= 2,
        "the joint cycle must carry its finite relation and both members' comparisons"
    );
}

/// The lowered loop with one field of its ranking changed.
fn tampered(change: impl FnOnce(&mut terminal_psi::TerminalNaturalCycle)) -> Refusal {
    let mut elaboration = lower_ranked(less_one(), witnessed(0));
    let Some(TerminalRankedScc::Natural(cycles)) = &mut elaboration.module.machines[0].ranked_scc
    else {
        panic!("the loop is ranked");
    };
    change(&mut cycles[0]);
    discharge(&mut elaboration).unwrap_err()
}

#[test]
fn a_ranking_that_is_not_the_graphs_own_refuses_the_machine() {
    // A jump back that only claims to keep the rank leaves the loop with no
    // strict descent on its one cycle, whatever the comparisons prove.
    let preserved = tampered(|cycle| {
        for edge in &mut cycle.edges {
            edge.comparison = TerminalNaturalRankComparison::Preserving;
        }
    });
    assert_eq!(preserved.reason, RefusalReason::ForeignIdentity);
    assert!(
        preserved.detail == "removing strict rank edges leaves a control cycle",
        "{preserved:?}"
    );
    // An edge claim that is missing.
    let missing = tampered(|cycle| {
        cycle.edges.pop();
    });
    assert_eq!(missing.reason, RefusalReason::ForeignIdentity);
    // A successor rank that is not what the jump back passes: the head's
    // own parameter in place of the call's argument.
    let substituted = tampered(|cycle| {
        let strict = |edge: &terminal_psi::TerminalNaturalRankEdge| {
            edge.comparison == TerminalNaturalRankComparison::Strict
        };
        let kept = cycle.edges.iter().find(|edge| !strict(edge)).unwrap();
        let kept = kept.successor_ranks;
        let back = cycle.edges.iter_mut().find(|edge| strict(edge)).unwrap();
        back.successor_ranks = kept;
    });
    assert_eq!(substituted.reason, RefusalReason::ForeignIdentity);
    assert!(
        substituted.detail.contains("successor rank"),
        "{substituted:?}"
    );
}

#[test]
fn a_terminating_machine_with_an_unranked_cycle_is_refused() {
    // The loop lowered with no clause is a productive cycle that owes no
    // rank. The same graph under a contract that publishes `Terminates`
    // has nothing that shows it ends.
    assert!(discharge(&mut lower_ranked(less_one(), None)).is_ok());
    let mut elaboration = lower_ranked(less_one(), None);
    elaboration.module.machines[0].contract.termination =
        semantic_vocabulary::Termination::Terminates;
    let refusal = discharge(&mut elaboration).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(
        refusal.detail,
        "contract termination promise over an unranked cycle"
    );
}
