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
    crate::front_end::resolved_program_with_generic_data(
        "data First<T> { value: T; } data Second<T> { value: T; }
         machine first(value: First<u64>) -> First<u64> { value }
         machine second(value: Second<u64>) -> Second<u64> { value }",
    )
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
        let source = crate::front_end::resolved_program_with_generic_data(text);
        crate::lower_symbol_resolved_trees(&source).expect(text);
    }
}

#[test]
fn range_replay_rejects_forged_and_duplicate_custody() {
    let baseline = crate::front_end::resolved_program_with_generic_data(
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
