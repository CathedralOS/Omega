use super::*;

#[test]
fn eliminated_extent_preserves_collection_evaluation_bounds_and_selection() {
    // This is the existing composed-Unit scalar extent producer fixture, not
    // a claim that array-return transport for attached receivers is closed.
    let source = r#"
        boundary trait Host { machine exit(code: i32); }
        data Root { values: [i32; 5]; }
        machine Root::enter(&mut self) {
            let length: u64 = (self.values[1..4]).len;
            transition length == 3 {
                true -> yes()
                false -> no()
            }
            state yes(&mut self) { Host::exit(1); }
            state no(&mut self) { Host::exit(2); }
        }
        machine make_array(output: &mut u8) -> [i32; 5] { output = 1; [7, 9, 11, 13, 15] }
        machine donor(output: &mut u8) -> [i32; 5] { make_array(output) }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    let original = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("existing checked extent fixture");
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .expect("enter");
    let state = &original.machine_states(machine)[0];
    let state_symbol = state.symbol;
    let checked_trees::statement::StatementNode::LocalData(local) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("length initializer");
    };
    let source = local.initial_value;
    let value = original
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .find(|expression| {
            expression.state == state_symbol
                && expression.statement_ordinal == 0
                && expression.expression.primitive_type() == Some(PrimitiveType::U64)
        })
        .expect("retained compile-known extent")
        .expression
        .clone();
    assert!(matches!(&value, Scalar::IntegerLiteral { literal } if literal.value_u64() == Some(3)));
    let element = CheckedCallScalarArgument::Pure(value);
    let validate = |checked: &CheckedTrees| {
        super::super::validate(
            checked,
            state_symbol,
            0,
            source,
            PrimitiveType::U64,
            &element,
        )
    };
    validate(&original).expect("actual producer extent has static source and bounds evidence");
    let ExpressionNode::Member(member) = original.expression_table.expression(source) else {
        panic!("extent");
    };
    let projection = member.receiver;
    let ExpressionNode::Indexed(indexed) = original.expression_table.expression(projection) else {
        panic!("subslice");
    };
    let collection = indexed.collection;
    let range = indexed.index;
    let maker = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "make_array")
        .expect("maker");
    let target = original.machine_states(maker)[0].symbol;
    let call = original
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.target_symbol == target)
                .then_some(expression.clone())
        })
        .expect("effectful donor call");
    let mut changed = original.clone();
    *changed.typed.expression_table.expression_mut(collection) = call;
    assert!(matches!(
        validate(&changed),
        Err(LoweringError::Unsupported(
            "eliminated subslice extent requires retained source evaluation and view bounds custody"
        ))
    ));

    let mut changed = original.clone();
    let ExpressionNode::Range(bounds) = changed.expression_table.expression(range) else {
        panic!("bounds");
    };
    let end = bounds.end;
    *changed.typed.expression_table.expression_mut(end) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(6));
    assert!(
        validate(&changed).is_err(),
        "eliminated view exceeds static source extent"
    );

    let mut changed = original.clone();
    changed
        .facts
        .operators
        .uses
        .append(checked_trees::CheckedOperatorUseFact {
            expression: projection,
            spelling: language_core::OperatorSpelling::Index,
            ..Default::default()
        });
    assert!(
        validate(&changed).is_err(),
        "eliminated view changed selected spelling"
    );
}
