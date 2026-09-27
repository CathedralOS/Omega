//! Identify standard declarations by resolved symbol and retained source owner.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::trait_definition::TraitDefinition;

/// Both checking and policy evaluation use the same exact declarations. A local
/// trait with the same spelling cannot change foreign-carrier admission.
pub fn standard_calling_traits(
    program: &TypedTrees,
) -> Option<(&TraitDefinition, &TraitDefinition)> {
    let exact = |name: &str| {
        let mut declarations = program.traits().iter().filter(|definition| {
            definition.name.as_str() == name
                && !definition.is_boundary
                && has_calling_vocabulary_source(program, definition.symbol)
        });
        let declaration = declarations.next()?;
        declarations.next().is_none().then_some(declaration)
    };
    let calling = exact("Calling")?;
    let policy = exact("CallingPolicy")?;
    (program.trait_type_parameters(calling).len() == 1
        && program.trait_machine_signatures(calling).is_empty())
    .then_some((calling, policy))
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
