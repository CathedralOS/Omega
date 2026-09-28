use super::{ExpressionHandle, ExpressionNode, State, SymbolHandle, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::flow::CanonicalPlace;
use crate::tests::front_end::typed_program;
use typed_trees::statement::StatementNode;

mod atomics;
mod borrows;
pub(crate) mod calls;
mod depth;
mod indexes;
mod members;

fn initializer(program: &TypedTrees, state: &State) -> ExpressionHandle {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "cut" => {
                Some(local.initial_value)
            }
            _ => None,
        })
        .expect("computed local")
}

fn parameter_place(program: &TypedTrees, state: &State, name: &str) -> CanonicalPlace {
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
        .expect("parameter");
    CanonicalPlace {
        root: facts::PlaceRoot::Symbol(parameter.symbol),
        segments: Vec::new(),
    }
}

/// Build the checked operator evidence production hands to range facts: the
/// same value/operator fact construction, including domain selection, so a
/// recorded use row is the exact occurrence custody the checker consults.
fn selected_operator_facts(program: &TypedTrees) -> checked_trees::CheckedOperatorFacts {
    let proof_plan = proof::obligations::build_proof_plan(program);
    let values = crate::values::build_value_facts(program, &proof_plan);
    let mut operators = crate::operators::build_operator_facts(program, &values);
    crate::operators::select_pending_domain_operator_meanings(program, &mut operators);
    operators
}

#[test]
fn retention_requires_complete_disjoint_writes_for_every_operand() {
    let program = typed_program(
        "machine window(left: i64, right: i64, unrelated: i64) {
        let cut: i64 = left - right;
    }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let label = program.expression_table.display_name(expression);
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
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, None)
            .is_empty()
    );
    for root in [
        facts::PlaceRoot::Unknown,
        facts::PlaceRoot::Symbol(SymbolHandle::invalid()),
        facts::PlaceRoot::Expression(expression),
    ] {
        let writes = [CanonicalPlace {
            root,
            segments: Vec::new(),
        }];
        assert!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .is_empty()
        );
    }
}

#[test]
fn identical_labels_cannot_choose_between_different_typed_reads() {
    let program = typed_program(
        "machine first(original: i64, unrelated: i64) {
        let cut: i64 = original - 1;
    }
    machine second(original: i64, unrelated: i64) {
        let cut: i64 = original - 1;
    }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let first_expression = initializer(&program, first_state);
    let second_expression = initializer(&program, second_state);
    assert_ne!(first_expression, second_expression);
    assert_eq!(
        program.expression_table.display_name(first_expression),
        program.expression_table.display_name(second_expression)
    );
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.record_expression_dependencies(&program, first, first_state, first_expression);
    let writes = [parameter_place(&program, first_state, "unrelated")];
    assert_eq!(
        facts
            .preserved_expression_labels(&program, first, first_state, Some(&writes))
            .len(),
        1
    );
    // Repeated registration of the same exact meaning is harmless.
    facts.record_expression_dependencies(&program, first, first_state, first_expression);
    assert_eq!(facts.expression_dependencies.len(), 1);
    facts.record_expression_dependencies(&program, second, second_state, second_expression);
    assert!(
        facts
            .preserved_expression_labels(&program, first, first_state, Some(&writes))
            .is_empty()
    );
}
