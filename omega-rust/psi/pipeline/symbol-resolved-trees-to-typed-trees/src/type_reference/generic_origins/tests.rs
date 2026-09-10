use super::*;

fn source() -> resolved::SymbolResolvedTrees {
    resolve(
        "data First<T> { value: T; } data Second<T> { value: T; }
         machine first(value: First<u64>) -> First<u64> { value }
         machine second(value: Second<u64>) -> Second<u64> { value }",
    )
}

fn resolve(text: &str) -> resolved::SymbolResolvedTrees {
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let syntax = syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax).unwrap();
    syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap()
}

#[test]
fn generated_instance_replays_exact_application_base_and_arguments() {
    let baseline = source();
    crate::lower_symbol_resolved_trees(&baseline).expect("unchanged generic occurrences");
    let origin = baseline
        .tables
        .types
        .generic_application_origins
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    let TypeReference::Generic(original) = baseline.child_type_reference(origin.application) else {
        panic!("generic application");
    };
    let other = baseline
        .data_definitions
        .iter()
        .find(|data| data.generic_instance.is_none() && data.symbol != original.base_symbol)
        .unwrap()
        .symbol;
    let different_argument = baseline
        .symbols
        .child_handles(baseline.symbols.root())
        .unwrap()
        .find(|symbol| baseline.symbols.name(*symbol) == "u32")
        .unwrap();
    for change in 0..3 {
        let mut corrupted = baseline.clone();
        let mut changed = original.clone();
        if change == 0 {
            changed.base_symbol = other;
        } else {
            changed.arguments = corrupted
                .tables
                .declarations
                .child_type_references
                .insert_many([TypeReference::Named {
                    symbol: different_argument,
                    name: resolved::name::DiagnosticName::generated("u32"),
                }]);
        }
        if change == 2 {
            let application = corrupted
                .tables
                .declarations
                .child_type_references
                .insert(TypeReference::Generic(changed));
            corrupted.tables.types.generic_application_origins.insert(
                resolved::types::GenericApplicationOrigin {
                    instance: origin.instance,
                    application,
                },
            );
        } else {
            *corrupted
                .tables
                .declarations
                .child_type_references
                .get_mut(origin.application) = TypeReference::Generic(changed);
        }
        let diagnostics = match crate::lower_symbol_resolved_trees(&corrupted) {
            Ok(_) => panic!("corrupted generic origin {change} accepted"),
            Err(diagnostics) => diagnostics,
        };
        assert!(
            diagnostics
                .message
                .contains("exact retained instance origin"),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn missing_generated_application_origin_rejects() {
    let mut source = source();
    source.tables.types.generic_application_origins = Default::default();
    assert!(crate::lower_symbol_resolved_trees(&source).is_err());
}

#[test]
fn inferred_literals_and_lifetime_arguments_retain_their_actual_owners() {
    for text in [
        "data Box<T> { value: T; } machine make() -> Box<u64> { let value: Box<u64> = Box { value: 1u64 }; value }",
        "data View<'a, T> { value: &'a T; } machine keep<'b>(value: View<'b, u64>) -> View<'b, u64> { value }",
        "data Inner<T> { value: T; } data Outer<T> { value: T; } machine keep(value: Outer<Inner<u64>>) -> Outer<Inner<u64>> { value }",
    ] {
        let source = resolve(text);
        crate::lower_symbol_resolved_trees(&source).expect(text);
    }
}
