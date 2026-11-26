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
    ImportedProductMember, JoinedPath, SourceScopedTopLevelBinding, SymbolLookup, SymbolTable,
    SymbolTableAppender, SymbolTableBuilder, SymbolTableExtension, UnresolvedName,
    same_semantic_name,
};
/*
    The custody gate in front of the sealed intrinsic catalog: only a file the
    core supplied, outside any admitted package graph, can be classified,
    and its package-relative path, declaration path and exact source bytes
    must then match one catalog row. A user package that copies a bundled file
    and its names therefore receives no compiler custody.
*/
pub fn sealed_intrinsic_at(
    file: &source_admission::SourceFile,
    declaration: &str,
) -> Option<language_core::sealed_intrinsics::SealedIntrinsic> {
    bundled_declarations::sealed_intrinsics::SealedIntrinsicCatalog::classify(core_site(
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
    bundled_declarations::core_vocabulary::CoreVocabularyCatalog::classify(core_site(
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
    let site = core_site(file, "")?;
    bundled_declarations::core_vocabulary::CoreVocabularyCatalog::source(site.file, site.source)
}

fn core_site<'a>(
    file: &'a source_admission::SourceFile,
    declaration: &'a str,
) -> Option<bundled_declarations::sealed_intrinsics::SealedDeclarationSite<'a>> {
    if file.origin != source_admission::SourceOrigin::Toolchain || file.package_identity.is_some() {
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

#[cfg(test)]
mod installed_core_custody {
    use super::*;

    #[test]
    fn identical_files_from_the_build_library_do_not_supply_core() {
        use source_admission::{SourceFile, SourceOrigin};
        use std::path::PathBuf;

        let (relative, name, source) =
            bundled_declarations::sealed_intrinsics::hosted_process_exit_sources()[1];
        let occurrence = |root: &str, origin| {
            let root = PathBuf::from(root);
            SourceFile {
                source_id: source_text::SourceId(0),
                path: root.join(relative),
                package_root: root,
                package_identity: None,
                dependency_scope: source_provenance::DependencyScope::Build,
                origin,
                resolution_stratum: Default::default(),
                source: std::str::from_utf8(source).unwrap().into(),
            }
        };
        let core = occurrence("installation/core", SourceOrigin::Toolchain);
        let library = occurrence("installation/build", SourceOrigin::ToolchainLibrary);
        assert!(core_vocabulary_at(&core, name).is_some());
        assert!(core_vocabulary_at(&library, name).is_none());
        assert!(sealed_intrinsic_at(&core, "ProcessExit::exit_process").is_some());
        assert!(sealed_intrinsic_at(&library, "ProcessExit::exit_process").is_none());
    }
}
