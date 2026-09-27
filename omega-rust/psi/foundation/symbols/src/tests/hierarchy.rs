use crate::{SourceScopedTopLevelBinding, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin, SourceResolutionStratum, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn authored_extension_appends_without_mutating_base_symbol_identity() {
    let mut sources = SourceMap::default();
    let base_source = sources
        .add(PathBuf::from("base.omg"), String::from("Base"))
        .source_id;
    let shadow_source = sources
        .add(PathBuf::from("shadow.omg"), String::from("Base"))
        .source_id;
    let reference_source = sources
        .add(PathBuf::from("reference.omg"), String::from("Base"))
        .source_id;
    let extension_source = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            String::from("Extension"),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(Arc::new(sources.clone())),
        vec![SourceScopedTopLevelBinding::new(
            reference_source,
            shadow_source,
            "Base",
        )],
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let base_declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(base_source, Span::new(0, 4))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(shadow_source, Span::new(0, 4))),
            ),
        ],
    ))
    .collect::<Vec<_>>();
    let [base, shadow] = base_declarations.as_slice() else {
        panic!("two base declarations")
    };
    let base_member = SymbolTableBuilder::child_handles(builder.insert_children(
        *base,
        [(SymbolKind::Field, SymbolNameRef::Static("member"))],
    ))
    .next()
    .expect("base member");
    let base_snapshot = (*base, *shadow, base_member);

    let mut extension = builder
        .finish()
        .begin_extension(Some(Arc::new(sources)), Vec::new());
    let generated = extension.insert_top_level([(
        SymbolKind::Data,
        SymbolNameRef::Source(SourceSpan::new(extension_source, Span::new(0, 9))),
    )]);
    let [generated] = generated.as_slice() else {
        panic!("one extension symbol")
    };
    let symbols = extension.finish();

    assert_eq!((*base, *shadow, base_member), base_snapshot);
    assert_eq!(symbols.find_child_by_name(root, "Base"), Some(*base));
    assert_eq!(
        symbols.find_child_by_name(root, "Extension"),
        Some(*generated)
    );
    assert_eq!(
        symbols.find_child_by_name(*base, "member"),
        Some(base_member)
    );
    assert_eq!(symbols.get(base_member).parent, *base);
    assert_eq!(symbols.get(*generated).parent, root);
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Base",
            &[SymbolKind::Data],
            SourceSpan::new(reference_source, Span::new(0, 4)),
        ),
        Some(*shadow),
        "extension must preserve base source-scoped bindings",
    );
}

#[test]
fn current_activation_extension_is_invisible_to_base_but_sees_the_complete_extension() {
    let package_identity =
        PackageKeyIdentity::from_digest([9; 32]).expect("nonzero package identity");
    let mut sources = SourceMap::default();
    let base_source = sources
        .add_with_metadata(
            PathBuf::from("package/main.omg"),
            String::from("Value"),
            PathBuf::from("package"),
            Some(package_identity),
            SourceOrigin::User,
        )
        .source_id;
    let first_extension = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("package/.omega/generated/first.omg"),
            String::from("Value ExtensionOnly"),
            PathBuf::from("package"),
            Some(package_identity),
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let second_extension = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("package/.omega/generated/second.omg"),
            String::from("use"),
            PathBuf::from("package"),
            Some(package_identity),
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(base_source, Span::new(0, 5))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(first_extension, Span::new(0, 5))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(first_extension, Span::new(6, 19))),
            ),
        ],
    ))
    .collect::<Vec<_>>();
    let symbols = builder.finish();
    let reference = |source_id| SourceSpan::new(source_id, Span::new(0, 1));

    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Value",
            &[SymbolKind::Data],
            reference(base_source),
        ),
        Some(declarations[0]),
    );
    assert!(!symbols.source_reference_can_see_symbol(reference(base_source), declarations[1],));
    assert!(symbols.source_reference_can_see_symbol(reference(second_extension), declarations[0],));
    assert!(symbols.source_reference_can_see_symbol(reference(second_extension), declarations[1],));
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Value",
            &[SymbolKind::Data],
            reference(second_extension),
        ),
        Some(declarations[1]),
        "an extension unit must prefer its shared extension stratum over Base",
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "ExtensionOnly",
            &[SymbolKind::Data],
            reference(second_extension),
        ),
        Some(declarations[2]),
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "ExtensionOnly",
            &[SymbolKind::Data],
            SourceSpan::default(),
        ),
        Some(declarations[2]),
        "source-free focused consumers retain their permissive lookup behavior",
    );
    assert!(symbols.source_reference_can_see_symbol(SourceSpan::default(), declarations[1],));
    assert!(symbols.source_scopes_separate(declarations[0], declarations[1]));
    assert_eq!(
        symbols.symbol_package_identity(declarations[1]),
        Some(package_identity),
        "resolution stratum must not alter package provenance",
    );
}
