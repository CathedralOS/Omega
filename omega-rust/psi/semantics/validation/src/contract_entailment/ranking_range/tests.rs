use super::*;

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
