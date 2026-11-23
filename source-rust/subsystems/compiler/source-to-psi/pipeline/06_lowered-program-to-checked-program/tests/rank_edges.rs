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

fn mutual(first: &str, second: &str) -> String {
    format!(
        "machine first(n: u64) -> u64 terminates by n {{ transition {first} }} \
         machine second(n: u64) -> u64 terminates by n {{ transition {second} }}"
    )
}

/// The target block and comparison of each cycle edge that carries a
/// jump's argument, by whether that argument is a subtraction result.
fn handoffs(
    machine: &TerminalMachine,
) -> Vec<(
    semantic_vocabulary::BlockId,
    bool,
    TerminalNaturalRankComparison,
)> {
    let subtractions: Vec<_> = machine
        .operations()
        .filter(|operation| matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. }))
        .filter_map(|operation| operation.result.scalar().map(|value| value.id))
        .collect();
    let cycle = cycle(machine);
    let mut rows: Vec<_> = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
                ..
            } if block.id != machine.entry => {
                let claim = cycle.edges.iter().find(|claim| claim.edge == *edge)?;
                Some((
                    *target,
                    subtractions.contains(&arguments[0]),
                    claim.comparison,
                ))
            }
            _ => None,
        })
        .collect();
    rows.sort_by_key(|row| (row.0, row.1));
    rows
}

/// The head block of the member a machine's entry jumps to.
fn own_head(machine: &TerminalMachine) -> semantic_vocabulary::BlockId {
    match &machine.blocks[0].terminator {
        Terminator::Jump { target, .. } => *target,
        other => panic!("the entry jumps to its head: {other:?}"),
    }
}

/// In a mutual cycle each member's machine holds the whole component, one
/// head block per member. The call that passes `n - 1` is the one Strict
/// edge, into the head of the member it calls; the call that passes `n`
/// unchanged is Preserving. Moving the decrement from `second` to `first`
/// moves the Strict claim to the other head.
#[test]
fn in_a_mutual_cycle_only_the_decrementing_call_claims_descent() {
    use TerminalNaturalRankComparison::{Preserving, Strict};
    let handoff = "{ _ -> second(n), }";
    let decrement_into_first = "n == 0 { true -> 9, false -> first(n - 1), }";
    let decrement_into_second = "n == 0 { true -> 9, false -> second(n - 1), }";
    let back = "{ _ -> first(n), }";
    for (program, strict_into) in [
        (Program::of(&mutual(handoff, decrement_into_first)), "first"),
        (Program::of(&mutual(decrement_into_second, back)), "second"),
    ] {
        let heads = [
            ("first", own_head(program.machine("first"))),
            ("second", own_head(program.machine("second"))),
        ];
        let head = |name: &str| heads.iter().find(|(member, _)| *member == name).unwrap().1;
        let preserved_into = if strict_into == "first" {
            "second"
        } else {
            "first"
        };
        for member in ["first", "second"] {
            let machine = program.machine(member);
            assert_eq!(
                handoffs(machine),
                {
                    let mut expected = vec![
                        (head(strict_into), true, Strict),
                        (head(preserved_into), false, Preserving),
                    ];
                    expected.sort_by_key(|row| (row.0, row.1));
                    expected
                },
                "{member}"
            );
        }
        program.judged(|_| {}).accepted();

        // Turning every claim in `first`'s copy of the component around
        // makes the unchanged handoff claim descent, which the checker
        // cannot prove, so it rejects the cycle's comparisons.
        let verdict = program.judged_in("first", |machine| {
            let Some(TerminalRankedScc::Natural(cycles)) = &mut machine.ranked_scc else {
                unreachable!()
            };
            for claim in &mut cycles[0].edges {
                claim.comparison = match claim.comparison {
                    Strict => Preserving,
                    Preserving => Strict,
                };
            }
        });
        let rejection = verdict.rejected();
        assert!(
            rejection.unproved.len() == 2
                && rejection
                    .unproved
                    .iter()
                    .all(|kind| kind.starts_with("RankComparison")),
            "{rejection:?}"
        );
    }
}

/// The value the range's ceiling is bound to: the third argument the entry
/// passes to the block that establishes the range.
fn ceiling_value(machine: &TerminalMachine) -> ValueId {
    let Terminator::Jump { target, .. } = &machine.blocks[0].terminator else {
        panic!("the entry jumps to the range's block")
    };
    machine
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .map(|block| block.parameters[2].id)
        .expect("the range's block binds floor, measure and ceiling")
}

