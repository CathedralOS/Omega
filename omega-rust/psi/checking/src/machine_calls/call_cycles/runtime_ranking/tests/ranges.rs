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
fn prefix_let_bound_to_a_boundary_result_keeps_the_entry_telescope() {
    // A value-position boundary call in an initializer frames through its
    // signature the same way: `observe` writes nothing, so the fresh local
    // cannot disturb a premise carrier.
    let source = PREFIX_CALL.replace(
        "self.audit(index);",
        "let seen: u64 = observe(index); self.audit(index);",
    );
    let admitted_source = format!("boundary machine observe(value: u64) -> u64; {source}");
    assert_eq!(admitted(&typed_source(&admitted_source)).len(), 1);
    // The same initializer behind an exclusive-argument boundary signature
    // frames its write to the carrier it borrows: `index` is protected, so
    // the call -- not the premise -- must give way.
    let written = format!(
        "boundary machine observe(value: &mut u64) -> u64; {}",
        source
            .replace("observe(index)", "observe(&mut index)")
            .replace(
                "machine Main::scan_a(&mut self, index: u64 [0..=4], limit",
                "machine Main::scan_a(&mut self, mut index: u64 [0..=4], limit",
            )
    );
    assert!(admitted(&typed_source(&written)).is_empty());
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
fn runtime_division_actuals_substitute_through_call_sites() {
    // `spare`/`extra` ride outside every endpoint atom: the runtime-divisor
    // actual exercises non-polynomial call substitution itself while the
    // carried endpoints keep the exact `cap`/`step` operands.
    let source = "data Main {}
        machine Main::first(&mut self, remaining: u64 [0..=5], cap: u64 [0..=20], step: u64 [1..=5], spare: u64)
        terminates by remaining in 0..(cap / step + 6);
        -> u64 {
            transition remaining > 0 {
                true -> self.second(cap, remaining - 1, step, cap % step)
                false -> remaining
            }
        }
        machine Main::second(&mut self, limit: u64 [0..=20], pending: u64 [0..=5], width: u64 [1..=5], extra: u64)
        terminates by pending in 0..(limit / width + 6);
        -> u64 {
            transition pending > 0 {
                true -> self.first(pending, limit, width, extra)
                false -> pending
            }
        }";
    assert_eq!(admitted(&typed_source(source)).len(), 1);
    // A zero modulus inside the actual still fails its own formation: the
    // substitution must not launder an undefined operation.
    let zeroed = source.replace("cap % step)", "cap % (step - step))");
    assert!(admitted(&typed_source(&zeroed)).is_empty());
    // Moving the remainder actual into the endpoint-read `limit` slot changes
    // the endpoint: `(cap % step) / width` is not `cap / step`.
    let moved = source.replace(
        "self.second(cap, remaining - 1, step, cap % step)",
        "self.second(cap % step, remaining - 1, step, cap)",
    );
    assert!(admitted(&typed_source(&moved)).is_empty());
}

#[test]
fn mixed_component_runtime_division_actuals_keep_endpoint_conservation() {
    // One ranged member and one unranged member: `cap`/`step` are conserved
    // endpoint inputs while `extra` only has to transport the remainder
    // actual's own exact value.
    let source = "data Main {}
        machine Main::outer(&mut self, remaining: u64 [0..=5], cap: u64 [0..=20], step: u64 [1..=5], spare: u64)
        terminates by remaining in 0..(cap / step + 6);
        -> u64 {
            transition remaining > 0 {
                true -> self.inner(remaining - 1, cap, step, cap % step)
                false -> remaining
            }
        }
        machine Main::inner(&mut self, n: u64 [0..=5], limit: u64 [0..=20], width: u64 [1..=5], extra: u64)
        terminates by n;
        -> u64 {
            transition n > 0 {
                true -> self.outer(n, limit, width, extra)
                false -> n
            }
        }";
    assert_eq!(admitted(&typed_source(source)).len(), 1);
    // A different moved value in the pinned `width` slot is not the endpoint
    // input the range reads.
    let moved = source.replace(
        "self.outer(n, limit, width, extra)",
        "self.outer(n, limit, extra, extra)",
    );
    assert!(admitted(&typed_source(&moved)).is_empty());
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
