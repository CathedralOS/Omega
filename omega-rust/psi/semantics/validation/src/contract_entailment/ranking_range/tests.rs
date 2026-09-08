use super::*;

#[test]
fn constructor_operands_retain_their_nominal_type_for_selected_meaning() {
    let program =
        typed("data Card { rank: u64; } machine make(card: Card) -> Card { Card { rank: 1 } }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (expression, owner) = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            if let ExpressionNode::StructLiteral(literal) = node {
                Some((handle, literal.type_symbol))
            } else {
                None
            }
        })
        .expect("constructor operand");
    let expected = program
        .type_reference_table
        .find_named_type_reference(owner)
        .expect("exact Card carrier");
    assert_eq!(
        meanings::builtin(&program, machine, state, expression, 0),
        Some(Some(expected))
    );
}

#[test]
fn state_aliases_cannot_reuse_root_or_sibling_formal_symbols() {
    let mut program = typed(
        "machine walk(left: u32, right: u32) -> u32 { transition { _ -> next(right, left) } state next(first: u32, second: u32) { first } }",
    );
    let machine = program.machines()[0].clone();
    let root = program.machine_states(&machine)[0].clone();
    let state = program.machine_states(&machine)[1].clone();
    let root_parameters = program.state_parameters(&root);
    let mapping = [root_parameters[1].symbol, root_parameters[0].symbol];
    assert!(validate_mapping(&program, &machine, &state, &mapping).is_some());
    let original = program.state_parameters(&state)[0].symbol;
    for forged in [mapping[1], program.state_parameters(&state)[1].symbol] {
        program.state_parameters.span_mut_or_empty(state.parameters)[0].symbol = forged;
        assert!(validate_mapping(&program, &machine, &state, &mapping).is_none());
    }
    program.state_parameters.span_mut_or_empty(state.parameters)[0].symbol = original;
    assert!(validate_mapping(&program, &machine, &state, &mapping).is_some());
}

#[test]
fn missing_auxiliary_source_alias_cannot_skip_destination_copy_equality() {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};

    let program = typed(
        r#"
        machine walk(remaining: u32 [0..=5], step: u32)
        requires step > 0;
        terminates by remaining in 0..=5;
        -> u32 {
            transition { _ -> drop(remaining) }
            state drop(pending: u32 [0..=5]) {
                transition { _ -> next(pending, 0, 1) }
            }
            state next(left: u32, first: u32, second: u32) { left }
        }
    "#,
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let root = &states[0];
    let source = &states[1];
    let destination = &states[2];
    let parameters = program.state_parameters(root);
    let custody = program
        .ranking_expression_custody_for(machine.symbol)
        .unwrap();
    let subject = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Name(name) if name.symbol == parameters[0].symbol)
                .then_some(handle)
        })
        .unwrap();
    let StatementNode::Transition(transition) =
        &program.statement_table.statements(source.statement_nodes)[0]
    else {
        panic!("one arrival");
    };
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        panic!("named arrival");
    };
    assert!(
        prove_ranking_range_transition(
            &program,
            machine,
            custody.rank_range.unwrap(),
            RankingRangeMeasure::Single(subject),
            RankingRangePremises::EntryInvariant,
            RankingRangeState {
                state: source,
                entry_parameters: &[parameters[0].symbol]
            },
            RankingRangeState {
                state: destination,
                entry_parameters: &[
                    parameters[0].symbol,
                    parameters[1].symbol,
                    parameters[1].symbol
                ]
            },
            &[],
            &[],
            program.statement_table.expression_handles(*arguments),
        )
        .is_none()
    );
}

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
