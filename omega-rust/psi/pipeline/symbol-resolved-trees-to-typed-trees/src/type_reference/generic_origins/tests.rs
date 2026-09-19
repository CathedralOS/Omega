use super::{
    Diagnostic, Handle, SymbolHandle, TypeReference, application, resolved, typed,
    validate_range_arguments,
};
thread_local! {
    pub(super) static ORIGIN_ROSTER_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static ORIGIN_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static DEFINITION_ROSTER_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

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
    let syntax = syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest::new(syntax),
    )
    .unwrap();
    syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap()
}

fn replay_with_roster_lookup(
    source: &resolved::SymbolResolvedTrees,
    typed: &typed::TypedTrees,
) -> Result<(), Diagnostic> {
    let mut replay = typed.clone();
    for (_, origin) in source.tables.types.generic_application_origins.iter() {
        let (name, symbol) = match source.child_type_reference(origin.instance) {
            TypeReference::Named { name, symbol } => (name, *symbol),
            TypeReference::Generic(value) => (&value.base_name, value.base_symbol),
            _ => {
                return Err(Diagnostic::error(
                    "generated instance lost its retained type",
                ));
            }
        };
        application(source, Some(&mut replay), name, symbol)?;
    }
    Ok(())
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

#[test]
fn nested_generic_arguments_preserve_contiguous_parent_rosters() {
    let source = resolve(
        "data Inner<T> { value: T; } data Pair<A, B> { first: A; second: B; }
         machine keep(value: Pair<u8, Inner<u64>>) -> Pair<u8, Inner<u64>> { value }",
    );
    let typed = crate::lower_symbol_resolved_trees(&source)
        .expect("nested argument lowering cannot interleave a parent roster");
    let pair = typed
        .data_definitions()
        .iter()
        .find(|definition| {
            definition.generic_instance.is_some()
                && typed.symbols.name(definition.symbol).starts_with("Pair<")
        })
        .expect("closed Pair instance");
    let typed::types::TypeReferenceNode::Generic { arguments, .. } = typed
        .type_reference_table
        .type_reference(pair.generic_instance.expect("retained Pair application"))
    else {
        panic!("retained generic application")
    };
    let [first, second] = typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("two parent arguments, excluding the nested child's arguments")
    };
    let typed::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*first)
    else {
        panic!("first argument is the authored scalar")
    };
    assert_eq!(typed.symbols.name(*symbol), "u8");
    let inner_symbol = match typed.type_reference_table.type_reference(*second) {
        typed::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
        typed::types::TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
        _ => panic!("second argument retains the nested carrier"),
    };
    assert!(typed.symbols.name(inner_symbol).starts_with("Inner"));
    replay_with_roster_lookup(&source, &typed)
        .expect("retained exact application identities replay");
}

#[test]
fn range_replay_visits_each_origin_once_and_agrees_with_use_lookup() {
    for count in [1, 16, 64] {
        let mut text = String::from(
            "data Box<T> { value: T; } machine bounded(value: u64[0..=10]) -> u64[0..=10] { value }",
        );
        for ordinal in 0..count {
            text.push_str(&format!(
                " data Unrelated{ordinal} {{ value: u32; }}
                  machine keep{ordinal}(value: Box<u64>) -> Box<u64> {{ value }}"
            ));
        }
        let source = resolve(&text);
        let typed = crate::lower_symbol_resolved_trees(&source).expect("range uses type check");
        let origin_count = source
            .tables
            .types
            .generic_application_origins
            .iter()
            .count();
        assert!(origin_count >= count * 2);
        ORIGIN_ROSTER_VISITS.set(0);
        ORIGIN_VALIDATIONS.set(0);
        DEFINITION_ROSTER_VISITS.set(0);
        validate_range_arguments(&source, &typed).expect("direct replay");
        assert_eq!(ORIGIN_ROSTER_VISITS.get(), origin_count);
        assert_eq!(ORIGIN_VALIDATIONS.get(), origin_count);
        assert_eq!(
            DEFINITION_ROSTER_VISITS.get(),
            source.data_definitions.len()
        );
        ORIGIN_ROSTER_VISITS.set(0);
        replay_with_roster_lookup(&source, &typed).expect("original replay traversal");
        assert_eq!(ORIGIN_ROSTER_VISITS.get(), origin_count * origin_count);
        // The standalone lookup continues to independently select the same uses.
        let mut replay = typed.clone();
        for (_, origin) in source.tables.types.generic_application_origins.iter() {
            let (name, symbol) = match source.child_type_reference(origin.instance) {
                TypeReference::Named { name, symbol } => (name, *symbol),
                TypeReference::Generic(value) => (&value.base_name, value.base_symbol),
                _ => panic!("generated carrier"),
            };
            assert_eq!(
                application(&source, Some(&mut replay), name, symbol).unwrap(),
                Some(source.child_type_reference(origin.application)),
            );
        }
    }
}

