use crate::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::item::Item;
use syntax_trees::statement::StatementNode;

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
        assert!(matches!(
            completion.guard,
            syntax_trees::statement::TransitionGuardNode::Always
        ));
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
