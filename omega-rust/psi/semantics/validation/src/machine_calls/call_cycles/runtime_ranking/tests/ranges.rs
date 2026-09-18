use super::super::{
    ExpressionHandle, RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
    TransitionGuardNode, TransitionTargetNode, prove_ranking_range_call,
};
use super::{RankProjection, StatementNode, TypedTrees, admitted, typed_source};
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

#[test]
fn mixed_endpoint_transport_proves_computed_equality_without_inventing_a_pin() {
    let source = RANGED.replace(" in floor..=ceiling", "");
    for endpoint in ["upper + 0", "upper + (lower - lower)"] {
        let program = typed_source(&source.replace(
            "upper, remaining, lower)",
            &format!("{endpoint}, remaining, lower)"),
        ));
        assert_eq!(admitted(&program).len(), 1, "{endpoint}");
    }
    let changed =
        typed_source(&source.replace("upper, remaining, lower)", "upper + 1, remaining, lower)"));
    assert!(admitted(&changed).is_empty());
    let selected = typed_source(&format!(
        "operator + u64::add(left: u64, right: u64) -> u64; {}",
        source.replace("upper, remaining, lower)", "upper + 0, remaining, lower)")
    ));
    assert!(admitted(&selected).is_empty());
}

pub(super) fn progress(
    program: &TypedTrees,
    source_position: usize,
) -> Option<RankingRangeCallProgress> {
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
                paired_subject: source_rank.paired_subject,
                range: source_rank.range,
            },
            &RankingRangeCallSite {
                state: source,
                entry_parameters: &[],
            },
            RankingRangeCallMember {
                machine: callee,
                subject: destination_rank.subject,
                paired_subject: destination_rank.paired_subject,
                range: destination_rank.range,
            },
            &guards,
            program.statement_table.expression_handles(*arguments),
        );
    }
    None
}

#[test]
fn internal_state_calls_keep_their_entry_roles() {
    let source = "data Main {}
        machine Main::count(&mut self, remaining: u32 [0..=9])
        terminates by remaining in 0..=9;
        -> u32 {
            transition remaining > 0 { true -> hold(remaining) false -> remaining }
            state hold(pending: u32 [0..=9]) {
                transition pending > 0 { true -> self.step(pending) false -> pending }
            }
        }
        machine Main::step(&mut self, n: u32 [0..=9])
        terminates by n in 0..=9;
        -> u32 {
            transition n > 0 { true -> self.count(n - 1) false -> n }
        }";
    // `hold -> step(pending)` is weak (pending carries the entry role
    // `remaining`); `step -> count(n - 1)` is strict — the component admits.
    assert_eq!(admitted(&typed_source(source)).len(), 1);
    // The same site still answers to the shared ranking: a transported value
    // that cannot prove non-increase rejects the component.
    let increased = source.replace("self.step(pending)", "self.step(pending + 1)");
    assert!(admitted(&typed_source(&increased)).is_empty());
}

#[test]
fn internal_state_calls_read_duplicated_rank_copies() {
    // `pair(left, right)` holds two copies of the same arrival value; the
    // discovery kept both claims only because every arrival forwarded a bare
    // name, so the copies are equal and either may transport the rank.
    let source = "data Main {}
        machine Main::count(&mut self, remaining: u32 [0..=9])
        terminates by remaining in 0..=9;
        -> u32 {
            transition remaining > 0 { true -> pair(remaining, remaining) false -> remaining }
            state pair(left: u32 [0..=9], right: u32 [0..=9]) {
                transition left > 0 { true -> self.step(right) false -> left }
            }
        }
        machine Main::step(&mut self, n: u32 [0..=9])
        terminates by n in 0..=9;
        -> u32 {
            transition n > 0 { true -> self.count(n - 1) false -> n }
        }";
    assert_eq!(admitted(&typed_source(source)).len(), 1);
    // A computed copy of the required carrier keeps its claim at this level:
    // the member's own ranged arrival judgment -- which the checked stage
    // runs for every multi-state ranged member -- is what proves the copy
    // equal, and the component consumes that evidence rather than re-running
    // it. `remaining + 0` is not a strict step, so no claimant is named the
    // moved copy and the equal-copy trust still applies.
    let computed = source.replace(
        "pair(remaining, remaining)",
        "pair(remaining, remaining + 0)",
    );
    assert_eq!(admitted(&typed_source(&computed)).len(), 1);
    // `remaining + 1` is a strict `carrier + positive` step, so discovery
    // names `right` the moved copy and demotes `left`. The component then
    // reads the increased carrier directly: an `AtLeast` bound cannot prove
    // the descending rank, so the rejection the member-level check used to
    // own now surfaces here too -- `pair(remaining, remaining + 1)` feeding
    // `step(right)` hands `remaining + 1` to a callee that subtracts one,
    // and the cycle preserves the rank instead of decreasing it.
    let moved = source.replace(
        "pair(remaining, remaining)",
        "pair(remaining, remaining + 1)",
    );
    assert!(admitted(&typed_source(&moved)).is_empty());
    // A computed copy of an entry the range proof never names keeps no
    // premise claim: `right` spells `spare + 0`, so the site has no route
    // from the rank to the transported value.
    let stray = source
        .replace(
            "remaining: u32 [0..=9])",
            "remaining: u32 [0..=9], spare: u32)",
        )
        .replace("pair(remaining, remaining)", "pair(remaining, spare + 0)")
        .replace("self.count(n - 1)", "self.count(n - 1, 0)");
    assert_ne!(stray, source);
    assert!(admitted(&typed_source(&stray)).is_empty());
}

