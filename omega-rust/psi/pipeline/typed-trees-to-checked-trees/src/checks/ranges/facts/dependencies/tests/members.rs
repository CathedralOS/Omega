use super::{ExpressionNode, SymbolHandle, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::facts::RangeCallContext;
use crate::checks::ranges::facts::dependencies::tests::initializer;
use crate::checks::ranges::facts::dependencies::tests::parameter_place;
use crate::checks::ranges::facts::dependencies::tests::typed_source;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

fn window(program: &TypedTrees) -> (&Machine, &State) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("window"))
        .expect("window");
    (machine, &program.machine_states(machine)[0])
}

fn statement_index_of(program: &TypedTrees, state: &State, name: &str) -> usize {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .position(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.name.as_str() == name)
        })
        .expect("named local statement")
}

/// Build the checked call occurrence evidence the statement checker hands to
/// range facts: the same borrow/flow join and call-frame resolver production
/// constructs per owning state.
fn checked_facts<'program>(
    program: &'program TypedTrees,
) -> (
    checked_trees::BorrowFacts,
    checked_trees::FlowFacts,
    Option<validation::CallFrameResolver<'program>>,
) {
    let borrows = crate::borrow::build_borrow_facts(program);
    let flow = crate::checks::ranges::cache_tests::range_flow_fixture(program, &borrows);
    let frames = validation::CallFrameResolver::new(program);
    (borrows, flow, frames)
}

/// Record the `let cut = ..` initializer's dependencies with the statement
/// index the statement checker would have set, returning its display label.
fn record_label(
    facts: &mut RangeFacts,
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
) -> String {
    let expression = initializer(program, state);
    facts.statement_index = statement_index_of(program, state, "cut");
    facts.record_expression_dependencies(program, machine, state, expression);
    program.expression_table.display_name(expression)
}

/// A member on a call's returned temporary reads exactly the checked call's
/// footprint: `compute(seed).a` cannot name result storage, so its read set
/// is whatever evaluating `compute(seed)` read — no more and no less.
#[test]
fn a_call_receiver_member_reads_the_call_footprint() {
    let program = typed_source(
        "data Pair { a: i64; b: i64; }
        machine compute(seed: i64) -> Pair { Pair { a: seed, b: 0 } }
        machine window(seed: i64, other: i64, unrelated: i64) {
            let cut: i64 = compute(seed).a;
        }",
    );
    let (machine, state) = window(&program);
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    let label = record_label(&mut facts, &program, machine, state);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("call-receiver member footprint");
    assert_eq!(
        reads.as_slice(),
        [parameter_place(&program, state, "seed")].as_slice(),
        "{reads:?}"
    );
    for (name, survives) in [("seed", false), ("other", true), ("unrelated", true)] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
}

/// `Pair { a: left, b: right }.a` evaluates every initializer where the
/// literal appears, so the member's footprint is the literal's own read set
/// even though the projection selects only one field.
#[test]
fn a_literal_receiver_member_reads_each_initializer() {
    let program = typed_source(
        "data Pair { a: i64; b: i64; }
        machine window(left: i64, right: i64, unrelated: i64) {
            let cut: i64 = Pair { a: left, b: right }.a;
        }",
    );
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    let label = record_label(&mut facts, &program, machine, state);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("literal-receiver member footprint");
    assert_eq!(
        reads.as_slice(),
        [
            parameter_place(&program, state, "left"),
            parameter_place(&program, state, "right"),
        ]
        .as_slice(),
        "{reads:?}"
    );
    for (name, survives) in [("left", false), ("right", false), ("unrelated", true)] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
}

/// A deeper member on a temporary keeps the producing footprint:
/// `compute(seed).inner.a` still reads only what `compute(seed)` read, since
/// neither projection names caller storage.
#[test]
fn a_nested_temporary_member_reads_the_producing_footprint() {
    let program = typed_source(
        "data Inner { a: i64; b: i64; }
        data Outer { inner: Inner; other: i64; }
        machine compute(seed: i64) -> Outer { Outer { inner: Inner { a: seed, b: 0 }, other: 0 } }
        machine window(seed: i64, unrelated: i64) {
            let cut: i64 = compute(seed).inner.a;
        }",
    );
    let (machine, state) = window(&program);
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    let label = record_label(&mut facts, &program, machine, state);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("nested temporary member footprint");
    assert_eq!(
        reads.as_slice(),
        [parameter_place(&program, state, "seed")].as_slice(),
        "{reads:?}"
    );
    let writes = [parameter_place(&program, state, "unrelated")];
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, Some(&writes))
            .contains(&label),
        "a disjoint write preserves the premise"
    );
}

