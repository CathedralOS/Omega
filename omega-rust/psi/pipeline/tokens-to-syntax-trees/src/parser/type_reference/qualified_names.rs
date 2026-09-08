use source::SourceId;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{DataMember, Item};
use syntax_trees::types::TypeReferenceNode;

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
            crate::parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
                .expect("parse boolean domain index");
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
            let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(*argument)
            else {
                panic!("boolean literal retains the canonical named-atom syntax");
            };
            assert_eq!(name.as_str(), spelling);
            let expected_start = text.find(spelling).expect("authored boolean literal");
            assert_eq!(name.source_span().source_id, source_id);
            assert_eq!(name.source_span().span.start, expected_start);
            assert_eq!(name.source_span().span.end, expected_start + spelling.len());
        }
    }
}

#[test]
fn domain_index_names_retain_authored_source_and_complete_span() {
    for spelling in ["SIZE", "dungeon::combat::SIZE"] {
        let text = format!("data Packet {{ value: u64 in Indexed<{spelling}>; }}");
        let tokens = Lexer::new(&text).tokenize().expect("tokenize domain index");
        let source_id = SourceId(17);
        let mut syntax = SyntaxTrees::new(SourceId::default());
        crate::parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("parse domain index");
        let domains = syntax.type_references.domain_constraints();
        let [domain] = domains.as_slice() else {
            panic!("one domain application");
        };
        let [argument] = syntax
            .type_references
            .type_reference_handles(domain.arguments)
        else {
            panic!("one domain index");
        };
        let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(*argument)
        else {
            panic!("named domain index");
        };
        let expected_start = text.find(spelling).expect("authored index spelling");
        assert_eq!(name.as_str(), spelling);
        assert_eq!(name.source_span().source_id, source_id);
        assert_eq!(name.source_span().span.start, expected_start);
        assert_eq!(name.source_span().span.end, expected_start + spelling.len());
    }
}

#[test]
fn qualified_type_names_retain_the_complete_authored_path() {
    for suffix in ["", "<u64>"] {
        let text = format!("data Packet {{ value: dungeon::combat::Damage{suffix}; }}");
        let tokens = Lexer::new(&text)
            .tokenize()
            .expect("tokenize qualified type");
        let syntax = crate::parse_syntax_trees(&tokens).expect("parse qualified type");
        let Item::Data(declaration) = syntax.root_items().next().expect("Packet") else {
            panic!("data declaration");
        };
        let [DataMember::Field(field)] = syntax.items.data_members(declaration.members) else {
            panic!("one field");
        };
        let name = match syntax.type_references.type_reference(field.type_reference) {
            TypeReferenceNode::Named(name) => name,
            TypeReferenceNode::Generic { base_name, .. } => base_name,
            other => panic!("named type, got {other:?}"),
        };
        assert_eq!(name.as_str(), "dungeon::combat::Damage");
        let span = name.source_span().span;
        assert_eq!(&text[span.start..span.end], name.as_str());
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

#[test]
fn qualified_type_names_in_generic_arguments_keep_authored_custody() {
    let text = "data Packet { value: Box<dungeon::combat::Damage>; }";
    let tokens = Lexer::new(text)
        .tokenize()
        .expect("tokenize generic argument");
    let syntax = crate::parse_syntax_trees(&tokens).expect("parse generic argument");
    let Item::Data(declaration) = syntax.root_items().next().expect("Packet") else {
        panic!("data declaration");
    };
    let [DataMember::Field(field)] = syntax.items.data_members(declaration.members) else {
        panic!("one field");
    };
    let TypeReferenceNode::Generic { arguments, .. } =
        syntax.type_references.type_reference(field.type_reference)
    else {
        panic!("generic field");
    };
    let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
        panic!("one argument");
    };
    let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(*argument) else {
        panic!("named argument");
    };
    assert_eq!(name.as_str(), "dungeon::combat::Damage");
    let span = name.source_span().span;
    assert_eq!(&text[span.start..span.end], name.as_str());
}
