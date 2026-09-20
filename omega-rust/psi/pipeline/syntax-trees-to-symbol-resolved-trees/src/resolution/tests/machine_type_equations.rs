use crate::{ExtensionRequest, ResolutionRequest, resolve, resolve_extension};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use tokens_to_syntax_trees::parse_syntax_trees_with_id;

#[test]
fn unused_machine_equations_still_validate_type_and_value_kinds() {
    let tokens =
        Lexer::new("machine bad<Type, const Count: u64>() -> u64 where Type == Count { 0 }")
            .tokenize()
            .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let diagnostics = resolve(ResolutionRequest::new(&syntax)).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("equation mixes type and value")),
        "{diagnostics:?}"
    );
}

#[test]
fn retained_machine_equation_cannot_be_discarded_by_extension_call() {
    let base_source = "machine capacity<Length, const Capacity: u64>() -> u64 where Length == u64[0..=Capacity] { Capacity }";
    let extension_source = "machine recovered() -> u64 { capacity<u64[0..=256], 512>() }";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/equation.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_syntax =
        parse_syntax_trees_with_id(base_id, &Lexer::new(base_source).tokenize().unwrap()).unwrap();
    let sources = Arc::new(sources);
    let base = resolve(ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(sources.clone()),
        top_level_bindings: Vec::new(),
    })
    .unwrap();
    assert!(
        base.machines
            .iter()
            .any(|machine| machine.has_structural_type_equations)
    );
    let extension = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source).tokenize().unwrap(),
    )
    .unwrap();
    let diagnostics = resolve_extension(ExtensionRequest {
        base,
        syntax: &extension,
        sources,
        top_level_bindings: Vec::new(),
    })
    .unwrap_err();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("retained machine structural equation")),
        "{diagnostics:?}"
    );
}

#[test]
fn caller_type_binder_cannot_be_reinterpreted_as_same_named_global_type() {
    let source = "data Element {} machine chosen<Type>() -> u64 where Type == Element { 0 }
        machine forward<Element>() -> u64 { chosen<Element>() }";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let result = resolve(ResolutionRequest::new(&syntax));
    assert!(
        result.is_err(),
        "a caller binder must remain open despite a same-named global data declaration"
    );
}

#[test]
fn nested_caller_type_binder_cannot_be_reinterpreted_as_global_array_element() {
    let source = "data Element {} machine chosen<Array>() -> u64 where Array == [Element; 7] { 0 }
        machine forward<Element>() -> u64 { chosen<[Element; 7]>() }";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let result = resolve(ResolutionRequest::new(&syntax));
    assert!(
        result.is_err(),
        "a nested caller binder remains open in the selected type tree"
    );
}

#[test]
fn machine_equations_do_not_escape_through_implicit_operator_or_conformance_supply() {
    for source in [
        "machine + chosen<Type>(left: u64, right: u64) -> u64 where Type == u64 { 0 }",
        "trait Operation { machine chosen() -> u64; } machine chosen<Type>() -> u64 satisfies Operation::chosen where Type == u64 { 0 }",
    ] {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let diagnostics = resolve(ResolutionRequest::new(&syntax)).unwrap_err();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("explicit calls; operator and conformance")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn unused_attached_equations_still_validate_type_and_value_kinds() {
    let source =
        "data Item {} machine Item::bad<Type, const Count: u64>() -> u64 where Type == Count { 0 }";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let diagnostics = resolve(ResolutionRequest::new(&syntax)).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("equation mixes type and value")),
        "{diagnostics:?}"
    );
}

#[test]
fn bare_attached_selection_retains_its_exact_owner_and_member_roster() {
    use symbol_resolved_trees::expression::ExpressionNode;
    for (module, selection) in [
        ("", "Buffer::capacity"),
        ("module scope;", "scope::Buffer::capacity"),
    ] {
        let source = format!(
            "{module} data Buffer {{}} data Other {{}}
             machine Buffer::capacity() -> u64 {{ 7 }}
             machine Other::capacity() -> u64 {{ 9 }}
             machine recovered() -> u64 {{ {selection} }}"
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
        let expressions = &resolved.tables.bodies.expressions;
        let path = expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::Name(path) if path.members.count() > 1 => Some(path),
                _ => None,
            })
            .expect("bare attached selection");
        let selected = &resolved.machines[0];
        assert_eq!(resolved.symbols.get(path.symbol).parent, selected.symbol);
        let members = expressions.name_path_member_symbols(path.member_symbols);
        assert_eq!(members.len(), selection.split("::").count());
        assert!(members.iter().all(|symbol| symbol.is_valid()));
        assert_eq!(members[members.len() - 2], selected.attached_data_symbol);
        assert_eq!(members.last(), Some(&path.symbol));
    }
}

#[test]
fn bare_attached_selection_cannot_reopen_a_shadowed_owner() {
    use symbol_resolved_trees::expression::ExpressionNode;
    for (caller, expected_kind) in [
        (
            "machine recovered(Buffer: u64) -> u64 { Buffer::capacity }",
            symbols::SymbolKind::Parameter,
        ),
        (
            "machine recovered<Buffer>() -> u64 { Buffer::capacity }",
            symbols::SymbolKind::TypeParameter,
        ),
        (
            "machine recovered() -> u64 { let Buffer: u64 = 0; Buffer::capacity }",
            symbols::SymbolKind::Local,
        ),
    ] {
        let source = format!(
            "data Buffer {{}} machine Buffer::capacity<Type>() -> u64 where Type == u64 {{ 7 }} {caller}"
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
        let path = resolved
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::Name(path) if path.members.count() == 2 => Some(path),
                _ => None,
            })
            .expect("shadowed qualified selection");
        assert_eq!(resolved.symbols.get(path.head_symbol).kind, expected_kind);
        assert!(
            !path.symbol.is_valid(),
            "an unresolved lexical suffix cannot select the global method"
        );
    }
}
