use super::parameter_scoped_type_reference;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionLateBinding,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

fn indexed_program(operator: &str) -> (TypedTrees, ExpressionHandle) {
    let source = format!(
        "data Choice [copy] {{ case Ready; case Empty; }}
         {operator}
         machine choose(values: [Choice; 1], index: u64) -> Choice {{ values[index] }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let indexed = program
        .expression_table
        .expression_entries()
        .find_map(|(expression, node)| {
            matches!(node, ExpressionNode::Indexed(_)).then_some(expression)
        })
        .expect("indexed expression");
    (program, indexed)
}

fn retain_builtin_index(program: &mut TypedTrees, indexed: ExpressionHandle) {
    let occurrence = program
        .expression_table
        .authored_selection_occurrences(indexed)
        .next()
        .expect("operator occurrence");
    let mut selections = program.authored_declaration_selections().clone();
    selections
        .finalize_intrinsic(
            occurrence,
            AuthoredDeclarationSelectionLateBinding::CheckedOperator,
            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        )
        .expect("finalize builtin operator");
    program.retain_authored_declaration_selections(selections);
}

#[test]
fn parameter_index_requires_retained_builtin_meaning() {
    let (mut program, indexed) = indexed_program("");
    let parameters = program
        .state_parameters(&program.machine_states(&program.machines()[0])[0])
        .to_vec();
    assert!(parameter_scoped_type_reference(&program, &parameters, indexed).is_none());
    retain_builtin_index(&mut program, indexed);
    let reference = parameter_scoped_type_reference(&program, &parameters, indexed)
        .expect("builtin element type");
    assert_eq!(
        program.type_reference_symbol(reference),
        program.data_definitions()[0].symbol
    );
    let unretained_node = program.expression_table.expression(indexed).clone();
    let unretained = program.expression_table.insert(unretained_node);
    assert!(parameter_scoped_type_reference(&program, &parameters, unretained).is_none());
}

#[test]
fn parameter_index_cannot_replace_declared_operator_result_with_element_type() {
    let (mut program, indexed) = indexed_program(
        "data Indexing {} boundary operator [] Indexing::index(values: &[Choice], index: u64) -> u64;",
    );
    let parameters = program
        .state_parameters(&program.machine_states(&program.machines()[0])[0])
        .to_vec();
    // Even forged builtin custody cannot erase an applicable declaration whose
    // result is u64 rather than the Choice collection element.
    retain_builtin_index(&mut program, indexed);
    assert!(parameter_scoped_type_reference(&program, &parameters, indexed).is_none());
}
