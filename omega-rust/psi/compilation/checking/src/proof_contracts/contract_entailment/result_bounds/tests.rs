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

#[test]
fn widened_return_rejects_narrowing_signedness_and_policy_erasure() {
    for (source, target) in [
        ("u16", "u8"),
        ("i8", "u64"),
        ("u8", "u8"),
        ("u8 in Wrapping", "u64"),
        ("u8", "u64 in Wrapping"),
    ] {
        let program = typed_program(&format!(
            "machine read(values: &[{source}], position: u64) -> {target}
             requires position < values.len {{ values[position] as {target} }}"
        ));
        assert!(
            super::infer(&program, &program.machines()[0]).is_none(),
            "{source} -> {target}"
        );
    }
}

#[test]
fn widening_does_not_hide_selected_index_or_effectful_coordinates() {
    let source = r#"
        data Meaning {}
        boundary operator [] Meaning::index(values: &[u8], position: u64) -> u8;
        machine read(values: &[u8], position: u64) -> u64
            requires position < values.len
            ensures 0 <= result && result < 256
        { values[position] as u64 }
    "#;
    let program = typed_program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .unwrap();
    assert!(super::infer(&program, machine).is_none());
    let program = typed_program(
        "machine choose() -> u64 { 0 }
         machine read(values: &[u8; 2]) -> u64 ensures 0 <= result && result < 256
         { values[choose()] as u64 }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .unwrap();
    assert!(super::infer(&program, machine).is_none());
}

#[test]
fn unrelated_call_result_does_not_hide_literal_return_bounds() {
    let source = r#"
        data U64Result [copy] { v: u64; }
        data Module { base_address: u64; }
        machine Module::get_end_address(&self) -> U64Result { U64Result { v: 12 } }
        pub machine Module::contains_address(&self, address: u64) -> u64
            ensures 0 <= result && result < 2
        {
            transition { _ -> check(address) }
            state check(&self, address: u64) -> u64 {
                let _end: U64Result = self.get_end_address();
                transition address >= self.base_address && address <= _end.v {
                    true -> (1)
                    false -> (0)
                }
            }
        }
    "#;
    assert_eq!(outcome(source, "Module::contains_address"), (1, 0));
}

#[test]
fn unrelated_effects_preserve_only_independent_return_bounds() {
    let source = r#"
        data Counter { value: u64; }
        machine Counter::clear(&mut self) -> bool { self.value = 0; false }
        machine Counter::read(&mut self) -> u64 ensures 0 <= result && result < 2 {
            self.value = 55;
            self.clear();
            transition self.clear() { true -> (1) false -> (0) }
        }
    "#;
    assert_eq!(outcome(source, "Counter::read"), (1, 0));
    assert_eq!(
        outcome(
            &source.replace("false -> (0)", "false -> (2)"),
            "Counter::read"
        )
        .0,
        0
    );

    let source = r#"
        machine effect() -> u64 { 100 }
        machine read() -> u64 ensures 0 <= result && result < 2 {
            transition { _ -> next(effect()) }
            state next(value: u64) -> u64 { 1 }
        }
    "#;
    assert_eq!(outcome(source, "read"), (1, 0));
    assert_eq!(
        outcome(
            &source.replace(
                "state next(value: u64) -> u64 { 1 }",
                "state next(value: u64) -> u64 { value }"
            ),
            "read"
        )
        .0,
        0,
        "internal arguments introduce no result-bound assumptions"
    );
}

#[test]
fn effects_invalidate_prior_guard_observations_and_captured_reads() {
    let source = r#"
        data Table { values: [u64; 2]; }
        machine Table::clear(&mut self) -> bool { self.values[0] = 0; false }
        machine Table::read(&mut self) -> u64 ensures 1 <= result {
            INTERVENING
            transition self.values[0] < 1 { true -> (1) false -> (self.values[0]) }
        }
    "#;
    assert_eq!(
        outcome(&source.replace("INTERVENING", ""), "Table::read"),
        (1, 0)
    );
    for intervening in [
        "self.clear();",
        "self.values[0] = 0;",
        "let cleared: bool = self.clear();",
    ] {
        let mut program = typed_program(&source.replace("INTERVENING", intervening));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Table::read")
            .unwrap()
            .clone();
        let state = program.machine_states(&machine)[0].clone();
        // Source dispatches must be complete. Move the already typed effect
        // between its two arms to exercise the proof's ordered invalidation.
        let statements = program
            .statement_table
            .statements_mut(state.statement_nodes);
        let first_arm = statements
            .iter()
            .position(|statement| {
                matches!(
                    statement,
                    typed_trees::statement::StatementNode::Transition(_)
                )
            })
            .unwrap();
        assert!(first_arm > 0);
        statements[..=first_arm].rotate_left(first_arm);
        let bounds =
            super::infer(&program, &machine).expect("explicit fallback still returns a carrier");
        assert_eq!(bounds.minimum.to_u64(), Some(0), "{intervening}");
    }
    let captured = r#"
        data Table { values: [u64; 2]; }
        machine Table::clear(&mut self) -> bool { self.values[0] = 0; false }
        machine Table::read(&mut self) -> u64 ensures 1 <= result {
            let selected: bool = self.values[0] >= 1;
            self.clear();
            transition selected { true -> (self.values[0]) false -> (1) }
        }
    "#;
    assert_eq!(outcome(captured, "Table::read").0, 0);
}

#[test]
fn complementary_guards_across_mutation_do_not_establish_exit_coverage() {
    use typed_trees::expression::{ExpressionNode, TableBinaryExpression};
    use typed_trees::statement::{StatementNode, TransitionGuardNode};
    let mut program = typed_program(
        "machine read(mut flag: bool) -> u64 ensures result < 2 {
            flag = true;
            transition flag { true -> (1) false -> (0) }
        }",
    );
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let statements = program.statement_table.statements(state.statement_nodes);
    let first_arm = statements
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))
        .unwrap();
    assert_eq!(first_arm, 1, "only the authored mutation precedes dispatch");
    let StatementNode::Transition(first) = &statements[first_arm] else {
        unreachable!()
    };
    let TransitionGuardNode::When(guard) = first.guard else {
        panic!("conditional arm")
    };
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(guard) else {
        panic!("Boolean comparison")
    };
    let left = comparison.left;
    let ExpressionNode::Name(subject) = program.expression_table.expression(left) else {
        panic!("guard reads the current flag parameter")
    };
    assert_eq!(subject.symbol, program.state_parameters(&state)[0].symbol);
    let false_value = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let complementary =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                operator: typed_trees::expression::BinaryOperator::Equal,
                left,
                right: false_value,
            }));
    let statements = program
        .statement_table
        .statements_mut(state.statement_nodes);
    statements[..=first_arm].rotate_left(first_arm);
    let StatementNode::Transition(last) = statements.last_mut().unwrap() else {
        panic!("fallback arm")
    };
    last.guard = TransitionGuardNode::When(complementary);
    assert!(
        super::super::entailment_covers_all_exits(&program, &machine),
        "syntactic complements alone appear exhaustive"
    );
    assert!(
        super::infer(&program, &machine).is_none(),
        "the changed predicate can miss both value arms"
    );
}

