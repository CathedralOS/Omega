use super::sourced_symbol_table;
use crate::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn resolves_managed_authored_symbol_package_identity() {
    let package_identity =
        PackageKeyIdentity::from_digest([1; 32]).expect("nonzero package identity");
    let (symbols, authored) = sourced_symbol_table([(SourceOrigin::User, Some(package_identity))]);

    assert_eq!(
        symbols.symbol_package_identity(authored[0]),
        Some(package_identity)
    );
}

#[test]
fn source_free_structural_children_inherit_authored_parent_provenance() {
    let package_identity =
        PackageKeyIdentity::from_digest([4; 32]).expect("nonzero package identity");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("package/main.omg"),
            String::from("machine"),
            PathBuf::from("package"),
            Some(package_identity),
            SourceOrigin::User,
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let machine = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [(
            SymbolKind::Machine,
            SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(0, 7))),
        )],
    ))
    .next()
    .expect("authored machine");
    let state = SymbolTableBuilder::child_handles(builder.insert_children(
        machine,
        [(SymbolKind::State, SymbolNameRef::Static("entry"))],
    ))
    .next()
    .expect("implicit state");
    let symbols = builder.finish();

    assert_eq!(
        symbols.symbol_package_identity(state),
        Some(package_identity)
    );
    assert_eq!(
        symbols.symbol_source_origin(state),
        Some(SourceOrigin::User)
    );
}

#[test]
fn owned_semantic_symbol_name_retains_authored_provenance() {
    let package_identity =
        PackageKeyIdentity::from_digest([5; 32]).expect("nonzero package identity");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("package/main.omg"),
            String::from("Ready"),
            PathBuf::from("package"),
            Some(package_identity),
            SourceOrigin::User,
        )
        .source_id;
    let source_span = SourceSpan::new(source_id, Span::new(0, 5));
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let domain = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [(
            SymbolKind::Domain,
            SymbolNameRef::OwnedSource {
                value: "Packet::Ready",
                source_span,
            },
        )],
    ))
    .next()
    .expect("authored semantic domain");
    let symbols = builder.finish();

    assert_eq!(symbols.name(domain), "Packet::Ready");
    assert_eq!(symbols.symbol_source_span(domain), Some(source_span));
    assert_eq!(
        symbols.symbol_package_identity(domain),
        Some(package_identity)
    );
}

#[test]
fn unmanaged_and_toolchain_symbols_have_no_package_identity() {
    let package_identity =
        PackageKeyIdentity::from_digest([2; 32]).expect("nonzero package identity");
    let (symbols, authored) = sourced_symbol_table([
        (SourceOrigin::User, None),
        (SourceOrigin::Toolchain, Some(package_identity)),
    ]);

    assert_eq!(symbols.symbol_package_identity(authored[0]), None);
    assert_eq!(symbols.symbol_package_identity(authored[1]), None);
}

#[test]
fn generated_symbols_inherit_authored_package_identity() {
    let package_identity =
        PackageKeyIdentity::from_digest([3; 32]).expect("nonzero package identity");
    let (mut symbols, authored) =
        sourced_symbol_table([(SourceOrigin::User, Some(package_identity))]);
    let generated =
        symbols.insert_generated_root_from(authored[0], SymbolKind::Machine, "generated");
    let generated_child = SymbolTableBuilder::child_handles(
        symbols.insert_generated_children(generated, [(SymbolKind::State, "entry")]),
    )
    .next()
    .expect("generated state");

    assert_eq!(symbols.symbol_package_identity(symbols.root()), None);
    assert_eq!(
        symbols.symbol_package_identity(generated),
        Some(package_identity)
    );
    assert_eq!(
        symbols.symbol_package_identity(generated_child),
        Some(package_identity)
    );
    assert_eq!(
        symbols.symbol_source_origin(generated_child),
        Some(SourceOrigin::User)
    );
    assert!(symbols.same_symbol_source_package(authored[0], generated_child));
}

#[test]
fn generated_toolchain_symbols_retain_toolchain_origin_without_package_identity() {
    let (mut symbols, authored) = sourced_symbol_table([(SourceOrigin::Toolchain, None)]);
    let generated = symbols.insert_generated_root_from(
        authored[0],
        SymbolKind::Machine,
        "generated_toolchain_machine",
    );

    assert_eq!(
        symbols.symbol_source_origin(generated),
        Some(SourceOrigin::Toolchain)
    );
    assert_eq!(symbols.symbol_package_identity(generated), None);
}
