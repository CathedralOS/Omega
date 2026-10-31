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
