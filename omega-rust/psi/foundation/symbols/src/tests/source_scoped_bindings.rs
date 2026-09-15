use crate::{SourceScopedTopLevelBinding, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use source::{SourceMap, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn source_scoped_top_level_binding_selects_exact_declaration_source() {
    let mut sources = SourceMap::default();
    let main_source = sources
        .add(PathBuf::from("main.omg"), String::from("Build"))
        .source_id;
    let build_source = sources
        .add(PathBuf::from("build.omg"), String::from("Build"))
        .source_id;
    let prelude_source = sources
        .add(PathBuf::from("<build-prelude>"), String::from("Build"))
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(Arc::new(sources)),
        vec![SourceScopedTopLevelBinding::new(
            build_source,
            prelude_source,
            "Build",
        )],
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(main_source, Span::new(0, 5))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(prelude_source, Span::new(0, 5))),
            ),
        ],
    ))
    .collect::<Vec<_>>();
    let symbols = builder.finish();
    let resolve = |source_id| {
        symbols
            .find_top_level_by_name_and_kinds_from_source(
                "Build",
                &[SymbolKind::Data],
                SourceSpan::new(source_id, Span::new(0, 5)),
            )
            .expect("Build declaration")
    };

    assert_eq!(resolve(main_source), declarations[0]);
    assert_eq!(resolve(build_source), declarations[1]);
    assert_eq!(resolve(prelude_source), declarations[1]);
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Build",
            &[SymbolKind::Data],
            SourceSpan::default(),
        ),
        Some(declarations[0]),
        "source-free generated names must not inherit an authored source binding",
    );
    assert!(symbols.source_scopes_separate(declarations[0], declarations[1]));
}

#[test]
fn source_scoped_top_level_binding_fails_closed_without_its_target() {
    let mut sources = SourceMap::default();
    let main_source = sources
        .add(PathBuf::from("main.omg"), String::from("Build"))
        .source_id;
    let build_source = sources
        .add(PathBuf::from("build.omg"), String::from("Build"))
        .source_id;
    let absent_prelude_source = sources
        .add(PathBuf::from("<build-prelude>"), String::from("Other"))
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(Arc::new(sources)),
        vec![SourceScopedTopLevelBinding::new(
            build_source,
            absent_prelude_source,
            "Build",
        )],
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    builder.insert_children(
        root,
        [(
            SymbolKind::Data,
            SymbolNameRef::Source(SourceSpan::new(main_source, Span::new(0, 5))),
        )],
    );
    let symbols = builder.finish();

    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Build",
            &[SymbolKind::Data],
            SourceSpan::new(build_source, Span::new(0, 5)),
        ),
        None,
    );
}

#[test]
fn source_scoped_top_level_binding_fails_closed_with_ambiguous_targets() {
    let mut sources = SourceMap::default();
    let build_source = sources
        .add(PathBuf::from("build.omg"), String::from("Build"))
        .source_id;
    let prelude_source = sources
        .add(
            PathBuf::from("<build-prelude>"),
            String::from("Build Build"),
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(Arc::new(sources)),
        vec![SourceScopedTopLevelBinding::new(
            build_source,
            prelude_source,
            "Build",
        )],
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    builder.insert_children(
        root,
        [
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(prelude_source, Span::new(0, 5))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(prelude_source, Span::new(6, 11))),
            ),
        ],
    );
    let symbols = builder.finish();

    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Build",
            &[SymbolKind::Data],
            SourceSpan::new(build_source, Span::new(0, 5)),
        ),
        None,
    );
}
