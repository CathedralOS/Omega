//! Decide which source declarations a caller's subject request names.
//! This reads package, generation and Build/Product context metadata only;
//! membership is neither a body judgment nor permission to execute source.

use crate::TypedTrees;
use semantic_vocabulary::PackageKeyIdentity;
use source::DependencyScope;
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
        || (subjects.contains(&None) && file.origin == source::SourceOrigin::User);
    Ok(program.symbols.symbol_source_is_selected(symbol)
        && !file.origin.is_toolchain()
        && file.dependency_scope == purpose
        && requested)
}