fn cyclic_state_parameter_result_source() -> &'static str {
    r#"
        data Producer { value: u64; }
        machine Producer::copy_modules(&mut self, output: &mut [u8; 8]) -> u64
        ensures 0 <= result && result < 9;
        {
            transition { _ -> copy(output, 0) }
            state copy(&mut self, output: &mut [u8; 8], position: u64) -> u64
            requires 0 <= position && position < 9;
            {
                transition position < self.value && position < 8 {
                    true -> store(output, position)
                    false -> (position)
                    _ -> (position)
                }
            }
            state store(&mut self, output: &mut [u8; 8], position: u64) -> u64
            requires 0 <= position && position < 8;
            {
                output[position] = 1;
                transition { _ -> copy(output, position + 1) }
            }
        }
    "#
}

#[test]
fn immutable_state_arrival_bounds_prove_cyclic_normal_returns() {
    assert_eq!(
        outcome(
            cyclic_state_parameter_result_source(),
            "Producer::copy_modules"
        ),
        (1, 0)
    );
}

#[test]
fn state_arrival_bounds_do_not_survive_mutable_reassignment() {
    let source = cyclic_state_parameter_result_source()
        .replace(
            "state copy(&mut self, output: &mut [u8; 8], position: u64)",
            "state copy(&mut self, output: &mut [u8; 8], mut position: u64)",
        )
        .replace(
            "transition position < self.value && position < 8",
            "position = 9; transition position < self.value && position < 8",
        );
    assert_eq!(outcome(&source, "Producer::copy_modules").0, 0);
}

#[test]
fn state_arrival_bounds_do_not_license_other_normal_exits() {
    for sibling in ["9", "position"] {
        let source = cyclic_state_parameter_result_source()
            .replace(
                "transition { _ -> copy(output, 0) }",
                "transition self.value == 0 { true -> copy(output, 0) false -> bad(self.value) }",
            )
            .replace(
                "state store(&mut self",
                &format!("state bad(&mut self, position: u64) -> u64 {{ {sibling} }} state store(&mut self"),
            );
        assert_eq!(outcome(&source, "Producer::copy_modules").0, 0, "{sibling}");
    }
}

#[test]
fn state_arrival_bounds_keep_open_full_width_carrier_endpoints() {
    let source = r#"
        machine forward(value: u64) -> u64 requires value >= 1
        ensures 1 <= result && result <= 18446744073709551615
        {
            transition { _ -> finish(value) }
            state finish(position: u64) -> u64 requires position >= 1 { position }
        }
    "#;
    assert_eq!(outcome(source, "forward"), (1, 0));
    assert_eq!(
        outcome(
            &source.replace(
                "result <= 18446744073709551615",
                "result < 18446744073709551615"
            ),
            "forward"
        )
        .0,
        0
    );
}
