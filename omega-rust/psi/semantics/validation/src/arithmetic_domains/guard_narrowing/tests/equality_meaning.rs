use super::*;

fn field_guard_environments(declaration: &str, condition: &str) -> (ValueEnv, ValueEnv) {
    let program = arrival_program(&format!(
        "{declaration}
         data Limit {{ zero: u32 [0..=0]; }}
         machine value(input: u32, limit: Limit) -> u32 {{
             transition {condition} {{ true -> 1u32 false -> 0u32 }}
         }}"
    ));
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let StatementNode::Transition(transition) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("authored guard");
    };
    let base = ValueEnv::new();
    (
        guard_narrowed_env(&program, machine, Some(state), &transition.guard, &base),
        fall_through_narrowed_env(&program, machine, Some(state), &transition.guard, &base),
    )
}

#[test]
fn authored_field_equality_and_inequality_supply_no_primitive_guard_bounds() {
    for (operator, name) in [("==", "equal"), ("!=", "different")] {
        let declaration =
            format!("operator {operator} u32::{name}(left: u32, right: u32) -> bool;");
        for bound in ["limit.zero", "(limit.zero + 0)"] {
            for comparison in [
                format!("input {operator} {bound}"),
                format!("{bound} {operator} input"),
            ] {
                for condition in [
                    comparison.clone(),
                    format!("!({comparison})"),
                    format!("({comparison}) == true"),
                    format!("false == ({comparison})"),
                    format!("({comparison}) != false"),
                    format!("!(({comparison}) == false)"),
                ] {
                    let (selected, fallback) = field_guard_environments(&declaration, &condition);
                    assert_eq!(
                        selected.get("input"),
                        None,
                        "{condition}: the authored comparison supplies no primitive fact on its true arm"
                    );
                    assert_eq!(
                        fallback.get("input"),
                        None,
                        "{condition}: complementing an authored comparison supplies no primitive fact"
                    );
                }
            }
        }
    }
}

#[test]
fn builtin_field_equality_keeps_singleton_and_excluded_endpoint_bounds() {
    let nonzero = Interval {
        low: Some(1),
        high: Some(i64::from(u32::MAX)),
    };
    for bound in ["limit.zero", "(limit.zero + 0)"] {
        for comparison in [format!("input == {bound}"), format!("{bound} == input")] {
            for (condition, equal_on_selected) in [
                (comparison.clone(), true),
                (format!("!({comparison})"), false),
                (format!("!!({comparison})"), true),
                (format!("({comparison}) == true"), true),
                (format!("false == ({comparison})"), false),
                (format!("({comparison}) != false"), true),
                (format!("!(({comparison}) == false)"), true),
            ] {
                let (selected, fallback) = field_guard_environments("", &condition);
                let (equal, unequal) = if equal_on_selected {
                    (selected, fallback)
                } else {
                    (fallback, selected)
                };
                assert_eq!(
                    equal.get("input"),
                    Some(Interval::constant(0)),
                    "{condition}"
                );
                assert_eq!(unequal.get("input"), Some(nonzero), "{condition}");
            }
        }
    }
}

#[test]
fn builtin_field_inequality_false_arm_retains_equality_through_wrappers() {
    for (condition, equal_on_selected) in [
        ("input != limit.zero", false),
        ("limit.zero != input", false),
        ("!(input != limit.zero)", true),
        ("(input != limit.zero) == false", true),
        ("false == (limit.zero != input)", true),
        ("(input != limit.zero) != true", true),
    ] {
        let (selected, fallback) = field_guard_environments("", condition);
        let equal = if equal_on_selected {
            selected
        } else {
            fallback
        };
        assert_eq!(
            equal.get("input"),
            Some(Interval::constant(0)),
            "{condition}"
        );
    }
}

#[test]
fn authored_scalar_literal_requires_does_not_seed_builtin_equality() {
    for condition in ["input == 0", "0 == input"] {
        let program = arrival_program(&format!(
            "operator == u8::custom(left: u8, right: u8) -> bool;
             machine value(input: u8) -> u8 requires {condition} {{ input + 1 }}"
        ));
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        assert_eq!(
            requires_value_env(&program, machine, state).get("input"),
            None,
            "{condition}: declaration selection must precede Requires interval seeding"
        );
    }
}

#[test]
fn builtin_scalar_literal_requires_still_seeds_equality() {
    for condition in ["input == 0", "0 == input"] {
        let program = arrival_program(&format!(
            "machine value(input: u8) -> u8 requires {condition} {{ input + 1 }}"
        ));
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        assert_eq!(
            requires_value_env(&program, machine, state).get("input"),
            Some(Interval::constant(0)),
            "{condition}"
        );
        let result = crate::validate_program(&program);
        assert!(result.is_ok(), "{condition}: {result:?}");
    }
}
