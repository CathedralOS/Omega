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

/// The per-label collision table must reproduce the per-row scans it
/// replaced: an expression row collides when any same-label expression row
/// differs in machine, state or reads, or any same-label binding differs from
/// it through its single place; a binding row collides when any same-label
/// binding names another exact place, or any same-label expression row
/// differs from its place. Incomplete reads and unresolved segments agree
/// with nothing, themselves included, so the table cannot assume reflexivity.
#[test]
fn label_collision_table_matches_every_row_scan() {
    let program = typed_program(
        "machine first(left: i64, right: i64) {
        let cut: i64 = left - right;
    }
    machine second(left: i64, right: i64) {
        let cut: i64 = left - right;
    }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let left = parameter_place(&program, first_state, "left");
    let right = parameter_place(&program, first_state, "right");
    let unresolved = CanonicalPlace {
        root: left.root,
        segments: vec![facts::PlaceSegment::Field {
            symbol: SymbolHandle::invalid(),
        }],
    };
    let owners = [
        (first.symbol, first_state.symbol),
        (second.symbol, second_state.symbol),
    ];
    let read_sets = [
        Some(vec![left.clone()]),
        Some(vec![right.clone()]),
        Some(vec![left.clone(), right.clone()]),
        Some(vec![unresolved.clone()]),
        None,
    ];
    let places = [left.clone(), right.clone(), unresolved.clone()];
    let labels = ["a", "b"];
    let scanned_expression = |facts: &RangeFacts, row: &super::ExpressionDependencies| {
        facts.expression_dependencies.iter().any(|other| {
            other.label == row.label
                && (other.machine != row.machine
                    || other.state != row.state
                    || !super::same_reads(&program, other.reads.as_deref(), row.reads.as_deref()))
        }) || facts.scalar_binding_dependencies.iter().any(|binding| {
            binding.label == row.label
                && (binding.machine != row.machine
                    || binding.state != row.state
                    || !super::same_reads(
                        &program,
                        row.reads.as_deref(),
                        Some(std::slice::from_ref(&binding.place)),
                    ))
        })
    };
    let scanned_binding = |facts: &RangeFacts, row: &super::ScalarBindingDependency| {
        facts.scalar_binding_dependencies.iter().any(|other| {
            other.label == row.label
                && (other.machine != row.machine
                    || other.state != row.state
                    || other.place != row.place)
        }) || facts.expression_dependencies.iter().any(|other| {
            other.label == row.label
                && (other.machine != row.machine
                    || other.state != row.state
                    || !super::same_reads(
                        &program,
                        other.reads.as_deref(),
                        Some(std::slice::from_ref(&row.place)),
                    ))
        })
    };
    // A fixed linear congruential walk keeps the table set deterministic.
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = |bound: usize| {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) as usize % bound
    };
    let expression = initializer(&program, first_state);
    for _ in 0..2_000 {
        let mut facts = RangeFacts::new(&[]);
        for _ in 0..next(5) {
            let (machine, state) = owners[next(owners.len())];
            facts
                .expression_dependencies
                .push(super::ExpressionDependencies {
                    expression,
                    label: labels[next(labels.len())].to_owned(),
                    machine,
                    state,
                    reads: read_sets[next(read_sets.len())].clone(),
                });
        }
        for _ in 0..next(4) {
            let (machine, state) = owners[next(owners.len())];
            facts
                .scalar_binding_dependencies
                .push(super::ScalarBindingDependency {
                    label: labels[next(labels.len())].to_owned(),
                    machine,
                    state,
                    place: places[next(places.len())].clone(),
                });
        }
        let collisions = facts.label_collisions(&program);
        for row in &facts.expression_dependencies {
            assert_eq!(
                collisions.expression_labels.contains(row.label.as_str()),
                scanned_expression(&facts, row),
            );
        }
        for row in &facts.scalar_binding_dependencies {
            assert_eq!(
                collisions.binding_labels.contains(row.label.as_str()),
                scanned_binding(&facts, row),
            );
        }
    }
}
