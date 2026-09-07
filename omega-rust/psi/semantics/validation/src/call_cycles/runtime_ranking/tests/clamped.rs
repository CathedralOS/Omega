use super::ranges::progress;
use super::*;

const CLAMPED: &str = "data Main {}
machine Main::first(&mut self, limit: u64, cursor: u64, capacity: u64)
requires limit <= capacity;
terminates by cursor -> Nat::IncreasingTo(limit) in 0..=capacity;
-> u64 { transition { _ -> self.second(capacity, cursor, limit) } }
machine Main::second(&mut self, ceiling: u64, position: u64, bound: u64)
requires bound <= ceiling;
terminates by position -> Nat::IncreasingTo(bound) in 0..=ceiling;
-> u64 { transition position < bound { true -> self.first(bound, position + 1, ceiling) false -> position } }
";

fn entry(program: &TypedTrees) -> bool {
    let machine = &program.machines()[0];
    let rank = RankProjection::resolve(program, machine).unwrap();
    prove_ranking_range_call_entry(
        program,
        RankingRangeCallMember {
            machine,
            subject: rank.subject,
            range: rank.range,
        },
    )
}

#[test]
fn call_entry_clamps_below_at_and_above_the_view_bound() {
    for cursor in [2, 6, 9] {
        let source = CLAMPED.replace(
            "requires limit <= capacity;",
            &format!("requires limit == 6 && capacity == 8 && cursor == {cursor};"),
        );
        assert!(entry(&typed_source(&source)), "{source}");
    }
    let program = typed_source(CLAMPED);
    assert!(entry(&program));
    assert_eq!(
        progress(&program, 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(admitted(&program).len(), 1);
}

#[test]
fn clamped_entry_honors_positive_floors_and_exclusive_ceilings() {
    let positive = CLAMPED.replace("0..=capacity", "1..=capacity").replace(
        "requires limit <= capacity;",
        "requires cursor < limit && limit <= capacity;",
    );
    assert!(entry(&typed_source(&positive)));
    assert!(!entry(&typed_source(
        &CLAMPED.replace("0..=capacity", "1..=capacity")
    )));
    let exclusive = CLAMPED
        .replace("0..=capacity", "0..capacity")
        .replace("requires limit <= capacity;", "requires limit < capacity;");
    assert!(entry(&typed_source(&exclusive)));
    assert!(!entry(&typed_source(
        &CLAMPED.replace("0..=capacity", "0..capacity")
    )));
    let empty = CLAMPED.replace("0..=capacity", "0..0").replace(
        "requires limit <= capacity;",
        "requires limit <= capacity && cursor >= limit;",
    );
    assert!(!entry(&typed_source(&empty)));
}

#[test]
fn raw_decrease_after_the_bound_is_not_strict_progress() {
    let source = CLAMPED.replace(
        "transition position < bound { true -> self.first(bound, position + 1, ceiling) false -> position }",
        "transition { _ -> self.first(bound, position + 1, ceiling) }");
    let program = typed_source(&source);
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert!(admitted(&program).is_empty());
}

#[test]
fn a_zero_destination_can_preserve_rank_despite_increasing_raw_distance() {
    let source = CLAMPED
        .replace(
            "requires limit <= capacity;",
            "requires limit <= capacity && limit < cursor;",
        )
        .replace(
            "self.second(capacity, cursor, limit)",
            "self.second(capacity, cursor - 1, limit)",
        );
    assert_eq!(
        progress(&typed_source(&source), 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
}

#[test]
fn zero_rank_cannot_restart_positive_or_move_pinned_inputs() {
    for source in [
        CLAMPED
            .replace(
                "requires limit <= capacity;",
                "requires limit <= capacity && cursor >= limit;",
            )
            .replace(
                "self.second(capacity, cursor, limit)",
                "self.second(capacity, 0, limit)",
            ),
        CLAMPED.replace(
            "self.second(capacity, cursor, limit)",
            "self.second(capacity, cursor, limit + 1)",
        ),
        CLAMPED.replace(
            "self.second(capacity, cursor, limit)",
            "self.second(capacity + 1, cursor, limit)",
        ),
    ] {
        assert!(progress(&typed_source(&source), 0).is_none(), "{source}");
    }
}

#[test]
fn clamped_membership_does_not_relax_the_local_state_raw_invariant() {
    let program = typed_source(CLAMPED);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let rank = RankProjection::resolve(&program, machine).unwrap();
    let limit = program
        .ranking_expression_custody_for(machine.symbol)
        .unwrap()
        .view_arguments[0];
    assert!(entry(&program));
    assert!(!crate::prove_ranking_range_entry(
        &program,
        machine,
        state,
        rank.range,
        crate::RankingRangeMeasure::IncreasingTo {
            subject: rank.subject,
            limit
        }
    ));
}
