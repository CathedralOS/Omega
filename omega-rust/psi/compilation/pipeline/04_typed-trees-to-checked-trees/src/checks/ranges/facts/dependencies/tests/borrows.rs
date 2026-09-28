use super::{ExpressionNode, TypedTrees};
use crate::checks::ranges::RangeFacts;
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

/// The same peel applies when the borrow is a builtin index collection:
/// `(&items)[i]` reads the element place `items[i]` plus the selector
/// operand `i`.
#[test]
fn a_borrowed_index_collection_reads_the_element_place() {
    let program = typed_program(
        "machine window(items: &[i64; 4], index: u64, unrelated: u64) {
            let cut: i64 = (&items)[index];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("borrowed collection footprint");
    let mut element = parameter_place(&program, state, "items");
    element.segments.push(facts::PlaceSegment::Index {
        expression: {
            let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
            else {
                panic!("indexed fixture")
            };
            indexed.index
        },
    });
    assert_eq!(
        reads.as_slice(),
        [parameter_place(&program, state, "index"), element.clone(),].as_slice(),
        "{reads:?}"
    );
    let label = program.expression_table.display_name(expression);
    for (write, survives) in [
        (parameter_place(&program, state, "items"), false),
        (parameter_place(&program, state, "index"), false),
        (parameter_place(&program, state, "unrelated"), true),
    ] {
        assert_eq!(
            facts
                .preserved_expression_labels(
                    &program,
                    machine,
                    state,
                    Some(std::slice::from_ref(&write))
                )
                .contains(&label),
            survives,
            "{write:?}"
        );
    }
}

/// Borrowing a temporary is not a place read: `(&compute()).a` cannot name
/// the call result's storage, so the read set stays incomplete rather than
/// borrowing the callee's operand footprint for a field the caller cannot
/// see.
#[test]
fn a_borrow_of_a_call_result_stays_incomplete() {
    let program = typed_program(
        "data Pair { a: i64; b: i64; }
        machine compute() -> Pair { Pair { a: 0, b: 0 } }
        machine window(pair: Pair, unrelated: i64) {
            let cut: i64 = (&compute()).a;
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}
