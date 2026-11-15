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
    match super::entryless::judged(text, false) {
        RouteVerdict::Accepted(accepted) => accepted,
        verdict => panic!("the program must be accepted: {verdict:?}\n{text}"),
    }
}

fn unproved(text: &str) -> Vec<UnprovedObligation> {
    match super::entryless::judged(text, false) {
        RouteVerdict::Rejected { unproved, .. } => unproved,
        verdict => panic!("the program must be rejected: {verdict:?}\n{text}"),
    }
}

fn refused(text: &str) -> String {
    match super::entryless::judged(text, false) {
        RouteVerdict::FallBack(refusal) => {
            assert_eq!(refusal.reason, RefusalReason::Unsupported);
            refusal.detail
        }
        verdict => panic!("the program must be refused: {verdict:?}\n{text}"),
    }
}

#[test]
fn a_measure_that_decreases_on_the_recursive_call_is_accepted() {
    // The fixture's loop (`tests/fixtures/pass/build/
    // activation_yields_without_replay`, `count_down`): under the arm's
    // guard `remaining > 0` the subtraction is safe and its result is
    // strictly below `remaining`.
    let accepted = accepted(&count_down("remaining", "remaining - 1"));
    let lowered = &accepted.elaboration.module.machines[0];
    assert_eq!(
        lowered.contract.termination,
        semantic_vocabulary::Termination::Terminates
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
fn every_rank_obligation_of_an_accepted_loop_has_a_kernel_checked_certificate() {
    // machine countdown(remaining: u32) -> u32 terminates by remaining
    //   { remaining > 0 -> countdown(remaining - 1); 0 }
    let mut elaboration = lower_ranked(less_one(), witnessed(0));
    let result = discharge(&mut elaboration).unwrap();
    let rows = verdicts(&elaboration, &result, 1, ranked);
    // One shared well-foundedness question and one comparison on each of
    // the cycle's two edges: the edge beneath the head and the jump back.
    assert_eq!(rows.len(), 3);
    for (obligation, verdict) in rows {
        assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
    }
    assert!(matches!(
        judge(Ok(lower_ranked(less_one(), witnessed(0)))).unwrap(),
        RouteVerdict::Accepted(_)
    ));
}

fn witnessed(subject: u32) -> Termination {
    Termination::Witnessed(typed_trees_to_terminal_psi::Ranking { subject })
}

fn less_one() -> Expression {
    strict(Strict::Subtract, slot(0, u32_type()), unsigned(1))
}

/// `machine countdown(remaining: u32, spare: u32) -> u32 <termination>
///  { remaining > 0 -> countdown(<next>, spare); 0 }`
fn lower_ranked(
    next: Expression,
    termination: Termination,
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
        termination,
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

#[test]
fn a_recursive_call_that_keeps_the_measure_is_rejected() {
    // The twin differs in the recursive argument alone: `remaining` is
    // passed on unchanged, so `remaining < remaining` is owed and has no
    // derivation.
    assert_no_descent(&unproved(&count_down("remaining", "remaining")));
}

#[test]
fn a_measure_the_recursive_call_does_not_decrease_is_rejected() {
    // The twin differs in the measure alone: `spare` is passed on
    // unchanged by the same call that decreases `remaining`.
    accepted(&count_down("remaining", "remaining - 1"));
    assert_no_descent(&unproved(&count_down("spare", "remaining - 1")));
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
            "contract rank range on a ranking witness",
        ),
        // Two subjects select the bounded-distance view.
        (
            "(remaining, limit)",
            "contract ranking view other than Nat::Descending",
        ),
    ] {
        assert_eq!(refused(&looping(scalars, measure)), detail, "{measure}");
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
        "contract ranking subject that is not an unsigned fixed-width integer formal"
    );
    // Two machines that call each other: one joint ranking, whose spelling
    // for differently shaped members the specification leaves open.
    assert_eq!(
        refused(
            "machine even(n: u64) -> u64 terminates by n {\n    \
                 transition n > 0 { true -> odd(n - 1) false -> 0 }\n\
             }\n\
             machine odd(n: u64) -> u64 terminates by n {\n    \
                 transition n > 0 { true -> even(n - 1) false -> 1 }\n\
             }\n"
        ),
        "contract ranking witness on a call cycle of several machines"
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
        preserved.detail.contains("no strict descent"),
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
    assert!(discharge(&mut lower_ranked(less_one(), Termination::Silent)).is_ok());
    let mut elaboration = lower_ranked(less_one(), Termination::Silent);
    elaboration.module.machines[0].contract.termination =
        semantic_vocabulary::Termination::Terminates;
    let refusal = discharge(&mut elaboration).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(
        refusal.detail,
        "contract termination promise over an unranked cycle"
    );
}
