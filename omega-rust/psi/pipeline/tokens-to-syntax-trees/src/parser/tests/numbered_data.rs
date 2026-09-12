use super::{Lexer, parse_syntax_trees};
use syntax_trees::item::{DataMember, Item};

#[test]
fn native_representation_syntax_does_not_admit_numbered_data() {
    for declaration in [
        "repr native data Message { #7 value: u32; }",
        "repr native data Message { retired #8; }",
        "repr native data Message<T> { #7 value: T; }",
        "repr native data Message { case #7 Ready; }",
    ] {
        let tokens = Lexer::new(declaration).tokenize().expect("tokens");
        let error = parse_syntax_trees(&tokens).expect_err("unsupported representation syntax");
        assert!(
            error
                .message
                .contains("`repr native` data cannot carry identity numbers")
        );
    }
    let tokens = Lexer::new("repr native data Message { value: u32; }")
        .tokenize()
        .expect("tokens");
    parse_syntax_trees(&tokens).expect("existing unnumbered representation syntax");
}

#[test]
fn numbered_declarations_share_ordinary_data_and_source_identity() {
    for declaration in [
        "data Message { #7 value: u32; retired #8; }",
        "data Message<T> { #7 value: T; retired #8; }",
        "data Message [copy] { #7 value: u32; retired #8; }",
        "data Message where value <= 10 { #7 value: u32; retired #8; }",
        "data Message<'scope> { #7 value: &'scope u32; retired #8; }",
        "data Message { #7 value: u32; case #9 Ready; }",
    ] {
        let tokens = Lexer::new(declaration).tokenize().expect("tokens");
        let parsed = parse_syntax_trees(&tokens).expect("ordinary numbered declaration");
        let Some(Item::Data(definition)) = parsed.root_items().next() else {
            panic!("numbering must not select a different declaration representation");
        };
        let DataMember::Field(field) = &parsed.items.data_members(definition.members)[0] else {
            panic!("ordinary first field");
        };
        assert_eq!(field.identity, Some(7));
        assert_eq!(field.name.as_str(), "value");
        let span = field.name.source_span().span;
        assert_eq!(&declaration[span.start..span.end], "value");
    }
}

#[test]
fn nested_version_blocks_reject_regardless_of_leading_member() {
    for members in [
        "version v1 { #1 value: u32; } #1 value: u32;",
        "#1 value: u32; version v1 { #1 value: u32; }",
        "retired #2; version v1 { #1 value: u32; } #1 value: u32;",
    ] {
        let declaration = format!("data Message {{ {members} }}");
        let tokens = Lexer::new(&declaration).tokenize().expect("tokens");
        let error = parse_syntax_trees(&tokens).expect_err("retired version syntax");
        assert!(error.message.contains("data `version` blocks are retired"));
    }
}

#[test]
fn numbered_data_rejects_duplicate_and_retired_identity_reuse() {
    for members in [
        "#7 left: u32; #7 right: u32;",
        "#7 value: u32; retired #7;",
        "#7 value: u32; retired #8; retired #8;",
    ] {
        let declaration = format!("data Message {{ {members} }}");
        let tokens = Lexer::new(&declaration).tokenize().expect("tokens");
        let error = parse_syntax_trees(&tokens).expect_err("identity collision");
        assert!(error.message.contains("declared more than once"));
    }
}
