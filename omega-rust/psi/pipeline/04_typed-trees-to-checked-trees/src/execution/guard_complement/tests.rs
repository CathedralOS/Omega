use super::guard_pair;
use crate::tests::front_end::checked_program;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole, CheckedScalarStateTerminator, CheckedTrees,
};

fn plans(
    first: CheckedBooleanExpression,
    second: CheckedBooleanExpression,
) -> CheckedScalarExpressionPlans {
    let state = symbols::SymbolHandle::from_arena_index(1);
    CheckedScalarExpressionPlans {
        expressions: [first, second]
            .into_iter()
            .enumerate()
            .map(
                |(index, expression)| checked_trees::CheckedLocatedScalarExpression {
                    state,
                    statement_ordinal: u32::try_from(index).unwrap(),
                    role: CheckedScalarExpressionRole::Guard,
                    expression: CheckedScalarExpression::Boolean(Box::new(expression)),
                },
            )
            .collect(),
        ..Default::default()
    }
}

#[test]
fn guard_pairs_require_one_selected_row_per_exact_coordinate() {
    let parameter = CheckedBooleanExpression::Parameter { position: 0 };
    let original = plans(
        parameter.clone(),
        CheckedBooleanExpression::Not(Box::new(parameter)),
    );
    let state = symbols::SymbolHandle::from_arena_index(1);
    assert!(guard_pair(&original, state, 0).is_some());
    for mutation in 0..6 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                changed.expressions.pop();
            }
            1 => {
                changed.expressions.remove(0);
            }
            2 => changed.expressions.push(changed.expressions[0].clone()),
            3 => changed.expressions.push(changed.expressions[1].clone()),
            4 => changed.expressions[1].state = symbols::SymbolHandle::from_parts(1, 2),
            _ => changed.expressions[1].role = CheckedScalarExpressionRole::Return,
        }
        assert!(
            guard_pair(&changed, state, 0).is_none(),
            "mutation {mutation}"
        );
    }
    assert!(guard_pair(&original, state, u32::MAX).is_none());
}

fn machine_named(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

/// The scalar graph's entry terminator, when the scalar family admits it.
fn scalar_entry(checked: &CheckedTrees, name: &str) -> Option<CheckedScalarStateTerminator> {
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine_named(checked, name))?;
    Some(graph.states.first()?.terminator.clone())
}

#[test]
fn scalar_graphs_admit_builtin_equality_and_two_case_pairs() {
    let checked = checked_program(
        r#"
        data Parity [copy] { case Even; case Odd; }
        machine pick(value: i32) -> i32 {
            transition {
                value == 3 -> (1)
                value != 3 -> (2)
            }
        }
        machine Parity::bit(&self) -> u64 {
            transition self {
                Parity::Even -> (0)
                Parity::Odd -> (1)
            }
        }
        "#,
    );
    for name in ["pick", "bit"] {
        assert!(
            matches!(
                scalar_entry(&checked, name),
                Some(CheckedScalarStateTerminator::Conditional { .. })
            ),
            "{name}: {:?}",
            scalar_entry(&checked, name)
        );
    }
}

/// The shared judgment over one machine's real Guard rows at statements 0/1.
/// Each source closes with `_`, which the front end requires whenever the
/// first two arms do not already cover every value.
fn first_pair_is_complementary(source: &str) -> bool {
    let checked = checked_program(source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("pick"))
        .expect("probe machine");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("entry state");
    assert!(
        guard_pair(&checked.facts.values.scalar_expressions, state.symbol, 0).is_some(),
        "both authored guards retain one checked row: {source}"
    );
    super::complementary(
        &checked.typed,
        &checked.facts.values.scalar_expressions,
        state,
        0,
    )
}

#[test]
fn ordered_and_three_case_pairs_are_not_complements_on_selected_rows() {
    for (source, expected) in [
        (
            "machine pick(value: i32) -> i32 { transition { value == 3 -> (1) value != 3 -> (2) _ -> (3) } }",
            true,
        ),
        (
            "machine pick(value: i32) -> i32 { transition { value < 3 -> (1) value >= 3 -> (2) _ -> (3) } }",
            false,
        ),
        (
            "machine pick(value: i32) -> i32 { transition { value > 3 -> (1) value <= 3 -> (2) _ -> (3) } }",
            false,
        ),
        (
            "data Tri [copy] { case A; case B; case C; }\n\
             machine Tri::pick(&self) -> u64 { transition self { Tri::A -> (0) Tri::B -> (1) _ -> (2) } }",
            false,
        ),
    ] {
        assert_eq!(first_pair_is_complementary(source), expected, "{source}");
    }
}
