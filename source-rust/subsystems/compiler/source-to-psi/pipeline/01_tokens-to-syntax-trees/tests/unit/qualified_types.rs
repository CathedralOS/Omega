use crate::frontend_fixture::Lexer;
use source_text::SourceId;
use syntax_trees::SyntaxTrees;
#[test]
fn boolean_domain_indices_retain_literal_identity_and_span_in_qualifications_and_casts() {
    for spelling in ["true", "false"] {
        for cast in [false, true] {
            let text = if cast {
                format!(
                    "machine convert(value: u64) {{ let tagged: u64 = value as +Tagged<{spelling}>; }}"
                )
            } else {
                format!("data Packet {{ value: u64 in policy\\Tagged<{spelling}>; }}")
            };
            let source_id = SourceId(17);
            let tokens = Lexer::new(&text)
                .tokenize()
                .expect("tokenize boolean domain index");
            let mut syntax = SyntaxTrees::new(SourceId::default());
            crate::parse_into(&mut syntax, source_id, &tokens).expect("parse boolean domain index");
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
    for name in [
        "dungeon::",
        "dungeon::::Damage",
        "dungeon::<u64>",
        r"dungeon\",
        r"dungeon\\Damage",
        r"dungeon::Packet\Header",
    ] {
        let text = format!("data Packet {{ value: {name}; }}");
        crate::tests::parse_error(&text);
    }
}

#[test]
fn a_module_path_can_end_in_a_static_member_or_generic_call() {
    let text = r"use net\wire\Packet::Header;
        data Record { field: net\wire\Packet::Header; }
        machine caller() { net\wire\send::<u8>(); }";
    crate::tests::parse_source(text);
}

#[test]
fn value_casts_require_explicit_domain_entries() {
    for target in ["u64 in Wrapping", "u64 in Marker", "u64 in Family<true>"] {
        let source = format!("machine m(x: u64) {{ let y: {target} = x as {target}; }}");
        let error = crate::tests::parse_error(&source);
        assert!(
            error.message.contains("`as T in D` is retired"),
            "{error:?}"
        );
    }
    for source in [
        "machine m(x: u64) { let y: u64 in Marker = x as +Marker; }",
        "machine m(x: u8) { let y: u64 in Wrapping = x as u64 +Wrapping; }",
        "machine m(x: @u64) { let y: @u8 in Marker = x as @u8 in Marker; }",
    ] {
        crate::tests::parse_source(source);
    }
}

#[test]
fn file_qualifiers_can_start_types_and_operands_after_an_operator() {
    let text = "machine read(input: @|wire|::Packet) -> u8 { input.flags | |wire|::MASK }";
    let syntax = crate::tests::parse_source(text);
    let contains_mask = syntax.expressions.iter_expressions().any(|(_, node)| {
        let syntax_trees::expression::ExpressionNode::Name { members, .. } = node else {
            return false;
        };
        syntax
            .expressions
            .identifier_path_members(*members)
            .iter()
            .any(|name| name.as_str(&syntax) == "|wire|::MASK")
    });
    assert!(contains_mask);

    let invalid = text.replace("flags | |wire|", "flags||wire|");
    crate::tests::parse_error(&invalid);
}
