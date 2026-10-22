use source_files_to_tokens::Lexer;
use syntax_trees::expression::ExpressionNode;

#[test]
fn constructor_paths_keep_authored_field_evaluation_order_and_full_span() {
    for name in [
        "Value",
        "settings :: Value",
        "dep :: settings :: Value :: Some",
    ] {
        let source = format!("{name} {{ second: observe_second(), first: observe_first() }}");
        let source_id = source::SourceId::default();
        let tokens = Lexer::new(&source).tokenize().expect("tokens");
        let mut trees = syntax_trees::SyntaxTrees::new(source_id);
        let retained_source = crate::input::token_cursor::Input::retain_source(&tokens);
        let input = crate::input::token_cursor::Input::new(
            source_id,
            &tokens,
            &retained_source,
            &mut trees,
        );
        let (expression, rest) =
            crate::expressions::parse_expression::parse_expression_handle(&mut trees, input)
                .expect("qualified constructor");
        assert!(rest.tokens.is_empty());
        let ExpressionNode::StructLiteral(literal) = trees.expressions.expression(expression)
        else {
            panic!("constructor must remain an ordinary structural literal");
        };
        assert_eq!(
            literal.constructor_name.as_str(&trees),
            name.replace(' ', "")
        );
        let span = literal.constructor_name.source_span().span;
        assert_eq!(&source[span.start..span.end], name);
        let fields = trees.expressions.struct_fields(literal.fields);
        assert_eq!(fields.len(), 2);
        for (field, expected) in fields.iter().zip(["second", "first"]) {
            assert_eq!(field.name.as_str(&trees), expected);
            let ExpressionNode::Call(call) = trees.expressions.expression(field.value) else {
                panic!("authored field call must be retained");
            };
            assert_eq!(call.target.as_str(&trees), format!("observe_{expected}"));
        }
    }
}
