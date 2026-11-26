//! What the parser hands name resolution for a constant written without a
//! type annotation.
//!
//! `const LIMIT = 7u8;` means `const LIMIT: u8 = 7u8;`
//! (`wiki/spec/language/numeric_values.md#literal-landing-and-destinations`).
//! Every later stage reads a constant's declared type, so the fact the parser
//! owes them is that type: the one the initializer's suffix, `as` target or
//! Boolean literal names, placed at the initializer. An initializer that names
//! no type is refused there.

use syntax_trees::item::Item;
use syntax_trees::types::TypeReferenceNode;

/// The declared type of the one constant in `source`, as the name it spells
/// and the source text its span covers.
fn declared_type(source: &str) -> (String, String) {
    let syntax = crate::tests::parse_source(source);
    let constant = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Const(definition) => Some(definition.clone()),
            _ => None,
        })
        .expect("one constant");
    match syntax
        .type_references
        .type_reference(constant.type_reference)
    {
        TypeReferenceNode::Named(name) => {
            let span = name.source_span().span;
            (
                name.as_str(&syntax).to_owned(),
                source[span.start..span.end].to_owned(),
            )
        }
        other => panic!("{source}: a named declared type, found {other:?}"),
    }
}

fn refusal(source: &str) -> (String, String) {
    let error = crate::tests::parse_error(source);
    let span = error.source_span.span;
    (error.message, source[span.start..span.end].to_owned())
}

#[test]
fn a_suffix_a_cast_or_a_boolean_names_the_constant_type_at_its_initializer() {
    for (source, name, written) in [
        ("const LIMIT = 7u8;", "u8", "7u8"),
        ("const OFFSET = -3i32;", "i32", "-3i32"),
        ("const HALF = 0.5f32;", "f32", "0.5f32"),
        ("const READY = true;", "bool", "true"),
        ("const WIDE = 300u16 as u64;", "u64", "u64"),
    ] {
        assert_eq!(
            declared_type(source),
            (name.to_owned(), written.to_owned()),
            "{source}"
        );
    }
}

#[test]
fn an_annotation_is_kept_as_written() {
    assert_eq!(
        declared_type("const LIMIT: u16 = 7u8;"),
        ("u16".to_owned(), "u16".to_owned())
    );
}

#[test]
fn a_wholly_anonymous_initializer_is_refused_where_it_is_written() {
    for (source, written) in [
        ("const LIMIT = 5;", "5"),
        ("const LIMIT = 2 + 3;", "2 + 3"),
        ("const HALF = 0.5;", "0.5"),
    ] {
        let (message, at) = refusal(source);
        assert!(message.contains("wholly anonymous"), "{source}: {message}");
        assert_eq!(at, written, "{source}");
    }
}

#[test]
fn an_initializer_whose_type_only_typing_finds_is_refused_as_not_supported() {
    for (source, written) in [
        ("const LIMIT = measure();", "measure()"),
        ("const LIMIT = OTHER;", "OTHER"),
        ("const LIMIT = 2u8 + 3;", "2u8 + 3"),
        ("const LIMIT = 5 as +Metres;", "5 as +Metres"),
    ] {
        let (message, at) = refusal(source);
        assert!(
            message.starts_with("not supported yet:"),
            "{source}: {message}"
        );
        assert_eq!(at, written, "{source}");
    }
}