#[test]
fn mixed_component_conserves_endpoints_through_internal_sites() {
    // `hold(pending, bound)` carries the ranked input and the authored
    // ceiling. The site consumes `outer`'s own range invariant, `0 <= pending
    // <= bound`, as the arrival's proven evidence -- the member's state-edge
    // judgment, which the checked stage runs for every ranged member with
    // internal arrivals, re-establishes it there -- so the guard need not
    // respell membership; the unranged member must still transport that
    // exact endpoint back.
    let source = "data Main {}
        machine Main::outer(&mut self, cap: u64, remaining: u64)
        requires remaining <= cap;
        terminates by remaining in 0..=cap;
        -> u64 {
            transition remaining > 0 { true -> hold(remaining, cap) false -> remaining }
            state hold(pending: u64, bound: u64) {
                transition pending > 0 {
                    true -> self.inner(pending, bound)
                    false -> pending
                }
            }
        }
        machine Main::inner(&mut self, n: u64, limit: u64)
        terminates by n;
        -> u64 {
            transition n > 0 && n <= limit { true -> self.outer(limit, n - 1) false -> n }
        }";
    assert_eq!(admitted(&typed_source(source)).len(), 1);
    let respelled = source.replace("pending > 0 {", "pending > 0 && pending <= bound {");
    assert_eq!(admitted(&typed_source(&respelled)).len(), 1);
    // An actual that is not the carried endpoint cannot pin the authored
    // ceiling, even when it is spelled from the same carrier.
    let moved = source.replace(
        "self.inner(pending, bound)",
        "self.inner(pending, bound + 1)",
    );
    assert!(admitted(&typed_source(&moved)).is_empty());
    let renamed = source.replace("self.inner(pending, bound)", "self.inner(pending, pending)");
    assert!(admitted(&typed_source(&renamed)).is_empty());
    // A prefix store into a carrier the invariant reads invalidates it.
    let written = source
        .replace("pending: u64, bound: u64)", "pending: u64, mut bound: u64)")
        .replace(
            "                transition pending > 0 {",
            "                bound = bound; transition pending > 0 {",
        );
    assert_ne!(written, source);
    assert!(admitted(&typed_source(&written)).is_empty());
    // Requires facts stay entry-site evidence: the entry obligation, not the
    // subordinate site, is what a missing premise fails.
    let missing_entry = source.replace("requires remaining <= cap;", "");
    assert!(admitted(&typed_source(&missing_entry)).is_empty());
}

