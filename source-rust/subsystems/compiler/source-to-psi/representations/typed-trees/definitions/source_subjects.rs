//! Decide which source declarations a caller's subject request names.
//! This reads package, generation and Build/Product context metadata only;
//! membership is neither a body judgment nor permission to execute source.

use crate::source_state::TypedTrees;
use semantic_vocabulary::PackageKeyIdentity;
use source_provenance::DependencyScope;
use symbols::SymbolHandle;

pub fn contains(
    program: &TypedTrees,
    symbol: SymbolHandle,
    subjects: &[Option<PackageKeyIdentity>],
    purpose: DependencyScope,
) -> Result<bool, &'static str> {
    if !program.symbols.symbols().nodes().is_valid(symbol) {
        return Err("declaration has no current source symbol");
    }
    let file = program
        .symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .ok_or("declaration without a source file")?;
    // A standalone source is given a package identity derived from its own
    // files once its Build has run, so the request for standalone source
    // (`None`) names its user files by origin, not by identity. No package
    // graph is admitted beside a standalone source, so those are its files.
    let requested = subjects.contains(&file.package_identity)
        || (subjects.contains(&None) && file.origin == source_admission::SourceOrigin::User);
    Ok(program.symbols.symbol_source_is_selected(symbol)
        && !file.origin.is_toolchain()
        && file.dependency_scope == purpose
        && (purpose != DependencyScope::Build || program.build_invocation_entry
            .is_none_or(|entry| entry.source_id == file.source_id))
        && requested)
}

/// Locate the selected Build machine in the receiving package's admitted Build sources.
pub fn rejoin_build_activation(
    program: &TypedTrees,
    subject: Option<PackageKeyIdentity>,
    declaration: &source_admission::RetainedDeclaration,
) -> Result<SymbolHandle, &'static str> {
    let symbol = source_symbol_table::source_symbols::rejoin_declaration(
        declaration,
        &program.symbols,
        program
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .filter(|&symbol| program.symbols.symbol_source_is_selected(symbol)),
    )?;
    // The built-in entry belongs to the toolchain, even after a package adopts its sources.
    // The declaration above has already rejoined the receiving table by exact source and span.
    let file = declaration.source();
    let default_path = std::path::Path::new(source_admission::source_identity::DEFAULT_BUILD_PATH);
    let toolchain_default = file.origin == source_admission::SourceOrigin::Toolchain
        && file.path == default_path
        && file.package_root == default_path
        && file.package_identity.is_none()
        && file.dependency_scope == DependencyScope::Build;
    if !toolchain_default && !contains(program, symbol, &[subject], DependencyScope::Build)? {
        return Err("selected Build activation is outside the package's admitted Build sources");
    }
    Ok(symbol)
}
