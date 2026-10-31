use super::{vocabulary, vocabulary_for_owner};
use source::{DependencyScope, SourceMap, SourceOrigin, SourceResolutionStratum, SourceSpan, Span};
use std::{path::PathBuf, sync::Arc};
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{TypedTrees, data::DataDefinition};

/// These declarations deliberately have no width telescope: selecting one
/// exact pair must reach the independent shape check, never accept its shape.
fn fixture(scopes: &[DependencyScope]) -> (TypedTrees, [SymbolHandle; 2]) {
    let mut sources = SourceMap::default();
    let owner_sources = [DependencyScope::Build, DependencyScope::Product].map(|scope| {
        sources
            .add_checked_instance(
                PathBuf::from(format!("/test/{scope:?}.omg")),
                "owner",
                PathBuf::from("/test"),
                None,
                SourceOrigin::User,
                SourceResolutionStratum::Base,
                scope,
            )
            .source_id
    });
    let core_sources = scopes
        .iter()
        .map(|scope| {
            sources
                .add_checked_instance(
                    PathBuf::from("/toolchain/core/external_binding.omg"),
                    "ForeignBinding DllImport",
                    PathBuf::from("/toolchain/core"),
                    None,
                    SourceOrigin::Toolchain,
                    SourceResolutionStratum::Base,
                    *scope,
                )
                .source_id
        })
        .collect::<Vec<_>>();
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let owners = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        owner_sources.map(|source| {
            (
                SymbolKind::Machine,
                SymbolNameRef::Source(SourceSpan::new(source, Span::new(0, 5))),
            )
        }),
    ))
    .collect::<Vec<_>>();
    let mut definitions = Vec::new();
    for source in core_sources {
        let symbols = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [("ForeignBinding", 0, 14), ("DllImport", 15, 24)].map(|(_, start, end)| {
                (
                    SymbolKind::Data,
                    SymbolNameRef::Source(SourceSpan::new(source, Span::new(start, end))),
                )
            }),
        ))
        .collect::<Vec<_>>();
        for (name, symbol) in ["ForeignBinding", "DllImport"].into_iter().zip(symbols) {
            definitions.push(DataDefinition {
                symbol,
                name: name.into(),
                is_public: true,
                ..Default::default()
            });
        }
    }
    let mut typed = TypedTrees::default();
    typed.symbols = builder.finish();
    for definition in definitions {
        typed.push_data_definition(definition);
    }
    (typed, owners.try_into().expect("two source owners"))
}

#[test]
fn each_owner_selects_its_checked_vocabulary_before_shape_validation() {
    let (typed, owners) = fixture(&[DependencyScope::Build, DependencyScope::Product]);
    let global = vocabulary(&typed, |_| true).unwrap_err();
    assert!(global[0].message.contains("ForeignBinding=2, DllImport=2"));
    for owner in owners {
        let selected = vocabulary_for_owner(&typed, owner).unwrap_err();
        assert!(
            selected[0]
                .message
                .contains("exactly three width parameters")
        );
    }
}

#[test]
fn missing_owner_vocabulary_cannot_fall_back_to_another_purpose() {
    let (typed, [build, product]) = fixture(&[DependencyScope::Product]);
    let missing = vocabulary_for_owner(&typed, build).unwrap_err();
    assert!(missing[0].message.contains("ForeignBinding=0, DllImport=0"));
    let product = vocabulary_for_owner(&typed, product).unwrap_err();
    assert!(
        product[0]
            .message
            .contains("exactly three width parameters")
    );
}

#[test]
fn competing_declarations_in_one_purpose_still_reject_with_exact_counts() {
    let (typed, [build, product]) = fixture(&[
        DependencyScope::Build,
        DependencyScope::Build,
        DependencyScope::Product,
    ]);
    let ambiguous = vocabulary_for_owner(&typed, build).unwrap_err();
    assert!(
        ambiguous[0]
            .message
            .contains("ForeignBinding=2, DllImport=2")
    );
    assert!(ambiguous[0].message.contains("scope=Build"));
    assert!(!ambiguous[0].message.contains("scope=Product"));
    let product = vocabulary_for_owner(&typed, product).unwrap_err();
    assert!(
        product[0]
            .message
            .contains("exactly three width parameters")
    );
}
