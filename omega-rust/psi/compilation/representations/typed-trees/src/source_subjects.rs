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
    let file = program.symbols.symbol_provenance_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .ok_or("declaration without a source file")?;
    Ok(program.symbols.symbol_source_is_selected(symbol)
        && !file.origin.is_toolchain()
        && file.dependency_scope == purpose
        && subjects.contains(&file.package_identity))
}
