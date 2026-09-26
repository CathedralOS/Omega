use super::{StatementNode, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::facts::RangeCallContext;
use crate::checks::ranges::facts::dependencies::tests::initializer;
use crate::checks::ranges::facts::dependencies::tests::parameter_place;
use crate::tests::front_end::typed_program;
use typed_trees::machine::Machine;
use typed_trees::state::State;

fn window(program: &TypedTrees) -> (&Machine, &State) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "window")
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

/// A const application selects a compile-time value. It reaches the callee as
/// a substituted static value rather than storage, so the operand accesses
/// still enumerate the complete footprint.
#[test]
fn a_const_applied_generic_call_reads_its_operand_footprint() {
    let program = typed_program(
        "machine scaled<const Factor: u64>(value: u64) -> u64 { value }
        machine window(original: u64, index: u64, unrelated: u64) {
            let cut: u64 = scaled<2u64>(original);
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("a const-applied call footprint");
    assert_eq!(
        reads.as_slice(),
        &[parameter_place(&program, state, "original")]
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [("original", false), ("index", true), ("unrelated", true)] {
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

/// A machine-valued static argument hands the callee a callable body whose
/// own reads this occurrence never authenticated, and a nested static
/// application can carry such a binder below the argument this scan sees.
/// Both keep the read set incomplete.
#[test]
fn a_machine_valued_or_nested_static_application_stays_incomplete() {
    for source in [
        "machine double(value: u64) -> u64 { value }
        machine apply<machine Chosen>(value: u64) -> u64
        where machine Chosen(value: u64) -> u64;
        { Chosen(value) }
        machine window(original: u64, unrelated: u64) {
            let cut: u64 = apply<double>(original);
        }",
        "data Pair<Element> { value: Element; }
        machine identity<Element>(value: &Element) -> u64 { 0u64 }
        machine window(original: Pair<u64>, unrelated: u64) {
            let cut: u64 = identity<Pair<u64>>(&original);
        }",
    ] {
        let program = typed_program(source);
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let statement_index = statement_index_of(&program, state, "cut");
        let (borrows, flow, frames) = checked_facts(&program);
        let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
        let mut facts = RangeFacts::new(&[]);
        facts.bound_program = Some(&program);
        facts.checked_calls = Some(&context);
        facts.statement_index = statement_index;
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(facts.expression_dependencies[0].reads.is_none(), "{source}");
    }
}
