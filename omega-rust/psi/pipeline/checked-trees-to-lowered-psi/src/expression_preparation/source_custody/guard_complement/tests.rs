use super::complementary;
use checked_trees::{CheckedScalarExpressionRole, CheckedTrees};

fn entry_state(checked: &CheckedTrees) -> symbols::SymbolHandle {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("pick"))
        .expect("probe machine");
    checked
        .machine_states(machine)
        .first()
        .expect("entry state")
        .symbol
}

#[test]
fn rejoined_guard_rows_decide_the_same_pairs_as_the_producers() {
    for (source, expected) in [
        (
            "machine pick(value: i32) -> i32 { transition { value == 3 -> (1) value != 3 -> (2) } }",
            true,
        ),
        (
            "data Parity [copy] { case Even; case Odd; }\n\
             machine Parity::pick(&self) -> u64 { transition self { Parity::Even -> (0) Parity::Odd -> (1) } }",
            true,
        ),
        (
            "machine pick(value: i32) -> i32 { transition { value < 3 -> (1) value >= 3 -> (2) _ -> (3) } }",
            false,
        ),
        (
            "data Tri [copy] { case A; case B; case C; }\n\
             machine Tri::pick(&self) -> u64 { transition self { Tri::A -> (0) Tri::B -> (1) _ -> (2) } }",
            false,
        ),
    ] {
        let checked = crate::front_end::checked_program(source);
        assert_eq!(
            complementary(&checked, entry_state(&checked), 0).ok(),
            Some(expected),
            "{source}"
        );
    }
}

#[test]
fn a_substituted_second_row_cannot_manufacture_a_complement() {
    let source = "machine pick(value: i32) -> i32 { transition { value < 3 -> (1) value >= 3 -> (2) _ -> (3) } }";
    let mut checked = crate::front_end::checked_program(source);
    let state = entry_state(&checked);
    let rows = &mut checked.facts.values.scalar_expressions.expressions;
    let first = rows
        .iter()
        .position(|row| {
            row.state == state
                && row.statement_ordinal == 0
                && row.role == CheckedScalarExpressionRole::Guard
        })
        .expect("first guard row");
    let second = rows
        .iter()
        .position(|row| {
            row.state == state
                && row.statement_ordinal == 1
                && row.role == CheckedScalarExpressionRole::Guard
        })
        .expect("second guard row");
    let checked_trees::CheckedScalarExpression::Boolean(first_guard) = &rows[first].expression
    else {
        panic!("Boolean guard row")
    };
    rows[second].expression = checked_trees::CheckedScalarExpression::Boolean(Box::new(
        checked_trees::CheckedBooleanExpression::Not(first_guard.clone()),
    ));
    assert!(
        !matches!(complementary(&checked, state, 0), Ok(true)),
        "`!(value < 3)` does not rejoin the authored `value >= 3`"
    );
}
