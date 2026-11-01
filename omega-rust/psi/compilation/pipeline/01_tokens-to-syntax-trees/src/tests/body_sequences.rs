use crate::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::item::Item;
use syntax_trees::statement::StatementNode;
use syntax_trees::statement::TransitionGuardNode;

#[test]
fn final_values_are_completion_edges_but_semicolon_expressions_are_not() {
    for body in [
        "callee()",
        "(callee())",
        "callee() + 1",
        "42 machine helper() {}",
    ] {
        let source = format!("machine run() -> u64 {{ {body} }}");
        let trees = super::parse_source(&source, "final value");
        let machine = super::root_machine(&trees, "machine");
        let state = super::entry_state(&trees, machine);
        let statements = trees.items.statements(state.statements);
        let StatementNode::Transition(completion) = trees.statements.statement(statements[0])
        else {
            panic!("final value must carry its completion edge");
        };
        assert!(matches!(completion.guard, TransitionGuardNode::Always));
        assert!(!completion.continuation.is_valid());
        assert!(completion.ends_dispatch);
        assert!(matches!(
            trees.statements.transition_target(completion.target),
            syntax_trees::statement::TransitionTargetNode::Value(_)
        ));
    }
    let trees = super::parse_source("machine run() { callee(); 42; }", "ordinary statements");
    let machine = super::root_machine(&trees, "machine");
    let state = super::entry_state(&trees, machine);
    let statements = trees.items.statements(state.statements);
    assert!(matches!(
        trees.statements.statement(statements[0]),
        StatementNode::Call(_)
    ));
    assert!(matches!(
        trees.statements.statement(statements[1]),
        StatementNode::Expression(_)
    ));
}

#[test]
fn nested_machine_declarations_and_copies_retain_direct_child_cohorts() {
    let source =
        "machine outer() -> u64 { 42 machine first() { machine inner() {} } machine second() {} }";
    let trees = super::parse_source(source, "nested declarations after a value completion");
    let outer = super::root_machine(&trees, "outer machine");
    assert_eq!(trees.root_items().count(), 1);
    assert_eq!(outer.nested_machines.len(), 2);
    let children = outer
        .nested_machines
        .handles()
        .map(|handle| {
            let Item::Machine(child) = trees.items.item(handle) else {
                panic!("machine child");
            };
            (child.name.as_str(&trees), child.nested_machines.len())
        })
        .collect::<Vec<_>>();
    assert_eq!(children, [("first", 1), ("second", 0)]);

    let mut copied = syntax_trees::SyntaxTrees::default();
    let Item::Machine(outer_copy) = copied.copy_item_from(&trees, &Item::Machine(outer.clone()))
    else {
        panic!("machine copy");
    };
    let children = outer_copy
        .nested_machines
        .handles()
        .map(|handle| {
            let Item::Machine(child) = copied.items.item(handle) else {
                panic!("copied machine child");
            };
            (child.name.as_str(&copied), child.nested_machines.len())
        })
        .collect::<Vec<_>>();
    assert_eq!(children, [("first", 1), ("second", 0)]);
}

