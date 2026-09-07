use super::*;

const RANGED: &str = "data Main {}
machine Main::first(&mut self, lower: u64, remaining: u64, upper: u64)
requires lower <= remaining && remaining <= upper;
terminates by remaining in lower..=upper;
-> u64 {
    transition remaining > lower { true -> self.second(upper, remaining, lower) false -> remaining }
}
machine Main::second(&mut self, ceiling: u64, pending: u64, floor: u64)
requires floor <= pending && pending <= ceiling;
terminates by pending in floor..=ceiling;
-> u64 {
    transition pending > floor { true -> self.first(floor, pending - 1, ceiling) false -> pending }
}";

fn progress(program: &TypedTrees, source_position: usize) -> Option<RankingRangeCallProgress> {
    let caller = &program.machines()[source_position];
    let callee = &program.machines()[1 - source_position];
    let source = &program.machine_states(caller)[0];
    let source_rank = RankProjection::resolve(program, caller)?;
    let destination_rank = RankProjection::resolve(program, callee)?;
    for statement in program.statement_table.statements(source.statement_nodes) {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = program.statement_table.transition_target(transition.target)
        else {
            continue;
        };
        if path.symbol != callee.symbol && path.symbol != program.machine_states(callee)[0].symbol {
            continue;
        }
        let guards = match transition.guard {
            TransitionGuardNode::Always => Vec::new(),
            TransitionGuardNode::When(guard) => vec![(guard, true)],
        };
        return prove_ranking_range_call(
            program,
            RankingRangeCallMember {
                machine: caller,
                subject: source_rank.subject,
                range: source_rank.range,
            },
            RankingRangeCallMember {
                machine: callee,
                subject: destination_rank.subject,
                range: destination_rank.range,
            },
            &guards,
            program.statement_table.expression_handles(*arguments),
        );
    }
    None
}

#[test]
fn ranged_call_uses_each_authored_telescope_and_classifies_weak_edges() {
    let program = typed_source(RANGED);
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
fn call_range_does_not_assume_destination_requirements_or_moved_endpoints() {
    for source in [
        RANGED.replace(
            "self.second(upper, remaining, lower)",
            "self.second(upper - 1, remaining, lower)",
        ),
        RANGED.replace(
            "self.second(upper, remaining, lower)",
            "self.second(upper, remaining + 1, lower)",
        ),
        RANGED.replace(
            "self.first(floor, pending - 1, ceiling)",
            "self.first(floor, pending - 2, ceiling)",
        ),
    ] {
        assert!(admitted(&typed_source(&source)).is_empty(), "{source}");
    }
}

#[test]
fn ranged_call_cycles_need_strict_progress_and_complete_range_evidence() {
    let preserving = RANGED.replace("pending - 1", "pending");
    assert!(admitted(&typed_source(&preserving)).is_empty());
    let mixed = RANGED.replace(" by pending in floor..=ceiling", " by pending");
    assert!(admitted(&typed_source(&mixed)).is_empty());
    let missing_entry = RANGED.replace("requires lower <= remaining && remaining <= upper;", "");
    assert!(admitted(&typed_source(&missing_entry)).is_empty());
}

#[test]
fn ranged_call_arithmetic_proves_a_variable_positive_step() {
    let program = typed_source(
        "data Main {}
        machine Main::first(&mut self, n: u64, step: u64, cap: u64)
        requires step > 0 && n <= cap;
        terminates by n in 0..=cap;
        -> u64 { transition n >= step { true -> self.second(n, step, cap) false -> n } }
        machine Main::second(&mut self, n: u64, step: u64, cap: u64)
        requires step > 0 && n <= cap;
        terminates by n in 0..=cap;
        -> u64 { transition n >= step { true -> self.first(n - step, step, cap) false -> n } }",
    );
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(admitted(&program).len(), 1);
}

#[test]
fn ranged_call_prefix_cannot_change_an_endpoint() {
    let source = RANGED.replace("lower: u64", "mut lower: u64").replace(
        "    transition remaining > lower",
        "    lower = 0; transition remaining > lower",
    );
    assert!(admitted(&typed_source(&source)).is_empty());
}

#[test]
fn invalid_authored_range_custody_cannot_fall_back_to_an_unranged_component() {
    let mut program = typed_source(RANGED);
    assert_eq!(admitted(&program).len(), 1);
    for custody in &mut program.ranking_expression_custody {
        custody.rank_range = Some(ExpressionHandle::invalid());
    }
    assert!(admitted(&program).is_empty());
}

#[test]
fn call_range_query_rejects_foreign_subject_endpoint_and_actual_handles() {
    let program = typed_source(RANGED);
    let caller = &program.machines()[0];
    let callee = &program.machines()[1];
    let caller_rank = RankProjection::resolve(&program, caller).unwrap();
    let callee_rank = RankProjection::resolve(&program, callee).unwrap();
    let caller_range = caller_rank.range;
    let callee_range = callee_rank.range;
    let (arguments, guards) = program
        .statement_table
        .statements(program.machine_states(caller)[0].statement_nodes)
        .iter()
        .find_map(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            let TransitionTargetNode::Named { arguments, .. } =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                return None;
            };
            Some((
                program
                    .statement_table
                    .expression_handles(*arguments)
                    .to_vec(),
                vec![(guard, true)],
            ))
        })
        .unwrap();
    let query = |subject, range, actuals: &[ExpressionHandle]| {
        prove_ranking_range_call(
            &program,
            RankingRangeCallMember {
                machine: caller,
                subject,
                range: caller_range,
            },
            RankingRangeCallMember {
                machine: callee,
                subject: callee_rank.subject,
                range,
            },
            &guards,
            actuals,
        )
    };
    assert_eq!(
        query(caller_rank.subject, callee_range, &arguments),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert!(query(callee_rank.subject, callee_range, &arguments).is_none());
    assert!(query(caller_rank.subject, caller_range, &arguments).is_none());
    let mut foreign = arguments;
    foreign[callee_rank.argument_position] = callee_rank.subject;
    assert!(query(caller_rank.subject, callee_range, &foreign).is_none());
}
