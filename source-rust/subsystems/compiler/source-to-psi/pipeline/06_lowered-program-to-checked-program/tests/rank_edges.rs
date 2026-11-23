//! Stage 05 must state, on each control edge of a ranked cycle, the rank
//! the edge carries to its target and whether that rank must fall (Strict)
//! or may stay (Preserving), and the lowered-form checker must prove the
//! claims from those values. A recursive call in terminal position is
//! lowered as a jump back to the machine's head, so the rank of the next
//! iteration is the value the jump passes.
//!
//! The spec rule: "Edges may preserve rank only if every complete cycle
//! contains strict descent", and each recursive transfer "owes the
//! comparison required by the selected view"
//! (`wiki/spec/language/termination.md`, "Ranking").
//!
//! The tests lower a countdown, assert which value each edge carries and
//! which edge claims the descent, and then change only that claim or that
//! value in the lowered program.

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, Verdict};
use semantic_vocabulary::{EdgeId, ValueId};
use terminal_check_tables::RefusalReason;
use terminal_psi::{
    OperationKind, TerminalMachine, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalRankedScc, Terminator,
};

fn countdown(next: &str) -> String {
    format!(
        "machine leaf(x: u64) -> u64 {{ x }} \
         machine count(n: u64) -> u64 terminates by n {{ \
             transition n == 0 {{ true -> leaf(n), false -> count({next}), }} \
         }}"
    )
}

fn cycle(machine: &TerminalMachine) -> &TerminalNaturalCycle {
    let Some(TerminalRankedScc::Natural(cycles)) = &machine.ranked_scc else {
        panic!("the machine has a ranked cycle: {:?}", machine.ranked_scc)
    };
    let [cycle] = cycles.as_slice() else {
        panic!("one cycle: {cycles:?}")
    };
    cycle
}

fn cycle_mut(machine: &mut TerminalMachine) -> &mut TerminalNaturalCycle {
    let Some(TerminalRankedScc::Natural(cycles)) = &mut machine.ranked_scc else {
        unreachable!()
    };
    &mut cycles[0]
}

/// Each cycle edge, with the one rank value it carries and its comparison.
fn claims(cycle: &TerminalNaturalCycle) -> Vec<(EdgeId, ValueId, TerminalNaturalRankComparison)> {
    cycle
        .edges
        .iter()
        .map(|edge| {
            let value = cycle
                .scalar_rank_value(edge.successor_ranks)
                .expect("one scalar rank per edge");
            (edge.edge, value.value, edge.comparison)
        })
        .collect()
}

/// The edge that jumps back to the head, and the value it passes.
fn back_jump(machine: &TerminalMachine) -> (EdgeId, ValueId) {
    let jumps: Vec<_> = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Jump {
                edge, arguments, ..
            } if block.id != machine.entry => {
                let [argument] = arguments.as_slice() else {
                    panic!("the jump passes one argument: {arguments:?}")
                };
                Some((*edge, *argument))
            }
            _ => None,
        })
        .collect();
    let [jump] = jumps.as_slice() else {
        panic!("one jump back: {jumps:?}")
    };
    *jump
}

/// In `count(n - 1)` the jump back to the head passes the value of the
/// subtraction, and that edge alone claims a strict decrease. The guard
/// edge into the decrementing arm keeps the head's rank. The exit through
/// `leaf` is not a cycle edge. The checker accepts the program.
#[test]
fn the_recursive_jump_claims_descent_with_the_value_it_passes() {
    let program = Program::of(&countdown("n - 1"));
    let count = program.machine("count");
    let (back, passed) = back_jump(count);
    let subtraction = count
        .operations()
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. }))
        .expect("`n - 1` is one exact subtraction");
    assert_eq!(
        subtraction.result.scalar().map(|value| value.id),
        Some(passed)
    );
    let head = count
        .blocks
        .iter()
        .find(|block| {
            matches!(&count.blocks[0].terminator, Terminator::Jump { target, .. } if *target == block.id)
        })
        .expect("the entry jumps to the head");
    let [rank] = head.parameters.as_slice() else {
        panic!("the head takes `n`")
    };
    let claims = claims(cycle(count));
    assert_eq!(claims.len(), 2, "{claims:?}");
    assert!(
        claims.contains(&(back, passed, TerminalNaturalRankComparison::Strict)),
        "{claims:?}"
    );
    assert!(
        claims.iter().any(|&(edge, value, comparison)| edge != back
            && value == rank.id
            && comparison == TerminalNaturalRankComparison::Preserving),
        "{claims:?}"
    );
    program.judged(|_| {}).accepted();
}

const NO_STRICT_EDGE: &str = "removing strict rank edges leaves a control cycle";

fn refusal(verdict: Verdict) -> String {
    match verdict {
        Verdict::Refused(refusal) => {
            assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
            refusal.detail
        }
        other => panic!("the program must be refused: {other:?}"),
    }
}

/// The twin `count(n)` keeps the measure, so no edge may claim a decrease
/// and the cycle has none: the checker refuses it. Turning only the jump's
/// Strict claim into Preserving in the lowered countdown gives the same
/// refusal. Keeping Strict but claiming the jump carries the head's
/// unchanged `n` is refused, because the jump passes `n - 1`.
#[test]
fn the_checker_follows_the_claim_and_the_value_on_the_jump_back() {
    let unchanged = Program::of(&countdown("n"));
    assert!(
        refusal(unchanged.judged(|_| {})).contains(NO_STRICT_EDGE),
        "count(n)"
    );

    let program = Program::of(&countdown("n - 1"));
    let count = program.machine("count");
    let (back, _) = back_jump(count);
    let position = cycle(count)
        .edges
        .iter()
        .position(|edge| edge.edge == back)
        .unwrap();
    let detail = refusal(program.judged_in("count", |machine| {
        cycle_mut(machine).edges[position].comparison = TerminalNaturalRankComparison::Preserving;
    }));
    assert!(detail.contains(NO_STRICT_EDGE), "{detail}");

    let rank = cycle(count)
        .ranks
        .iter()
        .find(|rank| rank.block != count.blocks[back_source(count, back)].id)
        .map(|rank| rank.values)
        .unwrap();
    let detail = refusal(program.judged_in("count", |machine| {
        cycle_mut(machine).edges[position].successor_ranks = rank;
    }));
    assert!(
        detail.contains(
            "successor rank is not the target observation under its exact actual arguments"
        ),
        "{detail}"
    );
}

/// The position of the block whose terminator is the edge `edge`.
fn back_source(machine: &TerminalMachine, edge: EdgeId) -> usize {
    machine
        .blocks
        .iter()
        .position(|block| matches!(&block.terminator, Terminator::Jump { edge: found, .. } if *found == edge))
        .unwrap()
}
