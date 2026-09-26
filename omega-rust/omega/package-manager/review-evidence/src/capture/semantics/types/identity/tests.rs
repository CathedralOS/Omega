use super::signature_type_identity;
use source::{SourceMap, SourceOrigin, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceNode;

#[test]
fn scratch_identity_preserves_source_custody_and_normalizes_local_handles() {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("toolchain/core/card.omg"),
            String::from("Card"),
            PathBuf::from("toolchain/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let card = SymbolTableBuilder::child_handles(symbols.insert_children(
        root,
        [(
            SymbolKind::Data,
            SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(0, 4))),
        )],
    ))
    .next()
    .unwrap();
    let mut original = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let named = original
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: card,
            name: Identifier::generated("Card"),
        });
    let source_identities = [(source_id, [7; 32])];
    let original_identity = signature_type_identity(
        &original,
        &source_identities,
        named,
        &[],
        &[],
        &[],
        &[],
        false,
    )
    .unwrap();
    let original_types = original.type_reference_table.clone();
    let mut scratch = original.clone();
    let node = scratch.type_reference_table.type_reference(named).clone();
    let duplicate = scratch.type_reference_table.insert(node);
    assert_ne!(named, duplicate);
    assert_eq!(
        original_identity,
        signature_type_identity(
            &scratch,
            &source_identities,
            duplicate,
            &[],
            &[],
            &[],
            &[],
            false,
        )
        .unwrap(),
    );
    assert!(
        signature_type_identity(&scratch, &[], duplicate, &[], &[], &[], &[], false,).is_err(),
        "scratch cannot invent missing source custody"
    );
    assert_ne!(
        original_identity,
        signature_type_identity(
            &scratch,
            &[(source_id, [8; 32])],
            duplicate,
            &[],
            &[],
            &[],
            &[],
            false,
        )
        .unwrap(),
        "a different exact source must remain a different identity",
    );
    let local = Identifier::generated("local");
    let public = Identifier::generated("public");
    let reference = scratch
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: duplicate,
            access: language_core::ReferenceAccess::Shared,
            lifetime: Some(local.clone()),
        });
    let direct = signature_type_identity(
        &scratch,
        &source_identities,
        reference,
        &[],
        std::slice::from_ref(&local),
        &[],
        &[],
        false,
    )
    .unwrap();
    let renamed = signature_type_identity(
        &scratch,
        &source_identities,
        reference,
        &[],
        std::slice::from_ref(&public),
        &[],
        &[(local, public.clone())],
        false,
    )
    .unwrap();
    assert_eq!(direct, renamed);
    assert!(
        signature_type_identity(
            &scratch,
            &source_identities,
            reference,
            &[],
            &[],
            &[],
            &[],
            false,
        )
        .is_err(),
        "scratch must not hide an unbound lifetime"
    );
    assert_eq!(original.type_reference_table, original_types);
}
