use source_files_to_tokens::Lexer;
use syntax_trees::item::{DataMember, Item};
use syntax_trees::types::TypeReferenceNode;

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
