use super::*;

#[test]
fn nested_static_constructor_namespace_is_not_a_live_operand() {
    let source = "data Inner { value: u64; }
        data Other { value: u64; }
        data Outer { inner: Inner; }
        machine Inner::new(value: u64) -> Inner { Inner { value: value } }
        machine Inner::observe(&self) -> u64 { self.value }
        machine wrap() -> Outer {
            let result: Outer = Outer { inner: Inner::new(7) };
            result
        }
        machine observe(inner: Inner) -> u64 { inner.observe() }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let wrapper = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wrap")
        .unwrap();
    let state = &program.machine_states(wrapper)[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("nested constructor local");
    };
    let root = local.initial_value;
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(root) else {
        panic!("outer constructor");
    };
    let nested = program.expression_table.struct_fields(literal.fields)[0].value;
    let ExpressionNode::Call(call) = program.expression_table.expression(nested) else {
        panic!("nested static call");
    };
    let receiver = call.receiver;
    let resolve = |expression| {
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) {
            Err("namespace must not become a live operand")
        } else {
            Ok(None)
        }
    };
    assert_eq!(
        expression_permission_provenance(&program, root, &mut |expression| resolve(expression)),
        Ok(None)
    );
    let other = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .unwrap()
        .symbol;
    for symbol in [other, symbols::SymbolHandle::invalid()] {
        let mut changed = program.clone();
        let ExpressionNode::Name(name) = changed.expression_table.expression_mut(receiver) else {
            panic!("static namespace");
        };
        name.symbol = symbol;
        name.head_symbol = symbol;
        assert!(
            expression_permission_provenance(&changed, root, &mut |expression| resolve(expression))
                .is_err()
        );
    }
    let observer = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap();
    let observer_state = &program.machine_states(observer)[0];
    let typed_trees::statement::StatementNode::Expression(observation) = &program
        .statement_table
        .statements(observer_state.statement_nodes)[0]
    else {
        panic!("value receiver call");
    };
    let ExpressionNode::Call(value_call) = program.expression_table.expression(*observation) else {
        panic!("value receiver");
    };
    let expected = PermissionProvenance::Established {
        machine_symbol: observer.symbol,
        state_symbol: observer_state.symbol,
        source: language_semantics::PermissionEventSource::StateEntry,
    };
    assert_eq!(
        expression_permission_provenance(&program, *observation, &mut |expression| {
            assert_eq!(expression, value_call.receiver);
            Ok(Some(expected))
        }),
        Ok(Some(expected))
    );
    let mut changed = program.clone();
    let ExpressionNode::Call(call) = changed.expression_table.expression_mut(nested) else {
        panic!("static call");
    };
    call.target_symbol = value_call.target_symbol;
    assert!(
        expression_permission_provenance(&changed, root, &mut |expression| resolve(expression))
            .is_err(),
        "a self formal cannot be replaced with a namespace"
    );
}

#[test]
fn missing_common_origin_is_distinct_from_stale_or_cyclic_operands() {
    let mut program = TypedTrees::default();
    let first = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let second = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let operands = program
        .expression_table
        .insert_expression_handles([first, second]);
    let root = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(operands));
    let mut resolve = |expression| {
        Ok(Some(PermissionProvenance::Established {
            machine_symbol: symbols::SymbolHandle::invalid(),
            state_symbol: symbols::SymbolHandle::invalid(),
            source: language_semantics::PermissionEventSource::Statement {
                statement_index: usize::from(expression == second),
            },
        }))
    };
    assert_eq!(
        expression_permission_provenance(&program, root, &mut resolve),
        Ok(None)
    );
    for replacement in [ExpressionHandle::invalid(), root] {
        let mut changed = program.clone();
        changed
            .expression_table
            .set_expression_handle_at_offset(operands, 1, replacement);
        assert!(expression_permission_provenance(&changed, root, &mut resolve).is_err());
    }
}

#[test]
fn exact_named_constants_have_no_live_place_origin() {
    let source = "const OFFSET: u64 = 17; machine value() -> u64 { OFFSET }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let mut program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let constant = program.const_declarations()[0].symbol;
    // Constant evaluation may already have substituted the source occurrence.
    // Retain its exact declaration handle to test the provenance query boundary.
    let root = program.expression_table.insert(ExpressionNode::Name(
        typed_trees::expression::TableNamePath {
            symbol: constant,
            head_symbol: constant,
            ..Default::default()
        },
    ));
    assert_eq!(
        expression_permission_provenance(&program, root, &mut |_| Err(
            "constant must not resolve to a live place"
        )),
        Ok(None)
    );
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(root) else {
        panic!("retained constant name");
    };
    path.symbol = symbols::SymbolHandle::invalid();
    assert!(
        expression_permission_provenance(&program, root, &mut |_| Err("invalid declaration"))
            .is_err()
    );
}
