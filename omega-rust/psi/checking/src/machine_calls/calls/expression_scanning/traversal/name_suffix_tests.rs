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
    ].into_iter().filter(|source| crate::validate_program(&crate::front_end::typed_program(source)).is_ok()).collect::<Vec<_>>();
    assert!(
        accepted.is_empty(),
        "unresolved lexical suffixes accepted: {accepted:#?}"
    );
}

#[test]
fn missing_head_cannot_hide_a_retained_runtime_root_in_the_leaf() {
    use typed_trees::expression::ExpressionNode;

    let mut program =
        crate::front_end::typed_program("machine read(value: u32) -> u32 { value::Missing }");
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
