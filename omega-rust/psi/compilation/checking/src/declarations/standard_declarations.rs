//! Identify standard declarations by resolved symbol and retained source owner.
//!
//! Calling vocabulary recognition enables policy evaluation, not ABI admission.
//! Recognize its resolved interface rather than the bundled file's bytes: a
//! dependency may retain the same contract across source refactors. The policy
//! evaluator still constructs the normalized input, decodes the closed result
//! vocabulary, and independently validates the resulting plan.

use symbols::SymbolHandle;
use typed_trees::trait_definition::TraitDefinition;

/// Shared typed declaration readers preserve the original interface owner.
pub fn boundary_has_explicit_calling_policy(
    program: &dyn crate::CheckingInput,
    definition: &TraitDefinition,
) -> bool {
    typed_trees::calling_vocabulary::boundary_has_explicit_calling_policy(
        program.typed_program(),
        definition,
    )
}

pub fn boundary_requirement_has_explicit_calling_policy(
    program: &dyn crate::CheckingInput,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
) -> bool {
    typed_trees::calling_vocabulary::boundary_requirement_has_explicit_calling_policy(
        program.typed_program(),
        declaring_trait,
        requirement,
    )
}

/// Every standard calling vocabulary the program loads, paired per declaring
/// source file. The toolchain copy and a dependency-supplied public copy are
/// each one exact vocabulary; another copy under a different
/// file is still a distinct declaration pair, so consumers that must select
/// the vocabulary a boundary references join on the relationship symbol
/// rather than the first match.
pub fn standard_calling_vocabularies(
    program: &dyn crate::CheckingInput,
) -> Vec<(&TraitDefinition, &TraitDefinition)> {
    typed_trees::calling_vocabulary::standard_calling_vocabularies(program.typed_program())
}

/// The standard calling vocabulary is a singleton per program. The same file
/// may load into separate scopes legally, but a second compatible copy
/// under a different path is a duplicate declaration of every name the
/// vocabulary publishes; report each colliding `data` with the ordinary
/// duplicate-declaration diagnostic.
pub fn validate_calling_vocabulary_uniqueness(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let mut paths = std::collections::BTreeSet::new();
    for (definition, _) in standard_calling_vocabularies(program) {
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

pub(crate) fn is_core_vector(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> bool {
    program.data_definitions().iter().any(|definition| {
        definition.symbol == symbol
            && definition.name.as_str() == "Vec"
            && has_toolchain_declaration_source(program, symbol, "vec.omg")
    })
}

pub(crate) fn is_core_vector_surface_state(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
