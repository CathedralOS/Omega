//! Identity of the toolchain-owned project-request protocol.
//!
//! The name locates vocabulary only within the exact retained toolchain source.
//! An authored boundary with the same spelling remains an ordinary service.
//! Classification grants neither a host binding nor authority over a Build value.

pub fn is_project_request_protocol(
    symbols: &symbols::SymbolTable,
    declaration: symbols::SymbolHandle,
) -> bool {
    is_build_protocol(symbols, declaration, "BuildRequestOperations")
}

/// Locate protocol vocabulary only in the retained toolchain Build prelude.
pub fn is_build_protocol(
    symbols: &symbols::SymbolTable,
    declaration: symbols::SymbolHandle,
    name: &str,
) -> bool {
    if !declaration.is_valid() || symbols.name(declaration) != name {
        return false;
    }
    symbols
        .symbol_source_span(declaration)
        .and_then(|span| symbols.source_file(span))
        .is_some_and(|source| {
            source.origin == source::SourceOrigin::Toolchain
                && source.path == std::path::Path::new("<build-prelude>")
        })
}
