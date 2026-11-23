#[test]
fn repeated_constant_uses_name_one_declaration_and_keep_their_own_source_selections() {
    use super::source_request;
    use crate::source_resolved::{AuthoredDeclarationSelectionTarget, expression::ExpressionNode};

    let source = "data Block [copy] { before: u64; bytes: [u8; 3]; after: u64; }
        const FILL: Block = Block { before: 17u64, bytes: [1u8, 2u8, 3u8], after: 91u64 };
        machine main() { let first: Block = FILL; let second: Block = FILL; }";
    let trees = crate::resolution::resolve(source_request(source)).unwrap();
    let declaration = trees.roots.const_declarations.iter().next().unwrap();
    let table = &trees.tables.bodies.expressions;
    let ExpressionNode::StructLiteral(_) = table.expression(declaration.initializer) else {
        panic!("the constant must have a record initializer");
    };
    let mut selected = Vec::new();
    for (start, name) in source.match_indices("FILL").skip(1) {
        let (handle, path) = table
            .iter_expressions()
            .find_map(|(handle, expression)| {
                let span = table.source_span(handle).span;
                match expression {
                    ExpressionNode::Name(path)
                        if span.start == start && span.end == start + name.len() =>
                    {
                        Some((handle, path))
                    }
                    _ => None,
                }
            })
            .unwrap();
        assert_eq!(path.symbol, declaration.symbol);
        let selection = table
            .authored_selection_occurrences(handle)
            .map(|id| trees.authored_declaration_selections().get(id).unwrap())
            .find(|row| {
                row.source_span() == table.source_span(handle)
                    && matches!(row.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == declaration.symbol)
            })
            .unwrap();
        assert_eq!(
            Some(selection.exposure()),
            table.authored_expression_exposure(handle)
        );
        selected.push(selection.occurrence_id());
    }
    assert_eq!(selected.len(), 2);
    assert_ne!(selected[0], selected[1]);
}

#[test]
fn a_nested_use_of_a_type_scoped_constant_retains_its_carrier() {
    use crate::source_resolved::expression::ExpressionNode;

    let source = "data Paint [copy] { value: u32; }
        const Paint::TOP: Paint = Paint { value: 9 };
        machine make() -> Paint { selected() machine selected() -> Paint { Paint::TOP } }";
    let trees = crate::resolution::resolve(super::source_request(source)).unwrap();
    let carrier = trees
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Paint")
        .unwrap();
    let constant = trees.roots.const_declarations.iter().next().unwrap();
    let table = &trees.tables.bodies.expressions;
    let start = source.rfind("Paint::TOP").unwrap();
    let path = table
        .iter_expressions()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Name(path) if table.source_span(handle).span.start == start => {
                Some(path)
            }
            _ => None,
        })
        .expect("selected type-scoped constant");
    assert_eq!(path.head_symbol, carrier.symbol);
    assert_eq!(path.symbol, constant.symbol);
    assert_eq!(
        table.name_path_member_symbols(path.member_symbols),
        &[carrier.symbol, constant.symbol]
    );
}