#[test]
fn mixed_component_conserves_projected_endpoints() {
    // `limits.cap` is a projected endpoint input: the unranged member
    // transports the record, and every edge must prove the actual installs
    // the leaf coordinate the endpoint reads -- a record that merely shares
    // the field spelling is not the input.
    let source = "data Main {}
        data Limits { cap: u64 [0..=9]; }
        machine Main::outer(&mut self, remaining: u64, limits: Limits)
        requires remaining <= limits.cap;
        terminates by remaining in 0..=limits.cap;
        -> u64 {
            transition remaining > 0 { true -> self.inner(remaining, limits) false -> remaining }
        }
        machine Main::inner(&mut self, n: u64, bounds: Limits)
        terminates by n;
        -> u64 {
            transition n > 0 && n <= bounds.cap { true -> self.outer(n - 1, bounds) false -> n }
        }";
    assert_eq!(admitted(&typed_source(source)).len(), 1);
    // A literal rebuild conserves the endpoint only because its installed
    // leaf is the transported coordinate itself.
    let rebuilt = source.replace(
        "self.outer(n - 1, bounds)",
        "self.outer(n - 1, Limits { cap: bounds.cap })",
    );
    assert_eq!(admitted(&typed_source(&rebuilt)).len(), 1);
    // A different leaf is a different endpoint value.
    let changed = source.replace(
        "self.outer(n - 1, bounds)",
        "self.outer(n - 1, Limits { cap: bounds.cap + 1 })",
    );
    assert!(admitted(&typed_source(&changed)).is_empty());
    // A record the component never traced to the authored input does not
    // carry the endpoint, even with the same declaration.
    let spare = source
        .replace(
            "n: u64, bounds: Limits)",
            "n: u64, bounds: Limits, spare: Limits)",
        )
        .replace(
            "self.inner(remaining, limits)",
            "self.inner(remaining, limits, Limits { cap: 0 })",
        )
        .replace("self.outer(n - 1, bounds)", "self.outer(n - 1, spare)");
    assert_ne!(spare, source);
    assert!(admitted(&typed_source(&spare)).is_empty());
    // A subordinate arrival still names the same transported input: `bound`
    // carries `limits`' role, so the endpoint reads `bound.cap` at the site.
    let held = source.replace(
        "transition remaining > 0 { true -> self.inner(remaining, limits) false -> remaining }",
        "transition remaining > 0 { true -> hold(remaining, limits) false -> remaining }
        state hold(pending: u64, bound: Limits) {
            transition pending > 0 { true -> self.inner(pending, bound) false -> pending }
        }",
    );
    assert_eq!(admitted(&typed_source(&held)).len(), 1);
    // The endpoint role can also ride inside a larger record: `wrap.limits`
    // is the member actual of the carrier's unique nested path.
    let nested = "data Main {}
        data Limits { cap: u64 [0..=9]; }
        data Wrap { limits: Limits; }
        machine Main::outer(&mut self, remaining: u64, limits: Limits)
        requires remaining <= limits.cap;
        terminates by remaining in 0..=limits.cap;
        -> u64 {
            transition remaining > 0 { true -> self.inner(remaining, Wrap { limits: limits }) false -> remaining }
        }
        machine Main::inner(&mut self, n: u64, wrap: Wrap)
        terminates by n;
        -> u64 {
            transition n > 0 && n <= wrap.limits.cap { true -> self.outer(n - 1, wrap.limits) false -> n }
        }";
    assert_eq!(admitted(&typed_source(nested)).len(), 1);
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
fn unranged_natural_call_still_requires_a_nonnegative_destination_rank() {
    let source = RANGED
        .replace(" in lower..=upper", "")
        .replace(" in floor..=ceiling", "");
    let program = typed_source(&source);
    assert_eq!(
        progress(&program, 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(admitted(&program).len(), 1);
    let underflow = typed_source(&source.replace("pending - 1", "pending - 2"));
    assert!(progress(&underflow, 1).is_none());
    assert!(admitted(&underflow).is_empty());
    let mut foreign = program;
    foreign.ranking_expression_custody[0].subjects[0] =
        foreign.ranking_expression_custody[1].subjects[0];
    assert!(progress(&foreign, 0).is_none());
    assert!(admitted(&foreign).is_empty());
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
    assert_eq!(admitted(&typed_source(&mixed)).len(), 1);
    let missing_entry = RANGED.replace("requires lower <= remaining && remaining <= upper;", "");
    assert!(admitted(&typed_source(&missing_entry)).is_empty());
}

#[test]
fn mixed_calls_prove_each_authored_membership_without_assuming_the_missing_range() {
    for omitted in [" in lower..=upper", " in floor..=ceiling"] {
        let source = RANGED.replace(omitted, "");
        let program = typed_source(&source);
        assert_eq!(
            progress(&program, 0),
            Some(RankingRangeCallProgress::NonIncreasing)
        );
        assert_eq!(
            progress(&program, 1),
            Some(RankingRangeCallProgress::Strict)
        );
        assert_eq!(admitted(&program).len(), 1);
        assert!(admitted(&typed_source(&source.replace("pending - 1", "pending"))).is_empty());
        assert!(admitted(&typed_source(&source.replace("pending - 1", "pending - 2"))).is_empty());
        // Membership can still hold after widening this ceiling. Only the
        // component-wide endpoint conservation exposes its changed identity.
        let changed = source.replace(
            "floor, pending - 1, ceiling)",
            "floor, pending - 1, ceiling + 1)",
        );
        assert!(admitted(&typed_source(&changed)).is_empty());
    }
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

const PREFIX_CALL: &str = "data Main {}
    machine Main::audit(&mut self, value: u64) -> u64 { value }
    machine Main::scan_a(&mut self, index: u64 [0..=4], limit: u64 [0..=4])
    requires index <= limit;
    terminates by (index, limit) -> Nat::BoundedDistance in 0..=4;
    -> u64 {
        self.audit(index);
        transition index < limit {
            true -> self.scan_b(index + 1, limit)
            false -> index
        }
    }
    machine Main::scan_b(&mut self, index: u64 [0..=4], limit: u64 [0..=4])
    requires index <= limit;
    terminates by (index, limit) -> Nat::BoundedDistance in 0..=4;
    -> u64 {
        transition index < limit {
            true -> self.scan_a(index + 1, limit)
            false -> index
        }
    }";

#[test]
fn prefix_call_with_a_disjoint_write_frame_preserves_the_component_ranking() {
    assert_eq!(admitted(&typed_source(PREFIX_CALL)).len(), 1);
    // The callee may write its own storage; only premise carriers matter.
    let storage_write = PREFIX_CALL
        .replace("data Main {}", "data Main { audits: u64 }")
        .replace("{ value }", "{ self.audits = self.audits + 1; value }");
    assert_eq!(admitted(&typed_source(&storage_write)).len(), 1);
}

#[test]
fn prefix_call_writing_a_premise_carrier_rejects_the_component() {
    // The callee writes through its mutable borrow into the ranked subject.
    let written = PREFIX_CALL
        .replace(
            "machine Main::audit(&mut self, value: u64) -> u64 { value }",
            "machine Main::audit(&mut self, value: &mut u64) -> u64 { value = value + 1; 0 }",
        )
        .replace("self.audit(index);", "self.audit(&mut index);")
        .replace(
            "machine Main::scan_a(&mut self, index: u64 [0..=4], limit",
            "machine Main::scan_a(&mut self, mut index: u64 [0..=4], limit",
        );
    assert_ne!(written, PREFIX_CALL);
    assert!(admitted(&typed_source(&written)).is_empty());
    // An argument hiding an authored operator has no frame evidence at all.
    let authored = format!(
        "operator - u64::sub(left: u64, right: u64) -> u64; {}",
        PREFIX_CALL.replace("self.audit(index);", "self.audit(index - 1);")
    );
    assert!(admitted(&typed_source(&authored)).is_empty());
    // A boundary callee's signature state has no body to summarize: its
    // exclusive-argument reach must not be assumed empty.
    let boundary = PREFIX_CALL
        .replace(
            "machine Main::scan_a(&mut self, index: u64 [0..=4], limit",
            "machine Main::scan_a(&mut self, mut index: u64 [0..=4], limit",
        )
        .replace("self.audit(index);", "reset(&mut index);");
    assert!(
        admitted(&typed_source(&format!(
            "boundary machine reset(value: &mut u64) -> u64 [1..=2]; {boundary}"
        )))
        .is_empty()
    );
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
    let caller_state = &program.machine_states(caller)[0];
    let query = |subject, range, actuals: &[ExpressionHandle]| {
        prove_ranking_range_call(
            &program,
            RankingRangeCallMember {
                machine: caller,
                subject,
                paired_subject: ExpressionHandle::invalid(),
                range: caller_range,
            },
            &RankingRangeCallSite {
                state: caller_state,
                entry_parameters: &[],
            },
            RankingRangeCallMember {
                machine: callee,
                subject: callee_rank.subject,
                paired_subject: callee_rank.paired_subject,
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
