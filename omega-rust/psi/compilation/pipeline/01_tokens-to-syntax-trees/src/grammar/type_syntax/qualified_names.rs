use source::SourceId;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
#[test]
fn boolean_domain_indices_retain_literal_identity_and_span_in_qualifications_and_casts() {
    for spelling in ["true", "false"] {
        for cast in [false, true] {
            let text = if cast {
                format!(
                    "machine convert(value: u64) {{ let tagged: u64 = value as u64 in Tagged<{spelling}>; }}"
                )
            } else {
                format!("data Packet {{ value: u64 in policy::Tagged<{spelling}>; }}")
            };
            let source_id = SourceId(17);
            let tokens = Lexer::new(&text)
                .tokenize()
                .expect("tokenize boolean domain index");
            let mut syntax = SyntaxTrees::new(SourceId::default());
            crate::parse(&mut syntax, source_id, &tokens).expect("parse boolean domain index");
            let arguments = if cast {
                syntax
                    .expressions
                    .iter_expressions()
                    .find_map(|(_, expression)| {
                        let syntax_trees::expression::ExpressionNode::Cast(cast) = expression
                        else {
                            return None;
                        };
                        Some(cast.semantic_domain_arguments)
                    })
                    .expect("indexed cast")
            } else {
                syntax.type_references.domain_constraints()[0].arguments
            };
            let [argument] = syntax.type_references.type_reference_handles(arguments) else {
                panic!("one boolean index");
            };
            // Parsing retains the authored value tree; build-time evaluation
            // gives a Boolean index its typed identity against the destination.
            let syntax_trees::types::TypeReferenceNode::ConstExpression(expression) =
                syntax.type_references.type_reference(*argument)
            else {
                panic!("boolean literal retains its authored value expression");
            };
            let syntax_trees::expression::ExpressionNode::Boolean(value) =
                syntax.expressions.expression(*expression)
            else {
                panic!("boolean index remains a Boolean literal");
            };
            assert_eq!(*value, spelling == "true");
            let expected_start = text.find(spelling).expect("authored boolean literal");
            let source_span = syntax.expressions.source_span(*expression);
            assert_eq!(source_span.source_id, source_id);
            assert_eq!(source_span.span.start, expected_start);
            assert_eq!(source_span.span.end, expected_start + spelling.len());
        }
    }
}

#[test]
fn qualified_type_names_reject_missing_path_members() {
    for name in ["dungeon::", "dungeon::::Damage", "dungeon::<u64>"] {
        let text = format!("data Packet {{ value: {name}; }}");
        let tokens = Lexer::new(&text)
            .tokenize()
            .expect("tokenize invalid qualified type");
        crate::parse_syntax_trees(&tokens).expect_err("every separator needs a member");
    }
}
