//! Cross-package duplicate law: declarations in distinct packages share a
//! resolution scope only where an authored import edge exposes the name.
//! A toolchain or dependency closure therefore coexists with a package's
//! own same-named declarations, while an authored `use` keeps the rule that
//! imports do not shadow authored declarations.

use std::path::PathBuf;
use std::sync::Arc;

use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceId, SourceMap, SourceOrigin, SourceSpan, Span};

use crate::{
    SourceScopedTopLevelBinding, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable,
    SymbolTableBuilder,
};

fn package_identity(byte: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([byte; 32]).expect("nonzero package identity")
}

/// One package file declaring `ExtentRootProvider` beside one dependency or
/// toolchain file declaring the same leaf name.
fn collision_table(
    dependency_identity: Option<PackageKeyIdentity>,
    dependency_origin: SourceOrigin,
    bindings: Vec<SourceScopedTopLevelBinding>,
) -> (SymbolTable, [SymbolHandle; 2]) {
    let mut sources = SourceMap::default();
    let fixture_root = PathBuf::from("package-fixture");
    sources.add_with_metadata(
        fixture_root.join("main.omg"),
        String::from("ExtentRootProvider"),
        fixture_root,
        Some(package_identity(41)),
        SourceOrigin::User,
    );
    let dependency_root = PathBuf::from("dependency-core");
    sources.add_with_metadata(
        dependency_root.join("extent.omg"),
        String::from("ExtentRootProvider"),
        dependency_root,
        dependency_identity,
        dependency_origin,
    );
    let mut builder =
        SymbolTableBuilder::with_sources_and_top_level_bindings(Some(Arc::new(sources)), bindings);
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [SourceId(0), SourceId(1)].map(|source| {
            (
                SymbolKind::Trait,
                SymbolNameRef::Source(SourceSpan::new(source, Span::new(0, 18))),
            )
        }),
    ))
    .collect::<Vec<_>>();
    (
        builder.finish(),
        declarations.try_into().expect("two declarations"),
    )
}

#[test]
fn broad_module_import_does_not_expose_unqualified_leaves() {
    let (mut symbols, [fixture, dependency]) = collision_table(
        Some(package_identity(77)),
        SourceOrigin::User,
        vec![SourceScopedTopLevelBinding::module_import(
            SourceId(0),
            SourceId(1),
            "dep::extent",
            1,
        )],
    );
    symbols
        .register_source_module(
            SourceId(1),
            [("extent", SourceSpan::new(SourceId(1), Span::new(0, 6)))],
        )
        .expect("module registration");
    // `dep::extent` names the module; `extent::ExtentRootProvider` inside it
    // stays reachable only by its qualified spelling.
    assert!(symbols.source_scopes_separate(fixture, dependency));
}