/// The conjunct of the range invariant that compares the measure with the
/// ceiling.
fn ceiling_part<'p>(
    machine: &TerminalMachine,
    predicate: &'p semantic_vocabulary::Proposition,
) -> &'p semantic_vocabulary::Proposition {
    use semantic_vocabulary::{Proposition, ScalarTerm};
    let ceiling = ceiling_value(machine);
    let Proposition::Conjunction(parts) = predicate else {
        panic!("floor and ceiling: {predicate:?}")
    };
    let found: Vec<_> = parts
        .iter()
        .filter(|part| {
            matches!(part,
                Proposition::LessOrEqual(_, ScalarTerm::Value { id, .. })
                | Proposition::LessThan(_, ScalarTerm::Value { id, .. }) if *id == ceiling)
        })
        .collect();
    let [part] = found.as_slice() else {
        panic!("one ceiling comparison: {parts:?}")
    };
    part
}

fn visit(requires: &str, range: &str) -> String {
    format!("machine visit(n: u64) requires {requires} terminates by n in {range} {{}}")
}

/// The rank range's invariant as stage 05 wrote it: whether the ceiling is
/// inclusive, and the constant the ceiling is bound to.
fn range_invariant(program: &Program) -> (bool, u64) {
    use semantic_vocabulary::{IntegerValue, Proposition};
    let visit = program.machine("visit");
    let invariants: Vec<_> = program
        .elaboration()
        .module
        .proofs
        .scalar_block_invariants
        .iter()
        .filter(|invariant| invariant.machine == visit.id)
        .collect();
    let [invariant] = invariants.as_slice() else {
        panic!("one rank range invariant: {invariants:?}")
    };
    let inclusive = match ceiling_part(visit, &invariant.predicate) {
        Proposition::LessOrEqual(..) => true,
        Proposition::LessThan(..) => false,
        other => panic!("a ceiling comparison: {other:?}"),
    };
    let bound = visit
        .operations()
        .filter_map(|operation| match operation.kind {
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(value),
            } if value != 0 => Some(value as u64),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [bound] = bound.as_slice() else {
        panic!("one nonzero bound: {bound:?}")
    };
    (inclusive, *bound)
}

/// `in 0..=9` writes an inclusive ceiling bound to 9 and `in 0..10` an
/// exclusive one bound to 10; both hold under `requires n <= 9`. Under
/// `requires n <= 10`, `0..=9` is rejected at the rank's arrival. Making
/// the lowered `0..=9` ceiling exclusive makes `requires n <= 9` too weak,
/// and the checker rejects it the same way.
#[test]
fn a_rank_range_keeps_its_own_ceiling_and_inclusivity() {
    let inclusive = Program::of(&visit("n <= 9", "0..=9"));
    assert_eq!(range_invariant(&inclusive), (true, 9));
    inclusive.judged(|_| {}).accepted();
    let exclusive = Program::of(&visit("n <= 9", "0..10"));
    assert_eq!(range_invariant(&exclusive), (false, 10));
    exclusive.judged(|_| {}).accepted();

    let arrival = |rejection: program::Rejection| {
        assert!(
            rejection
                .unproved
                .iter()
                .any(|kind| kind.starts_with("ArrivalInvariant")),
            "{rejection:?}"
        );
    };
    arrival(
        Program::of(&visit("n <= 10", "0..=9"))
            .judged(|_| {})
            .rejected(),
    );

    let machine = inclusive.machine("visit");
    let visit_id = machine.id;
    let target = ceiling_part(machine, &range_predicate(&inclusive, visit_id)).clone();
    arrival(
        inclusive
            .judged(|lowered| {
                use semantic_vocabulary::Proposition;
                for invariant in &mut lowered.module.proofs.scalar_block_invariants {
                    if invariant.machine != visit_id {
                        continue;
                    }
                    let Proposition::Conjunction(parts) = &mut invariant.predicate else {
                        unreachable!()
                    };
                    for part in parts.iter_mut() {
                        if *part == target {
                            let Proposition::LessOrEqual(measure, ceiling) = part.clone() else {
                                unreachable!()
                            };
                            *part = Proposition::LessThan(measure, ceiling);
                        }
                    }
                }
            })
            .rejected(),
    );
}

fn range_predicate(
    program: &Program,
    machine: semantic_vocabulary::MachineId,
) -> semantic_vocabulary::Proposition {
    program
        .elaboration()
        .module
        .proofs
        .scalar_block_invariants
        .iter()
        .find(|invariant| invariant.machine == machine)
        .expect("the range invariant")
        .predicate
        .clone()
}
