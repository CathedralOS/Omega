use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn static_paths_cannot_continue_through_runtime_bindings() {
    let accepted = [
        "machine read(value: u32) -> u32 { value::Missing }",
        "machine read() -> u32 { let value: u32 = 1; value::Missing }",
        "data Pair { count: u32; } machine read(value: Pair) -> u32 { value::Missing }",
        "data Signal { case Empty; case Full(count: u32); } machine read(value: Signal) -> u32 { value::Empty }",
        "machine read(value: [u32; 2]) -> u32 { value::Missing }",
        "machine read(value: &[u32]) -> u32 { value::Missing }",
        "data Pair { count: u32; } machine read(value: Pair) -> u32 { value::count }",
        "machine read(value: &[u8]) -> u64 { value::len }",
        "data Pair { count: u32; } machine read(value: Pair) -> u32 { value::count::Missing }",
        "data Buffer { values: [u32; 2]; } machine read(value: &Buffer) -> u32 { value::values::Missing }",
    ].into_iter().filter(|source| crate::validate_program(&typed(source)).is_ok()).collect::<Vec<_>>();
    assert!(
        accepted.is_empty(),
        "unresolved lexical suffixes accepted: {accepted:#?}"
    );
}

#[test]
fn selected_lexical_fields_and_static_case_values_remain_legal() {
    for source in [
        "data Pair { count: u32; } machine read(value: Pair) -> u32 { value.count }",
        "data Signal { case Empty; } machine read() -> Signal { Signal::Empty }",
        "machine read(value: &[u32; 2]) -> u64 { value.len }",
        "machine read(value: &[u8]) -> u64 { value.len }",
        "data Text { bytes: [u8]; } machine read(value: &Text) -> u64 { value.bytes.len }",
        "data Buffer { values: [u32; 2]; } machine read(value: &Buffer) -> u64 { value.values.len }",
    ] {
        crate::validate_program(&typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
    }
}

#[test]
fn missing_head_cannot_hide_a_retained_runtime_root_in_the_leaf() {
    use typed_trees::expression::ExpressionNode;

    let mut program = typed("machine read(value: u32) -> u32 { value::Missing }");
    assert!(crate::validate_program(&program).is_err());
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| match expression {
            ExpressionNode::Name(path)
                if program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 2 =>
            {
                Some(handle)
            }
            _ => None,
        })
        .expect("runtime-rooted static path");
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    assert!(path.head_symbol.is_valid());
    path.symbol = path.head_symbol;
    path.head_symbol = symbols::SymbolHandle::invalid();
    assert!(
        crate::validate_program(&program).is_err(),
        "missing head hid the retained runtime binding in the leaf"
    );
}
