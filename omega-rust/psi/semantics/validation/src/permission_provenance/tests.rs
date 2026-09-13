use super::*;

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
