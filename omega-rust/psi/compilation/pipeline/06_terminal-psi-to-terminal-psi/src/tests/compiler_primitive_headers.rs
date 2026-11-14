//! A catalog identity must arrive through real bundled source and retain its
//! complete generic header. These controls do not claim application checking.

use language_core::sealed_intrinsics::{CollectionPrimitive, CompilerPrimitive};
use std::sync::Arc;

const INDEXING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/indexing.omg"
));

fn typed(
    text: &str,
    origin: source::SourceOrigin,
) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let mut sources = source::SourceMap::default();
    let source = sources
        .add_checked_instance(
            "toolchain/indexing.omg".into(),
            text.to_owned(),
            "toolchain".into(),
            None,
            origin,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Product,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text)
        .tokenize()
        .expect("catalog source tokenizes");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens)
        .expect("catalog source parses");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: vec![],
            retained_base: None,
        },
    )?;
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|error| vec![error])
}

#[test]
fn actual_generic_catalog_source_produces_complete_typed_schemas() {
    let program = typed(INDEXING, source::SourceOrigin::Toolchain).unwrap();
    assert_eq!(program.machines().len(), 3);
    assert!(
        program.operators().is_empty(),
        "no independent partial header"
    );
    for (name, operation, generics, parameters, contracts) in [
        ("read", CollectionPrimitive::Read, 1, 2, 1),
        ("range", CollectionPrimitive::Range, 1, 3, 2),
        ("view", CollectionPrimitive::ArrayView, 2, 1, 1),
    ] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        assert_eq!(program.machine_type_parameters(machine).len(), generics);
        assert_eq!(machine.lifetime_parameters.len(), 1);
        let [entry] = program.machine_states(machine) else {
            panic!("one real entry signature")
        };
        assert_eq!(program.state_parameters(entry).len(), parameters);
        assert_eq!(
            program.machine_contracts(machine).len() + program.state_contracts(entry).len(),
            contracts
        );
        let (actual, schema) = program
            .compiler_primitive_schema(machine.symbol)
            .unwrap()
            .expect("exact source custody");
        assert_eq!(actual, CompilerPrimitive::Collection(operation));
        assert!(program.type_identity_text(schema).is_some());
        if let Some(view) = program.machine_token_binding_by_symbol(machine.symbol) {
            assert_eq!(view.parameters, entry.parameters);
            assert_eq!(view.contracts, machine.contracts);
        }
    }
}

#[test]
fn a_copied_or_locally_edited_generic_catalog_cannot_supply_primitives() {
    let edited = format!("{INDEXING}\n// local edit\n");
    for (text, origin) in [
        (INDEXING, source::SourceOrigin::User),
        (edited.as_str(), source::SourceOrigin::Toolchain),
    ] {
        let errors =
            typed(text, origin).expect_err("a copied signature supplies no catalog authority");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("has no body")
                    || error.message.contains("sealed toolchain")),
            "{errors:?}"
        );
    }
}
