//! Source-owned declaration lookup, construction and exact-source correspondence.
mod builtin;
mod retained_declaration;
mod storage;
mod table;
#[cfg(test)]
mod tests;
pub use builtin::{
    builtin_function_symbols, builtin_type_member_symbols, builtin_type_symbols,
    fixed_core_declaration_symbols, is_builtin_type_spelling,
};
pub use retained_declaration::{SourceSpanCandidates, rejoin_declaration, retain_declaration};
pub use storage::{SymbolHierarchy, SymbolRows};
use symbols::*;
pub use table::{
    JoinedPath, SourceScopedTopLevelBinding, SymbolLookup, SymbolTable, SymbolTableAppender,
    SymbolTableBuilder, SymbolTableExtension, UnresolvedName, same_semantic_name,
};
/*
    The custody gate in front of the sealed intrinsic catalog: only a file the
    toolchain supplied, outside any admitted package graph, can be classified,
    and its package-relative path, declaration path and exact source bytes
    must then match one catalog row. A user package that copies a bundled file
    and its names therefore receives no compiler custody.
*/
pub fn sealed_intrinsic_at(
    file: &source_admission::SourceFile,
    declaration: &str,
) -> Option<language_core::sealed_intrinsics::SealedIntrinsic> {
    bundled_declarations::sealed_intrinsics::SealedIntrinsicCatalog::classify(toolchain_site(
        file,
        declaration,
    )?)
}

/*
    The same gate in front of the core vocabulary catalog. Recognizing a core
    declaration by this route grants no custody: it only tells checking that
    the declaration is the bundled one its rules are written against.
*/
pub fn core_vocabulary_at(
    file: &source_admission::SourceFile,
    declaration: &str,
) -> Option<language_core::core_vocabulary::CoreVocabulary> {
    bundled_declarations::core_vocabulary::CoreVocabularyCatalog::classify(toolchain_site(
        file,
        declaration,
    )?)
}

/*
    The bundled core source `file` is, behind the same toolchain gate. This is
    recognition of the file only; the caller still checks which declaration.
*/
pub fn core_source_at(
    file: &source_admission::SourceFile,
) -> Option<language_core::core_vocabulary::CoreSource> {
    let site = toolchain_site(file, "")?;
    bundled_declarations::core_vocabulary::CoreVocabularyCatalog::source(site.file, site.source)
}

fn toolchain_site<'a>(
    file: &'a source_admission::SourceFile,
    declaration: &'a str,
) -> Option<bundled_declarations::sealed_intrinsics::SealedDeclarationSite<'a>> {
    if !file.origin.is_toolchain() || file.package_identity.is_some() {
        return None;
    }
    Some(
        bundled_declarations::sealed_intrinsics::SealedDeclarationSite {
            file: file.path.strip_prefix(&file.package_root).ok()?,
            declaration,
            source: file.source.as_bytes(),
        },
    )
}

pub fn is_build_protocol(symbols: &SymbolTable, declaration: SymbolHandle, name: &str) -> bool {
    if !declaration.is_valid() || symbols.name(declaration) != name {
        return false;
    }
    symbols
        .symbol_source_span(declaration)
        .and_then(|span| symbols.source_file(span))
        .is_some_and(|source| {
            source.origin == source_admission::SourceOrigin::Toolchain
                && source.path == std::path::Path::new("<build-prelude>")
        })
}
