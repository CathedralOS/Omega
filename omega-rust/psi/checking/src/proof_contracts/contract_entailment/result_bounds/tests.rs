use crate::front_end::typed_program;

fn outcome(source: &str, name: &str) -> (usize, usize) {
    let program = typed_program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap();
    (
        crate::proven_machine_contract_expressions(&program, machine.symbol).len(),
        crate::collect_contract_entailment_stand_downs(&program)
            .iter()
            .filter(|stand_down| stand_down.machine_symbol == machine.symbol)
            .count(),
    )
}

#[test]
fn guarded_array_read_proves_full_width_result_without_reading_initializer() {
    let source = r#"
        machine scale(position: u64) -> u64
            requires 0 <= position && position < 2
            ensures 1 <= result && result <= 18446744073709551615
        {
            let values: [u64; 2] = [0, 1000];
            transition values[position] >= 1 {
                true -> (values[position])
                false -> (1)
            }
        }
    "#;
    assert_eq!(outcome(source, "scale"), (1, 0));
    assert_eq!(
        outcome(
            &source.replace(
                "result <= 18446744073709551615",
                "result < 18446744073709551615"
            ),
            "scale"
        )
        .0,
        0
    );
}

#[test]
fn all_internal_states_contribute_return_bounds() {
    let source = r#"
        machine place(value: u64) -> u64 ensures 0 <= result && result <= 6 {
            transition value >= 1000 {
                true -> (6)
                false -> lower(value)
            }
            state lower(value: u64) -> u64 {
                transition value >= 10 {
                    true -> (3)
                    false -> bottom(value)
                }
            }
            state bottom(value: u64) -> u64 { 0 }
        }
    "#;
    assert_eq!(outcome(source, "place"), (1, 0));
    assert_eq!(
        outcome(
            &source.replace(
                "state bottom(value: u64) -> u64 { 0 }",
                "state bottom(value: u64) -> u64 { 7 }"
            ),
            "place"
        )
        .0,
        0
    );
    let foreign = source.replace("false -> bottom(value)", "false -> foreign(value)");
    let foreign = format!("machine foreign(value: u64) -> u64 {{ value }}\n{foreign}");
    assert_eq!(outcome(&foreign, "place").0, 0);
}

#[test]
fn changed_capture_and_different_read_keep_guarantee_open() {
    let source = r#"
        data Table { values: [u64; 2]; other: [u64; 2]; }
        machine Table::read(&mut self, mut position: u64) -> u64
            requires 0 <= position && position < 2
            ensures 1 <= result
        {
            let selected: bool = self.values[position] >= 1;
            INTERVENING
            transition selected {
                true -> (RETURNED)
                false -> (1)
            }
        }
    "#;
    assert_eq!(
        outcome(
            &source
                .replace("INTERVENING", "")
                .replace("RETURNED", "self.values[position]"),
            "Table::read"
        ),
        (1, 0)
    );
    for intervening in ["position = 1;", "self.values[0] = 0;"] {
        assert_eq!(
            outcome(
                &source
                    .replace("INTERVENING", intervening)
                    .replace("RETURNED", "self.values[position]"),
                "Table::read"
            )
            .0,
            0
        );
    }
    assert_eq!(
        outcome(
            &source
                .replace("INTERVENING", "")
                .replace("RETURNED", "self.other[position]"),
            "Table::read"
        )
        .0,
        0
    );
}

#[test]
fn false_arm_does_not_inherit_positive_read_bound() {
    let source = r#"
        machine read(values: &[u64; 2], position: u64) -> u64
            requires 0 <= position && position < 2
            ensures 1 <= result
        {
            transition values[position] >= 1 {
                true -> (1)
                false -> (values[position])
            }
        }
    "#;
    assert_eq!(outcome(source, "read").0, 0);
    let inverse = source.replace("values[position] >= 1", "values[position] < 1");
    assert_eq!(outcome(&inverse, "read"), (1, 0));
}

#[test]
fn readonly_call_selectors_and_writing_guard_operands_are_not_repeated_reads() {
    let source = r#"
        machine choose() -> u64 { 0 }
        machine read(values: &[u64; 2]) -> u64 ensures 1 <= result {
            transition values[choose()] >= 1 {
                true -> (values[choose()])
                false -> (1)
            }
        }
    "#;
    assert_eq!(outcome(source, "read").0, 0);
    let source = r#"
        data Table { values: [u64; 2]; }
        machine Table::clear(&mut self) -> bool { self.values[0] = 0; true }
        machine Table::read(&mut self) -> u64 ensures 1 <= result {
            transition self.values[0] >= 1 && self.clear() {
                true -> (self.values[0])
                false -> (1)
            }
        }
    "#;
    assert_eq!(outcome(source, "Table::read").0, 0);
}

#[test]
fn selected_index_and_comparison_meanings_do_not_supply_builtin_bounds() {
    let source = r#"
        SELECTED
        machine read(values: &[u64; 2], position: u64) -> u64
            requires 0 <= position && position < 2
            ensures 1 <= result
        {
            let selected: bool = values[position] >= 1;
            transition selected {
                true -> (values[position])
                false -> (1)
            }
        }
    "#;
    for selected in [
        "boundary operator [] Meaning::index(values: &[u64; 2], position: u64) -> u64;",
        "boundary operator >= Meaning::compare(left: u64, right: u64) -> bool;",
    ] {
        assert_eq!(outcome(&source.replace("SELECTED", selected), "read").0, 0);
    }
}

#[test]
fn missing_value_paths_and_signed_negative_values_are_not_certified() {
    let mut program = typed_program(
        "machine partial(flag: bool) -> u64 ensures result <= 6 { transition { flag -> 1 _ -> 0 } }",
    );
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let last = program
        .statement_table
        .statements_mut(state.statement_nodes)
        .last_mut()
        .unwrap();
    let typed_trees::statement::StatementNode::Transition(transition) = last else {
        panic!("fallback transition");
    };
    transition.guard = typed_trees::statement::TransitionGuardNode::When(
        typed_trees::expression::ExpressionHandle::invalid(),
    );
    assert!(super::infer(&program, &machine).is_none());
    let program = typed_program("machine empty() -> u64 ensures result >= 0 {}");
    assert!(super::infer(&program, &program.machines()[0]).is_none());
    assert_eq!(
        outcome(
            "machine negative() -> i64 ensures result >= 0 { -1 }",
            "negative"
        )
        .0,
        0
    );
}

#[test]
fn result_bounds_do_not_substitute_authored_names_or_callee_guarantees() {
    let program = typed_program("machine value(result: u64) -> u64 ensures result <= 6 { 0 }");
    let machine = &program.machines()[0];
    let bounds = super::infer(&program, machine).expect("literal returned value");
    for contract in program.machine_contracts(machine) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let typed_trees::domain::ProofFact::Expression(expression) = fact {
                assert!(!super::proves(&program, machine, *expression, &bounds));
            }
        }
    }
    let program = typed_program(
        r#"
        machine other() -> u64 ensures result <= 6 { 0 }
        machine value() -> u64 ensures other() <= 6 { 0 }
    "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let bounds = super::infer(&program, machine).expect("literal returned value");
    for contract in program.machine_contracts(machine) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let typed_trees::domain::ProofFact::Expression(expression) = fact {
                assert!(!super::proves(&program, machine, *expression, &bounds));
            }
        }
    }
}
