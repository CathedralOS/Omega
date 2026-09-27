use super::{
    StatementNode, ValueEnvironment, fall_through_narrowed_environment, guard_narrowed_environment,
};
fn field_guard_environments(
    declaration: &str,
    condition: &str,
) -> (ValueEnvironment, ValueEnvironment) {
    let program = crate::front_end::typed_program(&format!(
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
    let base = ValueEnvironment::new();
    (
        guard_narrowed_environment(&program, machine, Some(state), &transition.guard, &base),
        fall_through_narrowed_environment(&program, machine, Some(state), &transition.guard, &base),
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
                    assert!(
                        selected.ordered_values.is_empty(),
                        "{condition}: authored equality supplies no builtin order"
                    );
                    assert!(
                        fallback.ordered_values.is_empty(),
                        "{condition}: authored inequality supplies no complementary builtin order"
                    );
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
