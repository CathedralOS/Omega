use super::*;

fn checked(argument: &str, combined: bool) -> checked_trees::CheckedTrees {
    let source = format!(
        r#"
        machine inner(input: bool) -> bool {{ input }}
        machine outer(input: bool) -> bool {{ input }}
        machine value(flag: bool, other: bool) -> bool {{
            transition flag {{
                true -> finish({argument})
                false -> finish(flag || outer(inner(flag)))
            }}
            state finish(input: bool) -> bool {{ input }}
        }}
    "#
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let mut typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    if combined {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap()
            .clone();
        let state = typed.machine_states(&machine)[0].clone();
        let statements = typed.statement_table.statements_mut(state.statement_nodes);
        let [
            StatementNode::Transition(first),
            StatementNode::Transition(second),
        ] = statements
        else {
            panic!("two authored transition arms");
        };
        first.continuation = second.target;
        typed.machine_states_mut(&machine)[0].statement_nodes =
            arena::HandleSpan::from_parts(state.statement_nodes.start(), 1);
    }
    crate::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

#[test]
fn scalar_computations_keep_nested_authored_call_ordinals() {
    let checked = checked("other && outer(inner(flag))", false);
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| root.statement_ordinal == 0)
        .expect("selected argument root");
    let CheckedScalarComputationKind::Select { when_true, .. } = plans.nodes.get(root.root).kind
    else {
        panic!("RHS remains conditional");
    };
    let CheckedScalarComputationKind::Call {
        call_ordinal: outer,
        arguments,
        source_call,
        ..
    } = plans.nodes.get(when_true).kind
    else {
        panic!("outer invocation");
    };
    let inner_handle = plans.operands.span_or_empty(arguments)[0];
    let CheckedScalarComputationKind::Call {
        call_ordinal: inner,
        ..
    } = plans.nodes.get(inner_handle).kind
    else {
        panic!("inner invocation");
    };
    assert!(
        outer < inner,
        "preorder occurrence identities are not execution order"
    );
    assert_eq!(
        checked
            .facts
            .flow
            .control
            .calls
            .get(source_call)
            .call_ordinal,
        outer as usize
    );
}

#[test]
fn scalar_computations_keep_combined_arm_roles_distinct() {
    let checked = checked("other && outer(inner(flag))", true);
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots.iter().map(|(_, root)| root).collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].state, roots[1].state);
    assert_eq!(roots[0].statement_ordinal, roots[1].statement_ordinal);
    assert_eq!(
        roots[0].role,
        CheckedScalarExpressionRole::TransitionArgument {
            argument_ordinal: 0
        }
    );
    assert_eq!(
        roots[1].role,
        CheckedScalarExpressionRole::TransitionContinuationArgument {
            argument_ordinal: 0
        }
    );
    assert_ne!(roots[0].root, roots[1].root);
}

#[test]
fn scalar_computations_do_not_require_skipped_call_occurrences() {
    for argument in [
        "false && outer(inner(flag))",
        "true || outer(inner(flag))",
        "(true == false) && outer(inner(flag))",
    ] {
        let checked = checked(argument, false);
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| root.statement_ordinal == 0)
            .expect("known argument root");
        assert!(
            matches!(
                plans.nodes.get(root.root).kind,
                CheckedScalarComputationKind::Value(_)
            ),
            "{argument}: {:?}",
            plans.nodes.get(root.root).kind
        );
    }
}

#[test]
fn scalar_computations_apply_boolean_templates_to_computed_operands() {
    for argument in [
        "!(other && outer(inner(flag)))",
        "(other && outer(inner(flag))) == flag",
        "(other && outer(inner(flag))) != flag",
    ] {
        let checked = checked(argument, false);
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| root.statement_ordinal == 0)
            .expect("computed argument root");
        assert!(matches!(
            plans.nodes.get(root.root).kind,
            CheckedScalarComputationKind::Apply { .. }
        ));
    }
}
