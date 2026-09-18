use super::{ExpressionNode, StatementNode, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::facts::RangeCallContext;
use crate::checks::ranges::facts::dependencies::tests::initializer;
use crate::checks::ranges::facts::dependencies::tests::parameter_place;
use crate::checks::ranges::facts::dependencies::tests::selected_operator_facts;
use crate::checks::ranges::facts::dependencies::tests::typed_source;
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

fn machine_state<'a>(program: &'a TypedTrees, machine_name: &str) -> (&'a Machine, &'a State) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect(machine_name);
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

#[test]
fn a_selected_call_reads_only_its_established_operand_footprint() {
    let program = typed_source(
        "machine compute(original: u64) -> u64 { original }
        machine window(original: u64, index: u64, unrelated: u64) {
            let cut: u64 = compute(original);
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("an established call footprint");
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

#[test]
fn borrow_arguments_carry_the_callee_readable_place() {
    let program = typed_source(
        "machine observe(target: &mut u64) -> u64 { 0 }
        machine window(mut original: u64, unrelated: u64) {
            let cut: u64 = observe(&mut original);
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("borrowed argument footprint");
    assert!(
        reads.contains(&parameter_place(&program, state, "original")),
        "{reads:?}"
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [("original", false), ("unrelated", true)] {
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

#[test]
fn a_self_receiver_callee_reads_the_callers_machine_storage() {
    let program = typed_source(
        "data Main { count: u64; other: u64; }
        machine Main::measure(&self) -> u64 { self.count }
        machine Main::window(&mut self, unrelated: u64) -> u64 {
            let cut: u64 = self.measure();
            cut
        }",
    );
    let (machine, state) = machine_state(&program, "Main::window");
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("self-receiver callee footprint");
    // The footprint must contain the whole-machine read the `&self` callee
    // performs; the entry parameter's own root is a redundant alias for the
    // same storage and may appear alongside it.
    let machine_root = crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(machine.symbol),
        segments: Vec::new(),
    };
    assert!(reads.contains(&machine_root), "{reads:?}");
    let label = program.expression_table.display_name(expression);
    let self_write = machine_root;
    assert!(
        !facts
            .preserved_expression_labels(&program, machine, state, Some(&[self_write]))
            .contains(&label),
        "a write through self must retire the callee-read premise"
    );
    let writes = [parameter_place(&program, state, "unrelated")];
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, Some(&writes))
            .contains(&label),
        "a disjoint parameter write must preserve it"
    );
}

#[test]
fn a_nested_call_selector_extends_the_indexed_read_set() {
    let program = typed_source(
        "machine compute(index: u64) -> u64 { index }
        machine window(items: &[u64; 4], index: u64, unrelated: u64) {
            let cut: u64 = items[compute(index)];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("call-selected indexed footprint");
    assert_eq!(
        reads.len(),
        2,
        "the element place and the selector reads: {reads:?}"
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [("items", false), ("index", false), ("unrelated", true)] {
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

#[test]
fn missing_or_foreign_occurrence_evidence_keeps_the_read_set_incomplete() {
    let program = typed_source(
        "machine compute(original: u64) -> u64 { original }
        machine window(original: u64, unrelated: u64) {
            let cut: u64 = compute(original);
        }
        machine other(original: u64) {
            let rest: u64 = original;
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let (borrows, flow, frames) = checked_facts(&program);
    // Occurrence evidence is owner-local: a context built for another state
    // cannot locate this statement's call, so the footprint stays opaque.
    let other_machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "other")
        .expect("other");
    let other_state = &program.machine_states(other_machine)[0];
    let foreign =
        RangeCallContext::new(other_machine, other_state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&foreign);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
    // A present context whose statement index cannot join the authored call
    // is the same incomplete answer.
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index + 1;
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

#[test]
fn a_static_binder_or_hidden_argument_keeps_the_footprint_incomplete() {
    let mut program = typed_source(
        "machine compute(original: u64) -> u64 { original }
        machine window(original: u64, unrelated: u64) {
            let cut: u64 = compute(original);
        }",
    );
    let expression = {
        let (_, state) = window(&program);
        initializer(&program, state)
    };
    // A static binder can name callee-reachable storage the operand scan
    // cannot see; the typed field alone must block the footprint even though
    // the checked occurrence still exists.
    let binder = program.machines()[0].symbol;
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
        panic!("call fixture")
    };
    call.static_machine_parameter = binder;
    let (machine, state) = window(&program);
    let statement_index = statement_index_of(&program, state, "cut");
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// A static type application substitutes a declaration identity, never a
/// place: specialization "creates no runtime dictionary", so a statically
/// applied call reads exactly what the same call without the application
/// reads — the checked operand accesses at the exact occurrence.
#[test]
fn a_type_applied_generic_call_reads_its_established_operand_footprint() {
    for source in [
        "machine identity<Element>(value: Element) -> Element { value }
        machine window(original: u64, index: u64, unrelated: u64) {
            let cut: u64 = identity<u64>(original);
        }",
        "data Card { count: u64; }
        machine identity<Element>(value: &Element) -> u64 { 0u64 }
        machine window(original: Card, index: u64, unrelated: u64) {
            let cut: u64 = identity<Card>(&original);
        }",
    ] {
        let program = typed_source(source);
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let statement_index = statement_index_of(&program, state, "cut");
        let (borrows, flow, frames) = checked_facts(&program);
        let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
        let mut facts = RangeFacts::new(&[]);
        facts.checked_calls = Some(&context);
        facts.statement_index = statement_index;
        facts.record_expression_dependencies(&program, machine, state, expression);
        let reads = facts.expression_dependencies[0]
            .reads
            .as_ref()
            .expect("a statically applied call footprint");
        assert_eq!(
            reads.as_slice(),
            &[parameter_place(&program, state, "original")],
            "{source}"
        );
        let label = program.expression_table.display_name(expression);
        for (name, survives) in [("original", false), ("index", true), ("unrelated", true)] {
            let writes = [parameter_place(&program, state, name)];
            assert_eq!(
                facts
                    .preserved_expression_labels(&program, machine, state, Some(&writes))
                    .contains(&label),
                survives,
                "write to {name} under {source}"
            );
        }
    }
}

/// A const application selects a compile-time value. It reaches the callee as
/// a substituted static value rather than storage, so the operand accesses
/// still enumerate the complete footprint.
#[test]
fn a_const_applied_generic_call_reads_its_operand_footprint() {
    let program = typed_source(
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

/// The receiver rule is unchanged by the application: an applied `&self`
/// callee still reads the caller's machine storage, so a write through `self`
/// retires the premise.
#[test]
fn a_type_applied_self_receiver_call_reads_the_callers_machine_storage() {
    let program = typed_source(
        "data Main { count: u64; other: u64; }
        machine Main::measure<Element>(&self, value: Element) -> u64 { self.count }
        machine Main::window(&mut self, original: u64, unrelated: u64) -> u64 {
            let cut: u64 = self.measure<u64>(original);
            cut
        }",
    );
    let (machine, state) = machine_state(&program, "Main::window");
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("an applied self-receiver callee footprint");
    let machine_root = crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(machine.symbol),
        segments: Vec::new(),
    };
    assert!(reads.contains(&machine_root), "{reads:?}");
    let label = program.expression_table.display_name(expression);
    assert!(
        !facts
            .preserved_expression_labels(&program, machine, state, Some(&[machine_root]))
            .contains(&label),
        "a write through self must retire the applied callee-read premise"
    );
    let writes = [parameter_place(&program, state, "unrelated")];
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, Some(&writes))
            .contains(&label),
        "a disjoint parameter write must preserve it"
    );
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
        let program = typed_source(source);
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let statement_index = statement_index_of(&program, state, "cut");
        let (borrows, flow, frames) = checked_facts(&program);
        let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
        let mut facts = RangeFacts::new(&[]);
        facts.checked_calls = Some(&context);
        facts.statement_index = statement_index;
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(facts.expression_dependencies[0].reads.is_none(), "{source}");
    }
}

/// The static selection's own kind decides, not the mere presence of an
/// application: one admitted occurrence becomes incomplete when its argument
/// is retargeted at a callable state symbol or given a nested application,
/// with the checked call evidence untouched.
#[test]
fn only_storage_free_static_selections_admit_the_applied_call_footprint() {
    for drift in ["none", "callable", "nested"] {
        let mut program = typed_source(
            "machine identity<Element>(value: Element) -> Element { value }
            machine window(original: u64, unrelated: u64) {
                let cut: u64 = identity<u64>(original);
            }",
        );
        let expression = {
            let (_, state) = window(&program);
            initializer(&program, state)
        };
        let callable = {
            let identity = &program.machines()[0];
            program.machine_states(identity)[0].symbol
        };
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("applied call fixture")
        };
        match drift {
            "callable" => call.machine_arguments[0].symbol = callable,
            "nested" => {
                call.machine_arguments[0].application =
                    Some(Box::new(typed_trees::expression::StaticSymbolApplication {
                        lifetime_arguments: Box::default(),
                        arguments: Box::default(),
                    }))
            }
            _ => {}
        }
        let (machine, state) = window(&program);
        let statement_index = statement_index_of(&program, state, "cut");
        let (borrows, flow, frames) = checked_facts(&program);
        let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
        let mut facts = RangeFacts::new(&[]);
        facts.checked_calls = Some(&context);
        facts.statement_index = statement_index;
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert_eq!(
            facts.expression_dependencies[0].reads.is_some(),
            drift == "none",
            "{drift}"
        );
    }
}

/// Walking through a cast wrapper relaxes nothing below it: the authored
/// application under the wrapper still recurses into its own operands, so a
/// call standing in one of them must prove its exact checked occurrence.
/// `items[(low + compute(step)) as u64]` reads `low`, the call's operand place
/// `step`, and the selected element; without the checked-call join the same
/// wrapped selector stays incomplete.
#[test]
fn a_wrapped_arithmetic_selector_still_proves_its_call_operand_footprint() {
    let program = typed_source(
        "operator + u64::custom(left: u64, right: u64) -> u64;
        machine compute(original: u64) -> u64 { original }
        machine window(items: &[i64; 4], low: u64, step: u64, unrelated: u64) {
            let cut: i64 = items[(low + compute(step)) as u64];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let operators = selected_operator_facts(&program);
    let (borrows, flow, frames) = checked_facts(&program);
    let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
    let mut facts = RangeFacts::new(&[]);
    facts.checked_operators = Some(&operators);
    facts.checked_calls = Some(&context);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("a wrapped application whose call operand carries custody");
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        panic!("index fixture")
    };
    let mut selected = parameter_place(&program, state, "items");
    selected.segments.push(facts::PlaceSegment::Index {
        expression: indexed.index,
    });
    assert_eq!(
        reads.as_slice(),
        [
            parameter_place(&program, state, "low"),
            parameter_place(&program, state, "step"),
            selected,
        ]
        .as_slice()
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [
        ("low", false),
        ("step", false),
        ("items", false),
        ("unrelated", true),
    ] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
    let mut without_calls = RangeFacts::new(&[]);
    without_calls.checked_operators = Some(&operators);
    without_calls.statement_index = statement_index;
    without_calls.record_expression_dependencies(&program, machine, state, expression);
    assert!(
        without_calls.expression_dependencies[0].reads.is_none(),
        "a wrapped selector claimed a call operand footprint without the checked join"
    );
}
