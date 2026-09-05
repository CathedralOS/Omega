use super::*;
use typed_trees::expression::ExpressionNode;

fn proof_rejects(program: &typed_trees::TypedTrees) {
    let plan = proof::obligations::build_proof_plan(program);
    let diagnostics = proof::checker::check_proof_plan(&plan)
        .expect_err("a missing or invalidated arrival premise cannot prove the return");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove return value")),
        "{diagnostics:#?}"
    );
}

#[test]
fn guarded_arrival_requirement_proves_bounded_increment() {
    let source = r#"
        data Main {}
        machine Main::main(&mut self, value: u32 [0..=4]) -> u32 [0..=4] {
            transition value < 4 {
                true -> append(value)
                false -> (value)
            }
            state append(&mut self, value: u32 [0..=4]) -> u32 [0..=4]
            requires value < 4
            { value + 1 }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("the checked arrival requirement bounds the return");
}

#[test]
fn machine_entry_requirement_refolds_a_bounded_return() {
    let source = r#"
        machine increment(value: u32 [0..=4]) -> u32 [0..=4]
        requires value < 4
        { value + 1 }
    "#;
    lower_typed_trees(parse_typed_trees(source)).expect("entry-scoped requirement");
}

#[test]
fn absent_arrival_premise_keeps_the_declared_range() {
    let source = r#"
        machine increment(value: u32 [0..=4]) -> u32 [0..=4] { value + 1 }
    "#;
    proof_rejects(&parse_typed_trees(source));
    assert!(lower_typed_trees(parse_typed_trees(source)).is_err());
}

#[test]
fn a_guard_does_not_discharge_a_different_delivered_value() {
    let source = r#"
        data Main {}
        machine Main::main(&mut self, value: u32 [0..=4]) -> u32 [0..=4] {
            transition value < 4 {
                true -> append(4)
                false -> (value)
            }
            state append(&mut self, value: u32 [0..=4]) -> u32 [0..=4]
            requires value < 4
            { value + 1 }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("proving a body under requires must not authorize an invalid arrival");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call append")),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_sibling_state_cannot_supply_the_return_premise() {
    let source = r#"
        data Main {}
        machine Main::main(&mut self, value: u32 [0..=4]) -> u32 [0..=4] {
            transition { _ -> plain(value) }
            state narrow(value: u32 [0..=4]) -> u32 [0..=4]
            requires value < 4
            { value }
            state plain(value: u32 [0..=4]) -> u32 [0..=4] { value + 1 }
        }
    "#;
    proof_rejects(&parse_typed_trees(source));
}

#[test]
fn machine_requirement_does_not_leak_into_a_named_state() {
    let source = r#"
        data Main {}
        machine Main::main(&mut self, value: u32 [0..=4]) -> u32 [0..=4]
        requires value < 4
        {
            transition { _ -> plain(4) }
            state plain(value: u32 [0..=4]) -> u32 [0..=4] { value + 1 }
        }
    "#;
    proof_rejects(&parse_typed_trees(source));
}

#[test]
fn same_spelling_with_a_different_symbol_does_not_refine_the_return() {
    let mut program = parse_typed_trees(
        r#"
        machine increment(value: u32 [0..=4]) -> u32 [0..=4]
        requires value < 4
        { value + 1 }
        machine other(value: u32 [0..=4]) -> u32 { value }
    "#,
    );
    let other = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "other")
        .expect("other machine");
    let other_state = &program.machine_states(other)[0];
    let other_symbol = program.state_parameters(other_state)[0].symbol;
    let condition = program
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Binary(binary)
                if binary.operator == typed_trees::expression::BinaryOperator::Less =>
            {
                Some(binary.left)
            }
            _ => None,
        })
        .expect("arrival comparison");
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(condition) else {
        panic!("comparison operand is a named parameter");
    };
    path.symbol = other_symbol;
    path.head_symbol = other_symbol;
    proof_rejects(&program);
}

