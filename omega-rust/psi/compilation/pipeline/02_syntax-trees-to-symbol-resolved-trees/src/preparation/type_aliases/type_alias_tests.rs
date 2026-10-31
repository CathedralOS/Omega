//! Real parser trees exercise caller-first expansion and declaration cycles.

use super::*;

fn normalized(text: &str) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let mut sources = source::SourceMap::default();
    let id = sources
        .add("alias/main.omg".into(), text.to_owned())
        .source_id;
    let mut syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
    let selection =
        ConstantSelection::new(&syntax, Some(std::sync::Arc::new(sources)), Vec::new())?;
    normalize(&mut syntax, &selection)?;
    Ok(syntax)
}

#[test]
fn aliases_expand_supplied_arguments_before_rhs_cycles_and_keep_obligations() {
    let syntax =
        normalized("data Identity<T> = T; data Cell { value: Identity<Identity<u8>>; }").unwrap();
    let data = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) => Some(data),
            _ => None,
        })
        .unwrap();
    let syntax_trees::item::DataMember::Field(field) = &syntax.items.data_members(data.members)[0]
    else {
        panic!("field");
    };
    let TypeReferenceNode::Named(name) =
        syntax.type_references.type_reference(field.type_reference)
    else {
        panic!("transparent expansion");
    };
    assert_eq!(name.as_str(&syntax), "u8");
    assert_eq!(
        syntax
            .type_references
            .type_alias_applications(field.type_reference)
            .count(),
        2
    );
    // Re-entry adds a new use after the header's working RHS was normalized.
    // Source binding must still use its immutable authored RHS, and supplied
    // actuals must not resolve global alias `T` in place of Other's parameter.
    let mut repeated = normalized(
        "data T = bool; data Id<X> = X; data Wrap<U> = Id<U>; data Box<T> { value: Wrap<T>; }",
    )
    .unwrap();
    let new_source = "data Other<T> { value: Wrap<T>; }";
    let tokens = source_files_to_tokens::Lexer::new(new_source)
        .tokenize()
        .unwrap();
    let later =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(77), &tokens).unwrap();
    repeated.extend_from(&later);
    let selection = ConstantSelection::new(&repeated, None, Vec::new()).unwrap();
    normalize(&mut repeated, &selection).unwrap();
    let data = repeated
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str(&repeated) == "Other" => Some(data),
            _ => None,
        })
        .unwrap();
    let syntax_trees::item::DataMember::Field(field) =
        &repeated.items.data_members(data.members)[0]
    else {
        panic!("field");
    };
    let TypeReferenceNode::Named(name) = repeated
        .type_references
        .type_reference(field.type_reference)
    else {
        panic!("caller type parameter");
    };
    assert_eq!(name.as_str(&repeated), "T");
    let settled = repeated.clone();
    normalize(&mut repeated, &selection).unwrap();
    assert_eq!(
        repeated, settled,
        "a settled source phase must not allocate more header work"
    );

    let mut global = normalized("data Real { value: u8; } data Global<X> = Real; data Outer<Real> = Global<u8>; data View { value: Outer<bool>; }").unwrap();
    let tokens = source_files_to_tokens::Lexer::new("data Later { value: Outer<bool>; }")
        .tokenize()
        .unwrap();
    let later =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(78), &tokens).unwrap();
    global.extend_from(&later);
    let selection = ConstantSelection::new(&global, None, Vec::new()).unwrap();
    normalize(&mut global, &selection).unwrap();
    let later = global
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str(&global) == "Later" => Some(data),
            _ => None,
        })
        .unwrap();
    let syntax_trees::item::DataMember::Field(field) = &global.items.data_members(later.members)[0]
    else {
        panic!("field");
    };
    let TypeReferenceNode::Named(name) =
        global.type_references.type_reference(field.type_reference)
    else {
        panic!("global nominal");
    };
    assert_eq!(
        name.as_str(&global),
        "Real",
        "a normalized global must not become Outer’s same-spelled formal"
    );
    for source in [
        "data Recursive<T> = Recursive<[T; 2]>;",
        "data First<T> = Second<T>; data Second<T> = First<[T; 2]>;",
    ] {
        let errors = normalized(source).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("recursive declaration cycle"))
        );
    }
}
