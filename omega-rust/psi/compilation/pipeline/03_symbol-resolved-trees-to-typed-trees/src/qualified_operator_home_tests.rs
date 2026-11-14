//! A token binding keeps its resolved semantic home without changing its lexical owner.

use std::sync::Arc;

fn typed(units: &[(&str, &str)]) -> typed_trees::TypedTrees {
    let mut sources = source::SourceMap::default();
    let mut parsed_units = Vec::with_capacity(units.len());
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([31; 32]).unwrap();
    for (path, text) in units {
        let id = sources
            .add_with_metadata(
                (*path).into(),
                (*text).to_owned(),
                "package".into(),
                Some(package),
                source::SourceOrigin::User,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let parsed = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
        parsed_units.push(parsed);
    }
    let syntax = parsed_units
        .into_iter()
        .reduce(|mut combined, unit| {
            combined.extend_from(&unit);
            combined
        })
        .unwrap();
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    crate::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn imported_domain_operator_keeps_its_exact_home_and_lexical_module() {
    let program = typed(&[
        (
            "package/units.omg",
            "module units; public domain u32::Distance;",
        ),
        (
            "package/main.omg",
            "use units; public machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { ((left as -Distance) +% (right as -Distance)) as +Distance }",
        ),
    ]);
    let domain = program.domain_definitions().first().unwrap();
    let binding = program.machine_token_bindings().first().unwrap();
    assert_eq!(binding.home_domain, domain.symbol);
    assert!(!program.symbols.symbol_module(binding.symbol).is_valid());
}
