use super::parse_typed_trees;
use crate::lower_typed_trees;
use typed_trees::{TypedTrees, expression::ExpressionNode, statement::StatementNode};

fn check(source: &str, accepted: bool) {
    check_program(parse_typed_trees(source), accepted, source);
}

fn check_program(program: TypedTrees, accepted: bool, description: &str) {
    match lower_typed_trees(program) {
        Ok(_) => assert!(accepted, "unproved result bounds accepted: {description}"),
        Err(diagnostics) => {
            assert!(!accepted, "{description}: {diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
                "expected an exit proof rejection for {description}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn immutable_return_bounds_prove_the_original_choose_guarantee() {
    check(
        "machine choose(value: u64 [2..=4]) -> u64 [0..=4]
         ensures result >= 2 { value }",
        true,
    );
}

#[test]
fn immutable_return_bounds_preserve_strict_flipped_and_disjoint_comparisons() {
    for (guarantee, accepted) in [
        ("result > 1", true),
        ("1 < result", true),
        ("2 <= result", true),
        ("result < 5", true),
        ("5 > result", true),
        ("4 >= result", true),
        ("result != 1", true),
        ("5 != result", true),
        ("result >= 2 && result <= 4", true),
        ("result < 2 || 4 >= result", true),
        ("result > 2", false),
        ("2 < result", false),
        ("result < 4", false),
        ("4 > result", false),
        ("result == 2", false),
        ("result != 3", false),
        ("result >= 3", false),
        ("result <= 3", false),
    ] {
        check(
            &format!(
                "machine choose(value: u64 [2..=4]) -> u64 [0..=4]
                 ensures {guarantee} {{ value }}"
            ),
            accepted,
        );
    }
}

#[test]
fn singleton_return_bounds_prove_equality_in_both_orientations() {
    for (guarantee, accepted) in [
        ("result == 3", true),
        ("3 == result", true),
        ("result == 2", false),
        ("result != 3", false),
    ] {
        check(
            &format!(
                "machine choose(value: u64 [3..=3]) -> u64 [0..=4]
                 ensures {guarantee} {{ value }}"
            ),
            accepted,
        );
    }
}

#[test]
fn negated_comparisons_require_refutation_not_failed_proof() {
    for (guarantee, accepted) in [
        ("!(result < 2)", true),
        ("!(2 > result)", true),
        ("!(result <= 1)", true),
        ("!(result > 4)", true),
        ("!(result >= 5)", true),
        ("!(result == 1)", true),
        ("!!(result >= 2)", true),
        ("!(result < 3)", false),
        ("!(result <= 3)", false),
        ("!(result > 3)", false),
        ("!(result >= 3)", false),
        ("!(result == 3)", false),
        ("!(result != 3)", false),
        ("!(result >= 2)", false),
    ] {
        check(
            &format!(
                "machine choose(value: u64 [2..=4]) -> u64
                 ensures {guarantee} {{ value }}"
            ),
            accepted,
        );
    }
    check(
        "machine choose(value: u64 [3..=3]) -> u64
         ensures !(result != 3) { value }",
        true,
    );
}

#[test]
fn overlapping_ranges_do_not_replace_return_identity() {
    for (returned, accepted) in [("value", true), ("other", false)] {
        for guarantee in ["result == value", "value == result"] {
            check(
                &format!(
                    "machine choose(value: u64 [2..=4], other: u64 [2..=4]) -> u64
                     ensures {guarantee} {{ {returned} }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn returned_arithmetic_reuses_immutable_bounds() {
    check(
        "machine choose(value: u64 [2..=4]) -> u64
         ensures result >= 3 && result < 6 { value + 1 }",
        true,
    );
    check(
        "machine choose(value: u64 [2..=4], ceiling: u64 [5..=7]) -> u64
         ensures result < ceiling { value }",
        true,
    );
}

#[test]
fn every_return_occurrence_and_live_state_must_establish_the_bound() {
    for (other_range, accepted) in [("2..=4", true), ("0..=4", false)] {
        check(
            &format!(
                "machine choose(value: u64 [2..=4], other: u64 [{other_range}], flag: bool) -> u64
                 ensures result >= 2 {{
                     transition flag {{ true -> value false -> other }}
                 }}"
            ),
            accepted,
        );
        check(
            &format!(
                "machine choose(value: u64 [{other_range}]) -> u64
                 ensures result >= 2 {{
                     transition {{ _ -> finish(value) }}
                     state finish(value: u64 [{other_range}]) -> u64 {{ value }}
                 }}"
            ),
            accepted,
        );
    }
}

#[test]
fn mutable_captures_cannot_replay_initializers_at_exit() {
    check(
        "machine choose(value: u64 [2..=4], replacement: u64 [0..=4]) -> u64
         ensures result >= 2 {
             let mut current: u64 = value;
             current = replacement;
             let saved: u64 = current;
             current = value;
             saved
         }",
        false,
    );
}

#[test]
fn selected_comparisons_and_arithmetic_supply_no_builtin_result_bounds() {
    for (declaration, returned, guarantee) in [
        (
            "operator >= u64::custom(left: u64, right: u64) -> bool;",
            "value",
            "result >= 2",
        ),
        (
            "operator < u64::custom(left: u64, right: u64) -> bool;",
            "value",
            "1 < result",
        ),
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;",
            "value + 1",
            "result >= 3",
        ),
        (
            "operator < u64::custom(left: u64, right: u64) -> bool;",
            "value",
            "!(result < 2)",
        ),
    ] {
        check(
            &format!(
                "{declaration}
                 machine choose(value: u64 [2..=4]) -> u64
                 ensures {guarantee} {{ {returned} }}"
            ),
            false,
        );
    }
}

#[test]
fn same_spelled_foreign_parameter_cannot_supply_return_bounds() {
    let mut program = parse_typed_trees(
        "machine choose(value: u64 [2..=4]) -> u64 ensures result >= 2 { value }
         machine other(value: u64 [2..=4]) -> u64 { value }",
    );
    let first = &program.machine_states(&program.machines()[0])[0];
    let StatementNode::Expression(returned) =
        program.statement_table.statements(first.statement_nodes)[0]
    else {
        panic!("direct return");
    };
    let second = &program.machine_states(&program.machines()[1])[0];
    let foreign = program.state_parameters(second)[0].symbol;
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(returned) else {
        panic!("parameter return");
    };
    path.symbol = foreign;
    path.head_symbol = foreign;
    let diagnostics = lower_typed_trees(program).expect_err("foreign return binder must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
        "uses `value`, which is not a declared local, parameter, field, or type in this state"
    )
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn result_carrier_spelling_cannot_authorize_a_nominal_type() {
    let mut program = parse_typed_trees(
        "data Nominal {}
         machine choose(value: u64 [2..=4]) -> u64 ensures result >= 2 { value }",
    );
    let reference = program.machine_states(&program.machines()[0])[0].return_type;
    let nominal = program.data_definitions()[0].symbol;
    program.type_reference_table.substitute_node(
        reference,
        typed_trees::types::TypeReferenceNode::Named {
            symbol: nominal,
            name: typed_trees::name::Identifier::generated_static("u64"),
        },
    );
    let diagnostics = lower_typed_trees(program).expect_err("nominal return carrier must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("binds a numeric value into the `Nominal` data return value")),
        "{diagnostics:#?}"
    );
}