#[test]
fn overlapping_stores_and_calls_retire_the_arrival_premise() {
    for body in [
        "value = 4; value + 1",
        "let alias: &mut u32 [0..=4] = &mut value; alias = 4; value + 1",
        "overwrite(&mut value); value + 1",
        "overwrite_and_one(&mut value) + value",
    ] {
        let source = format!(
            r#"
            machine overwrite(value: &mut u32 [0..=4]) {{ value = 4; }}
            machine overwrite_and_one(value: &mut u32 [0..=4]) -> u32 [1..=1] {{
                value = 4;
                1
            }}
            machine increment(value: &mut u32 [0..=4]) -> u32 [0..=4]
            requires value < 4
            {{ {body} }}
        "#
        );
        proof_rejects(&parse_typed_trees(&source));
    }
}

#[test]
fn disjoint_writes_and_pure_calls_preserve_the_arrival_premise() {
    for body in ["other = 7; value + 1", "observe(); value + 1"] {
        let source = format!(
            r#"
            machine observe() {{}}
            machine increment(value: &mut u32 [0..=4], other: &mut u32) -> u32 [0..=4]
            requires value < 4
            {{ {body} }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source)).expect("disjoint prefix preserves arrival");
    }
}

#[test]
fn an_unknown_call_frame_cannot_preserve_an_arrival_premise() {
    let mut program = parse_typed_trees(
        r#"
        data Borrowed { value: &mut u32 [0..=4]; }
        machine observe(borrowed: Borrowed) {}
        machine increment(value: &mut u32 [0..=4]) -> u32 [0..=4]
        requires value < 4
        { observe(Borrowed { value: &mut value }); value + 1 }
    "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "increment")
        .expect("increment");
    let statements = program.machine_states(machine)[0].statement_nodes;
    for statement in program.statement_table.statements_mut(statements) {
        if let typed_trees::statement::StatementNode::Call(call) = statement {
            call.target_symbol = symbols::SymbolHandle::invalid();
            call.target = typed_trees::name::Identifier::generated("unknown");
        }
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "increment")
        .expect("increment");
    let call = program
        .statement_table
        .statements(statements)
        .iter()
        .find_map(|statement| {
            if let typed_trees::statement::StatementNode::Call(call) = statement {
                Some(call)
            } else {
                None
            }
        })
        .expect("unresolved aggregate-argument call");
    let frames = validation::CallFrameResolver::new(&program).expect("frame resolver");
    assert!(
        frames.may_write_paths(machine, call).is_none(),
        "an unresolved aggregate carrying a mutable reference must have an opaque frame"
    );
    proof_rejects(&program);
}

#[test]
fn unresolved_no_argument_call_preserves_an_unpassed_parameter() {
    let mut program = parse_typed_trees(
        r#"
        machine observe() {}
        machine increment(value: u32 [0..=4]) -> u32 [0..=4]
        requires value < 4
        { observe(); value + 1 }
    "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "increment")
        .expect("increment");
    let statements = program.machine_states(machine)[0].statement_nodes;
    for statement in program.statement_table.statements_mut(statements) {
        if let typed_trees::statement::StatementNode::Call(call) = statement {
            call.target_symbol = symbols::SymbolHandle::invalid();
            call.target = typed_trees::name::Identifier::generated("unknown");
        }
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "increment")
        .expect("increment");
    let call = program
        .statement_table
        .statements(statements)
        .iter()
        .find_map(|statement| {
            if let typed_trees::statement::StatementNode::Call(call) = statement {
                Some(call)
            } else {
                None
            }
        })
        .expect("unresolved no-argument call");
    let frames = validation::CallFrameResolver::new(&program).expect("frame resolver");
    assert_eq!(
        frames.may_write_paths(machine, call),
        Some(vec!["self".to_owned()])
    );
    let plan = proof::obligations::build_proof_plan(&program);
    proof::checker::check_proof_plan(&plan)
        .expect("the conservative receiver frame cannot modify the unpassed parameter");
}

#[test]
fn an_out_of_range_literal_remains_rejected() {
    proof_rejects(&parse_typed_trees(
        r#"
        machine answer() -> i32 [1..=50] { 100 }
    "#,
    ));
}
