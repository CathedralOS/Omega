//! Identify standard declarations by resolved symbol and retained source owner.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::trait_definition::TraitDefinition;

/// Both checking and policy evaluation use the same exact declarations. A local
/// trait with the same spelling cannot change foreign-carrier admission.
pub fn standard_calling_traits(
    program: &TypedTrees,
) -> Option<(&TraitDefinition, &TraitDefinition)> {
    let vocabularies = standard_calling_vocabularies(program);
    let [vocabulary] = vocabularies.as_slice() else {
        return None;
    };
    Some(*vocabulary)
}

/// Every standard calling vocabulary the program loads, paired per declaring
/// source file. The toolchain copy and a dependency-supplied public copy are
/// each one exact vocabulary; a second byte-identical copy under a different
/// file is still a distinct declaration pair, so consumers that must select
/// the vocabulary a boundary references join on the relationship symbol
/// rather than the first match.
pub fn standard_calling_vocabularies<'program>(
    program: &'program TypedTrees,
) -> Vec<(&'program TraitDefinition, &'program TraitDefinition)> {
    let mut per_file: std::collections::BTreeMap<
        usize,
        (Option<&TraitDefinition>, Option<&TraitDefinition>),
    > = std::collections::BTreeMap::new();
    for definition in program.traits().iter() {
        if definition.is_boundary || !has_calling_vocabulary_source(program, definition.symbol) {
            continue;
        }
        let Some(file) = program
            .symbols
            .symbol_source_span(definition.symbol)
            .and_then(|span| program.symbols.source_file(span))
        else {
            continue;
        };
        let entry = per_file.entry(file.source_id.0).or_default();
        match definition.name.as_str() {
            "Calling" => entry.0 = Some(definition),
            "CallingPolicy" => entry.1 = Some(definition),
            _ => {}
        }
    }
    per_file
        .into_values()
        .filter_map(|vocabulary| match vocabulary {
            (Some(calling), Some(policy)) => (program.trait_type_parameters(calling).len() == 1
                && program.trait_machine_signatures(calling).is_empty())
            .then_some((calling, policy)),
            _ => None,
        })
        .collect()
}

/// The standard calling vocabulary is a singleton per program. The same file
/// may load into separate scopes legally, but a second byte-identical copy
/// under a different path is a duplicate declaration of every name the
/// vocabulary publishes; report each colliding `data` with the ordinary
/// duplicate-declaration diagnostic.
pub fn validate_calling_vocabulary_uniqueness(
    program: &TypedTrees,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let mut paths = std::collections::BTreeSet::new();
    for definition in program.traits() {
        if definition.name.as_str() != "Calling"
            || definition.is_boundary
            || !has_calling_vocabulary_source(program, definition.symbol)
        {
            continue;
        }
        if let Some(source) = program
            .symbols
            .symbol_source_span(definition.symbol)
            .and_then(|span| program.symbols.source_file(span))
        {
            paths.insert(source.path.clone());
        }
    }
    let duplicate_paths: Vec<_> = paths.into_iter().skip(1).collect();
    if duplicate_paths.is_empty() {
        return;
    }
    for data_definition in program.data_definitions() {
        let duplicate = program
            .symbols
            .symbol_source_span(data_definition.symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|source| duplicate_paths.contains(&source.path));
        if duplicate {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "duplicate data `{}`",
                data_definition.name
            )));
        }
    }
}

fn has_calling_vocabulary_source(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    if has_toolchain_declaration_source(program, symbol, "calling.omg") {
        return true;
    }
    // Package-aware compilation can supply the public vocabulary as an ordinary
    // dependency rather than a toolchain source. Recognize that exact vocabulary,
    // not a package's filename or a similarly named pair of empty traits. The
    // selected policy body still runs and its complete result is checked.
    const CALLING_VOCABULARY: &str = include_str!("../../../../../source/library/std/calling.omg");
    program
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .is_some_and(|source| source.source.as_ref() == CALLING_VOCABULARY)
}

pub(crate) fn is_core_vector(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    program.data_definitions().iter().any(|definition| {
        definition.symbol == symbol
            && definition.name.as_str() == "Vec"
            && has_toolchain_declaration_source(program, symbol, "vec.omg")
    })
}

pub(crate) fn is_core_vector_surface_state(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> bool {
    // The retained core vector machines are source-visible signatures for
    // boundary operations, not empty runtime implementations. Do not infer an
    // effect-free body from these placeholders. Ordinary same-spelled user
    // methods retain normal body inference.
    has_toolchain_declaration_source(program, machine.symbol, "vec.omg")
        && program.machine_states(machine).len() == 1
        && program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
}

fn has_toolchain_declaration_source(
    program: &TypedTrees,
    symbol: SymbolHandle,
    relative_path: &str,
) -> bool {
    if !symbol.is_valid() {
        return false;
    }
    program
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .is_some_and(|source| {
            source.origin == source::SourceOrigin::Toolchain
                && source.path.strip_prefix(&source.package_root).ok()
                    == Some(std::path::Path::new(relative_path))
        })
}
