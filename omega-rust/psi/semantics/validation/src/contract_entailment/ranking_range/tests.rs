use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn slice_length_projection_does_not_numeric_bind_its_descriptor() {
    let program = typed("machine length(values: &[u8]) -> u64 { values.len }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (expression, receiver) = program
        .expression_table
        .iter_expressions()
        .find_map(|(expression, _)| {
            crate::places::collection_length_receiver(&program, machine, Some(state), expression)
                .map(|receiver| (expression, receiver))
        })
        .unwrap();
    let bindings = lengths::bindings(&program, state, None);
    let mut engine = Engine::strict_with_symbol_bindings(&program, machine, &[]);
    assert!(engine.normalize(expression).is_none());
    lengths::install(
        &program,
        machine,
        state,
        state,
        &bindings,
        &mut engine,
        &[expression],
    )
    .unwrap();
    let length = engine.normalize(expression).unwrap();
    assert_eq!(length, Polynomial::atom(bindings[0].1.clone()));
    assert!(engine.normalize(receiver).is_none());
    assert!(engine.bind_strict_projection(expression, length));
    assert!(!engine.bind_strict_projection(expression, Polynomial::default()));
    assert!(!engine.bind_strict_projection(receiver, Polynomial::default()));
}

#[test]
fn slice_projection_checks_subslice_geometry_before_using_its_length() {
    let program = typed("machine tail(values: &[u8]) -> &[u8] { values[1..] }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, ExpressionNode::Indexed(_)).then_some(expression)
        })
        .unwrap();
    let bindings = lengths::bindings(&program, state, None);
    let length = Polynomial::atom(bindings[0].1.clone());
    let mut engine = Engine::strict_with_symbol_bindings(&program, machine, &[]);
    assert!(
        lengths::actual(&program, machine, state, expression, &bindings, &mut engine).is_none()
    );
    assert!(engine.install_hypotheses(vec![(
        BinaryOperator::GreaterOrEqual,
        length.clone(),
        Polynomial::constant(BigInt::from_i64(1))
    )]));
    assert_eq!(
        lengths::actual(&program, machine, state, expression, &bindings, &mut engine),
        Some(length.sub(&Polynomial::constant(BigInt::from_i64(1))))
    );
}

#[test]
fn integer_rank_bindings_require_the_canonical_type_symbol() {
    let tokens = source_files_to_tokens::Lexer::new(
        "machine value(input: u64, signed: i64) -> u64 { input }",
    )
    .tokenize()
    .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let mut program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let parameters = program.state_parameters(state);
    let unsigned = parameters[0].type_reference;
    let signed = parameters[1].type_reference;
    assert_eq!(
        exact_integer_parameter(&program, unsigned),
        Some(PrimitiveType::U64)
    );
    let TypeReferenceNode::Named { name, .. } = program
        .type_reference_table
        .type_reference(unsigned)
        .clone()
    else {
        panic!("primitive unsigned type");
    };
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(signed).clone()
    else {
        panic!("primitive signed type");
    };
    for symbol in [Default::default(), symbol] {
        let forged = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: name.clone(),
            });
        assert_eq!(exact_integer_parameter(&program, forged), None);
    }
}