#[test]
fn nested_machine_layout_rejects_resumed_statements_and_package_exports() {
    for (source, expected) in [
        (
            "machine outer() { machine helper() {} helper(); }",
            "executable statements cannot follow",
        ),
        (
            "machine outer() { public machine helper() {} }",
            "cannot be package-exported",
        ),
    ] {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let error = parse_syntax_trees(&tokens).unwrap_err();
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

#[test]
fn entry_and_nested_bodies_preserve_expanded_statement_order_and_boundaries() {
    let body =
        "let first: u64 = 1; asm where clobbers none { lfence; sfence } transition { _ -> done() }";
    for source in [
        format!("machine process() {{ {body} machine done() {{}} }}"),
        format!("machine process() {{ machine begin() {{ {body} }} machine done() {{}} }}"),
    ] {
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let trees = parse_syntax_trees(&tokens).unwrap();
        let Item::Machine(machine) = trees.root_items().next().unwrap() else {
            panic!("machine root")
        };
        let machine = if trees
            .items
            .state(trees.items.state_handles(machine.states)[0])
            .statements
            .is_empty()
        {
            machine
                .nested_machines
                .handles()
                .find_map(|handle| match trees.items.item(handle) {
                    Item::Machine(child) if child.name.as_str(&trees) == "begin" => Some(child),
                    _ => None,
                })
                .expect("nested begin")
        } else {
            machine
        };
        let states = trees.items.state_handles(machine.states);
        assert_eq!(states.len(), 1);
        let statements = super::inline_assembly::instruction_statements(
            &trees,
            trees
                .items
                .statements(trees.items.state(states[0]).statements),
        );
        assert_eq!(statements.len(), 4);
        assert!(matches!(
            trees.statements.statement(statements[0]),
            StatementNode::LocalData(_)
        ));
        for (handle, expected) in [(statements[1], "asm#lfence"), (statements[2], "asm#sfence")] {
            assert!(
                matches!(trees.statements.statement(handle), StatementNode::Call(call) if call.target.as_str(&trees) == expected)
            );
        }
        assert!(matches!(
            trees.statements.statement(statements[3]),
            StatementNode::Transition(_)
        ));
    }
}

#[test]
fn every_machine_body_rejects_bare_arrows() {
    for (source, expected) in [
        ("machine process() { -> done() }", "machine entry bodies"),
        (
            "machine process() { machine begin() { -> done() } }",
            "machine entry bodies",
        ),
    ] {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let error = parse_syntax_trees(&tokens).unwrap_err();
        assert!(error.message.starts_with(expected), "{}", error.message);
        assert_eq!(error.source_span.span.start, source.find("->").unwrap());
    }
}

#[test]
fn named_data_binding_captures_initializer_once_and_retains_pattern_metadata() {
    let trees = super::parse_source(
        "data Pair { x: u64; y: u64; } machine read() -> u64 { let Pair { x as answer, y as _ } = produce(); answer }",
        "named data binding of a computed initializer",
    );
    let machine = super::root_machine(&trees, "read machine");
    let state = super::entry_state(&trees, machine);
    let statements = trees.items.statements(state.statements);
    let StatementNode::LocalData(capture) = trees.statements.statement(statements[0]) else {
        panic!("one initializer capture");
    };
    assert_eq!(
        capture.role,
        language_core::LocalBindingRole::DispatchSubject
    );
    assert!(matches!(
        trees.expressions.expression(capture.initial_value),
        syntax_trees::expression::ExpressionNode::Call(_)
    ));
    let pattern = capture
        .pattern
        .expect("explicit component roster on its capture");
    assert_eq!(trees.display_expression_name(pattern.head), "Pair");
    let fields = trees.expressions.match_fields(pattern.fields);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].member.as_str(&trees), "x");
    assert_eq!(fields[1].member.as_str(&trees), "y");
    assert!(!capture.name.as_str(&trees).contains("#x#"));
    let StatementNode::LocalData(answer) = trees.statements.statement(statements[1]) else {
        panic!("ordinary field binding");
    };
    let syntax_trees::expression::ExpressionNode::Member(member) =
        trees.expressions.expression(answer.initial_value)
    else {
        panic!("captured field projection");
    };
    assert!(matches!(
        trees.expressions.expression(member.receiver),
        syntax_trees::expression::ExpressionNode::Name(_)
    ));

    let mut copied = syntax_trees::SyntaxTrees::default();
    let Item::Machine(copied_machine) =
        copied.copy_item_from(&trees, &Item::Machine(machine.clone()))
    else {
        panic!("copied machine");
    };
    let copied_state = super::entry_state(&copied, &copied_machine);
    let copied_statements = copied.items.statements(copied_state.statements);
    let StatementNode::LocalData(binding) = copied.statements.statement(copied_statements[0])
    else {
        panic!("copied metadata");
    };
    let pattern = binding.pattern.expect("copy retains selectors");
    assert_eq!(copied.expressions.match_fields(pattern.fields).len(), 2);
    assert_eq!(copied.display_expression_name(pattern.head), "Pair");
}

#[test]
fn data_binding_rejects_duplicate_binding_names_at_the_pattern_site() {
    let tokens = Lexer::new("machine read() { let Pair { x as same, y as same } = value; }")
        .tokenize()
        .unwrap();
    let error = parse_syntax_trees(&tokens).expect_err("duplicate lexical bindings");
    assert!(error.message.contains("introduced more than once"));
}