/// `self.compute().a` inherits the callee's receiver footprint: the machine
/// storage the `&self` callee reads is the only caller place the projection
/// can observe.
#[test]
fn a_self_call_receiver_member_reads_the_receiver_footprint() {
    let program = typed_source(
        "data Pair { a: i64; b: i64; }
        data Main { seed: i64; other: i64; }
        machine Main::compute(&self) -> Pair { Pair { a: self.seed, b: 0 } }
        machine Main::window(&mut self, unrelated: i64) -> i64 {
            let cut: i64 = self.compute().a;
            cut
        }",
    );
    let (machine, state) = window(&program);
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    let label = record_label(&mut facts, &program, machine, state);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("self-call member footprint");
    let machine_root = crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(machine.symbol),
        segments: Vec::new(),
    };
    assert!(reads.contains(&machine_root), "{reads:?}");
    assert!(
        !facts
            .preserved_expression_labels(&program, machine, state, Some(&[machine_root]))
            .contains(&label),
        "a write through self retires the premise"
    );
    let writes = [parameter_place(&program, state, "unrelated")];
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, Some(&writes))
            .contains(&label),
        "a disjoint parameter write preserves it"
    );
}

/// A temporary member nested as a selector operand keeps both footprints:
/// `items[compute(seed).a as u64]` reads the index's producing call and the
/// element place it selects.
#[test]
fn a_temporary_member_inside_a_selector_keeps_both_footprints() {
    let program = typed_source(
        "data Pair { a: i64; b: i64; }
        machine compute(seed: i64) -> Pair { Pair { a: seed, b: 0 } }
        machine window(seed: i64, items: &[i64; 4], unrelated: i64) {
            let cut: i64 = items[compute(seed).a as u64];
        }",
    );
    let (machine, state) = window(&program);
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    let label = record_label(&mut facts, &program, machine, state);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("selector-nested temporary member footprint");
    assert!(
        reads.contains(&parameter_place(&program, state, "seed")),
        "{reads:?}"
    );
    assert_eq!(
        reads.len(),
        2,
        "the element place and the call read: {reads:?}"
    );
    for (name, survives) in [("seed", false), ("items", false), ("unrelated", true)] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
}

/// Without the exact checked call occurrence at this statement, the receiver
/// has no statement-use custody and `compute(seed).a` cannot enumerate the
/// callee's reads — the read set stays incomplete.
#[test]
fn a_call_receiver_member_without_call_custody_stays_incomplete() {
    let program = typed_source(
        "data Pair { a: i64; b: i64; }
        machine compute(seed: i64) -> Pair { Pair { a: seed, b: 0 } }
        machine window(seed: i64, unrelated: i64) {
            let cut: i64 = compute(seed).a;
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    // No checked-call context at all: the receiver's operand footprint cannot
    // be authenticated, so the member's read set stays opaque.
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index_of(&program, state, "cut");
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
    // A present context whose statement index cannot join the authored call
    // is the same incomplete answer.
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index_of(&program, state, "cut") + 1;
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// A member spelling that resolves to no declared field of the receiver's
/// type has no honest identity: dropping the binder's symbol alone is not
/// enough while the contextual walk still finds `Pair::a`, so the drifted
/// name must be unresolvable on `Pair`.
#[test]
fn a_temporary_member_with_unresolved_identity_stays_incomplete() {
    let mut program = typed_source(
        "data Pair { a: i64; b: i64; }
        machine window(left: i64, right: i64, unrelated: i64) {
            let cut: i64 = Pair { a: left, b: right }.a;
        }",
    );
    let expression = {
        let (_, state) = window(&program);
        initializer(&program, state)
    };
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression) else {
        panic!("member fixture")
    };
    member.member_symbol = SymbolHandle::invalid();
    member.member = "missing".into();
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    record_label(&mut facts, &program, machine, state);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// Member identity on a `match` result temporary cannot be recovered from
/// the receiver's position — the match carries no declared result type the
/// member walk can stand on — so `match .. .a` keeps an incomplete read set.
#[test]
fn a_match_receiver_member_stays_incomplete() {
    let program = typed_source(
        "data Pair { a: i64; b: i64; }
        machine window(flag: i64, left: Pair, right: Pair, unrelated: i64) {
            let cut: i64 = match flag { 0 -> left, _ -> right }.a;
        }",
    );
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    record_label(&mut facts, &program, machine, state);
    assert!(facts.expression_dependencies[0].reads.is_none());
}
