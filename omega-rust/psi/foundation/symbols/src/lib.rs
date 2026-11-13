/*
    Target-neutral symbol identities, names, paths and hierarchy storage.

    We give every declaration one durable `SymbolHandle`; names and paths are
    lookup and display metadata around it. `table` owns the hierarchy arena
    that scoped lookup walks, together with the builder, appender and
    extension that grow it, the per-source module and import namespaces, and
    the lazily built name indices. `builtin` seeds the language's source-free
    types, members and functions at fixed root-child ordinals, and
    `retained_declaration` carries an already selected declaration across
    independently allocated source contexts.
*/

/*
    We keep identity separate from spelling because every later stage compares
    declarations, not strings: once resolution finishes, a string name never
    stands in for a symbol. The table is an arena of dense handles with child
    spans rather than a map of names, so lookup walks contiguous storage and
    an extension can append generated source without rebuilding the base.
    Callers that do need hashing key `SymbolMap` and `SymbolKeyMap` by handle
    through the cheap `fast_map` hasher instead of SipHash.
*/

/*
    This is a Psi foundation crate with no pipeline stage of its own. Stage
    02 (`syntax-trees-to-symbol-resolved-trees`) builds the table and assigns
    every handle; the symbol-resolved, typed and checked tree representations
    store those handles, and stages 03 through 08, validation, proof,
    build-time evaluation, the interpreters and Omega's realization and
    package crates read them. Start at `table`; `symbol` holds the handle,
    name, path and kind types it stores.
*/

#![forbid(unsafe_code)]

mod builtin;
mod fast_map;
mod retained_declaration;
mod symbol;
mod table;

/*
    The custody gate in front of the sealed intrinsic catalog: only a file the
    toolchain supplied, outside any admitted package graph, can be classified,
    and its package-relative path, declaration path and exact source bytes
    must then match one catalog row. A user package that copies a bundled file
    and its names therefore receives no compiler custody.
*/
pub fn sealed_intrinsic_at(
    file: &source::SourceFile,
    declaration: &str,
) -> Option<language_core::sealed_intrinsics::SealedIntrinsic> {
    language_core::sealed_intrinsics::SealedIntrinsicCatalog::classify(toolchain_site(
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
    file: &source::SourceFile,
    declaration: &str,
) -> Option<language_core::core_vocabulary::CoreVocabulary> {
    language_core::core_vocabulary::CoreVocabularyCatalog::classify(toolchain_site(
        file,
        declaration,
    )?)
}

/*
    The bundled core source `file` is, behind the same toolchain gate. This is
    recognition of the file only; the caller still checks which declaration.
*/
pub fn core_source_at(
    file: &source::SourceFile,
) -> Option<language_core::core_vocabulary::CoreSource> {
    let site = toolchain_site(file, "")?;
    language_core::core_vocabulary::CoreVocabularyCatalog::source(site.file, site.source)
}

fn toolchain_site<'a>(
    file: &'a source::SourceFile,
    declaration: &'a str,
) -> Option<language_core::sealed_intrinsics::SealedDeclarationSite<'a>> {
    if !file.origin.is_toolchain() || file.package_identity.is_some() {
        return None;
    }
    Some(language_core::sealed_intrinsics::SealedDeclarationSite {
        file: file.path.strip_prefix(&file.package_root).ok()?,
        declaration,
        source: file.source.as_bytes(),
    })
}

pub use builtin::{
    BUILTIN_TYPE_COUNT, BuiltinFunction, BuiltinType, BuiltinTypeAtom, BuiltinTypeMember,
    FixedCoreDeclaration, builtin_function_symbols, builtin_type_member_symbols,
    builtin_type_symbols, fixed_core_declaration_symbols,
};
pub use fast_map::{BuildSymbolHasher, SymbolHasher, SymbolKeyMap, SymbolMap};
pub use retained_declaration::{RetainedDeclaration, SourceSpanCandidates};
pub use symbol::kind::SymbolKind;
pub use symbol::name::{SymbolName, SymbolNameRef};
pub use symbol::path::SymbolPath;
pub use symbol::{Symbol, SymbolHandle, SymbolNameHandle, SymbolSpan};
pub use table::{
    JoinedPath, SourceScopedTopLevelBinding, SymbolLookup, SymbolTable, SymbolTableAppender,
    SymbolTableBuilder, SymbolTableExtension, same_semantic_name,
};

#[cfg(test)]
mod tests;