#[test]
fn range_replay_rejects_forged_and_duplicate_custody() {
    let baseline = resolve(
        "data Box<T> { value: T; } machine keep(value: Box<u64>) -> Box<u64> { value }
         machine bounded(value: u64[0..=10]) -> u64[0..=10] { value }",
    );
    let typed = crate::lower_symbol_resolved_trees(&baseline).unwrap();
    let origin = baseline
        .tables
        .types
        .generic_application_origins
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    for change in 0..4 {
        let mut source = baseline.clone();
        match change {
            0 => {
                let definition = source
                    .data_definitions
                    .iter()
                    .find(|definition| definition.generic_instance.is_some())
                    .unwrap()
                    .clone();
                source.data_definitions.push(definition);
            }
            1 => {
                source.tables.types.generic_application_origins.insert(
                    resolved::types::GenericApplicationOrigin {
                        instance: Handle::from_parts(
                            origin.instance.arena_index(),
                            origin.instance.generation() + 1,
                        ),
                        application: origin.application,
                    },
                );
            }
            2 => {
                let application = source
                    .tables
                    .declarations
                    .child_type_references
                    .insert(TypeReference::Unit);
                source.tables.types.generic_application_origins.insert(
                    resolved::types::GenericApplicationOrigin {
                        instance: origin.instance,
                        application,
                    },
                );
            }
            _ => {
                source.data_definitions.for_each_mut(|definition| {
                    if definition.generic_instance.is_some() {
                        definition.symbol = SymbolHandle::invalid();
                    }
                });
            }
        }
        assert!(
            validate_range_arguments(&source, &typed).is_err(),
            "corruption {change}"
        );
        assert_eq!(
            validate_range_arguments(&source, &typed),
            replay_with_roster_lookup(&source, &typed)
        );
    }
    let mut duplicate = baseline.clone();
    duplicate
        .tables
        .types
        .generic_application_origins
        .insert(origin);
    ORIGIN_VALIDATIONS.set(0);
    validate_range_arguments(&duplicate, &typed).expect("identical duplicate custody agrees");
    assert_eq!(
        ORIGIN_VALIDATIONS.get(),
        duplicate
            .tables
            .types
            .generic_application_origins
            .iter()
            .count()
    );
}

#[test]
fn range_replay_preserves_first_use_error_order_before_a_malformed_suffix() {
    let baseline = resolve(
        "data Box<T> { value: T; }
         machine first(value: Box<u64>) -> Box<u64> { value }
         machine second(value: Box<u32>) -> Box<u32> { value }
         machine bounded(value: u64[0..=10]) -> u64[0..=10] { value }",
    );
    let typed = crate::lower_symbol_resolved_trees(&baseline).unwrap();
    let first = baseline
        .tables
        .types
        .generic_application_origins
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    let last = baseline
        .tables
        .types
        .generic_application_origins
        .iter()
        .last()
        .unwrap()
        .1
        .clone();
    let mut source = baseline.clone();
    // A later conflicting member of the first group precedes the second group's
    // error in the old replay, even though the second group occurs earlier.
    source.tables.types.generic_application_origins.insert(
        resolved::types::GenericApplicationOrigin {
            instance: first.instance,
            application: last.application,
        },
    );
    *source
        .tables
        .declarations
        .child_type_references
        .get_mut(last.application) = TypeReference::Unit;
    for malformed_suffix in [false, true] {
        if malformed_suffix {
            source.tables.types.generic_application_origins.insert(
                resolved::types::GenericApplicationOrigin {
                    instance: Handle::invalid(),
                    application: first.application,
                },
            );
        }
        assert_eq!(
            validate_range_arguments(&source, &typed),
            replay_with_roster_lookup(&source, &typed)
        );
        assert!(validate_range_arguments(&source, &typed).is_err());
    }
}
