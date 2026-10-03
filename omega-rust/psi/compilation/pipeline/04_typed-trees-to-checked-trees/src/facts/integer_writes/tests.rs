use crate::tests::front_end::checked_program;
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
    CheckedScalarExpression as Scalar,
};
use typed_trees::types::PrimitiveType;

fn integer(value: i64) -> Scalar {
    let text = value.to_string();
    Scalar::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_parts(
            value < 0,
            numerics::literals::IntegerRadix::Decimal,
            text.trim_start_matches('-'),
        )
        .unwrap()
        .with_landing(numerics::literals::IntegerLanding {
            landed_type: crate::values::landed_for_primitive(PrimitiveType::I64).unwrap(),
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
        }),
    }
}

fn fits_byte(value: Scalar) -> Boolean {
    Boolean::And {
        left: Box::new(Boolean::IntegerComparison {
            kind: Comparison::LessOrEqual,
            left: Box::new(integer(-128)),
            right: Box::new(value.clone()),
        }),
        right: Box::new(Boolean::IntegerComparison {
            kind: Comparison::LessOrEqual,
            left: Box::new(value),
            right: Box::new(integer(127)),
        }),
    }
}

#[test]
fn performed_integer_writes_keep_values_overwritten_before_return() {
    let checked = checked_program(
        "machine overwrite(destination: &mut i64) { destination = 128; destination = 0; }",
    );
    let writes = checked
        .facts
        .mutation
        .integer_writes
        .iter()
        .map(|(_, write)| write)
        .collect::<Vec<_>>();
    assert_eq!(writes.len(), 2);
    for (index, expected) in [128, 0].into_iter().enumerate() {
        assert_eq!(writes[index].statement_index, index);
        assert_eq!(writes[index].entry_value, Some(integer(expected)));
        assert!(writes[index].frame.is_complete());
    }
    assert_ne!(writes[0].value, writes[1].value);
}

#[test]
fn performed_integer_write_relations_use_captured_call_operands() {
    for (first, replacement, expected) in [(127, 128, true), (128, 0, false)] {
        let checked = checked_program(&format!(
            "machine change(value: &mut i64) -> i64 {{ value = {replacement}; 0 }}
             machine store(destination: &mut i64, value: i64, unused: i64) {{ destination = value; }}
             machine run(destination: &mut i64) {{ let mut value: i64 = {first}; store(destination, value, change(&mut value)); }}"
        ));
        let target = checked
            .machines()
            .iter()
            .find(|machine| checked.symbols.name(machine.symbol) == "store")
            .unwrap()
            .symbol;
        let state = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .find_map(|(_, state)| {
                (checked.symbols.name(state.machine_symbol) == "run").then_some(state)
            })
            .unwrap();
        let call = checked
            .facts
            .flow
            .control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .find(|call| {
                crate::checks::contracts::call_values::target_machine(&checked, call)
                    == Some(target)
            })
            .unwrap();
        let write = checked
            .facts
            .mutation
            .integer_writes
            .iter()
            .find_map(|(_, write)| (write.machine == target).then_some(write))
            .unwrap();
        let value = write
            .entry_value
            .clone()
            .expect("the unchanged formal retains entry provenance");
        let contexts = checked
            .facts
            .flow
            .semantic_constraint_contexts(call.entry_constraints)
            .collect::<Vec<_>>();
        assert_eq!(
            crate::checks::contracts::call_values::proves_entry_predicate(
                &checked,
                &checked.facts,
                state,
                call,
                &contexts,
                target,
                fits_byte(value)
            ),
            expected,
            "{first} captured before replacing the local with {replacement}"
        );
    }
}

#[test]
fn performed_integer_writes_do_not_replay_a_mutated_input() {
    let checked = checked_program(
        "machine store(destination: &mut i64, value: &mut i64) { value = 128; destination = value; }",
    );
    let writes = checked
        .facts
        .mutation
        .integer_writes
        .iter()
        .map(|(_, write)| write)
        .collect::<Vec<_>>();
    assert_eq!(writes.len(), 2);
    assert!(
        writes[1].entry_value.is_none(),
        "new contents are not the invocation input"
    );
}

#[test]
fn performed_integer_writes_keep_guards_and_abrupt_completion() {
    let checked = checked_program(
        "machine store(destination: &mut i64, value: i64, write: bool) crashes Trap {
             transition write { true -> update(destination, value) _ -> done() }
             state update(destination: &mut i64, value: i64) {
                 destination = value;
                 crash Trap;
             }
             state done() {}
         }",
    );
    let writes = checked
        .facts
        .mutation
        .integer_writes
        .iter()
        .map(|(_, write)| write)
        .collect::<Vec<_>>();
    assert_eq!(
        writes.len(),
        1,
        "crashing after the assignment does not remove it"
    );
    struct Selector(bool);
    impl crate::values::ScalarValueSource for Selector {
        fn binding(&mut self, position: usize) -> Option<facts::ScalarValue> {
            (position == 1).then_some(facts::ScalarValue::Boolean(self.0))
        }
    }
    let condition = Scalar::Boolean(Box::new(writes[0].entry_condition.clone().unwrap()));
    for selected in [false, true] {
        assert_eq!(
            crate::values::evaluate_checked_scalar(&condition, &mut Selector(selected)),
            Some(facts::ScalarValue::Boolean(selected))
        );
    }
    assert_eq!(
        writes[0].entry_value,
        Some(Scalar::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I64
        })
    );
}
