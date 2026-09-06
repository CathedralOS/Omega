use super::{
    Arc, PathBuf, SourceMap, SourceOrigin, SourceResolutionStratum, SourceScopedTopLevelBinding,
    SourceSpan, Span, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder,
};

#[test]
fn eligible_extension_method_remains_invisible_to_base_source() {
    let mut sources = SourceMap::default();
    let base_source = sources
        .add(PathBuf::from("base.omg"), String::from("Cell::read"))
        .source_id;
    let extension_source = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("extension.omg"),
            String::from("Cell::read"),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_reference = SourceSpan::new(base_source, Span::new(0, 10));
    let extension_reference = SourceSpan::new(extension_source, Span::new(0, 10));
    let (symbols, declarations) =
        method_symbols(sources, Vec::new(), [base_reference, extension_reference]);

    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source_matching(
            "Cell::read",
            &[SymbolKind::Machine],
            extension_reference,
            |candidate| candidate == declarations[1],
        ),
        Some(declarations[1]),
        "the eligible method is available within the extension",
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source_matching(
            "Cell::read",
            &[SymbolKind::Machine],
            base_reference,
            |candidate| candidate == declarations[1],
        ),
        None,
        "eligibility cannot expose an extension method to a base reference",
    );
}

#[test]
fn excluded_bound_method_does_not_fall_back_to_eligible_local_method() {
    let mut sources = SourceMap::default();
    let reference_source = sources
        .add(PathBuf::from("caller.omg"), String::from("Cell::read"))
        .source_id;
    let bound_source = sources
        .add(PathBuf::from("bound.omg"), String::from("Cell::read"))
        .source_id;
    let reference = SourceSpan::new(reference_source, Span::new(0, 10));
    let (symbols, declarations) = method_symbols(
        sources,
        vec![SourceScopedTopLevelBinding::new(
            reference_source,
            bound_source,
            "Cell::read",
        )],
        [reference, SourceSpan::new(bound_source, Span::new(0, 10))],
    );

    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source_matching(
            "Cell::read",
            &[SymbolKind::Machine],
            reference,
            |_| true,
        ),
        Some(declarations[1]),
        "the explicit binding selects the other source over the local method",
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source_matching(
            "Cell::read",
            &[SymbolKind::Machine],
            reference,
            |candidate| candidate == declarations[0],
        ),
        None,
        "excluding the bound method cannot authorize the eligible local alternative",
    );
}

#[test]
fn multiple_eligible_bound_methods_remain_ambiguous() {
    let mut sources = SourceMap::default();
    let reference_source = sources
        .add(PathBuf::from("caller.omg"), String::from("Cell::read"))
        .source_id;
    let bound_source = sources
        .add(
            PathBuf::from("bound.omg"),
            String::from("Cell::read Cell::read"),
        )
        .source_id;
    let reference = SourceSpan::new(reference_source, Span::new(0, 10));
    let (symbols, declarations) = method_symbols(
        sources,
        vec![SourceScopedTopLevelBinding::new(
            reference_source,
            bound_source,
            "Cell::read",
        )],
        [
            reference,
            SourceSpan::new(bound_source, Span::new(0, 10)),
            SourceSpan::new(bound_source, Span::new(11, 21)),
        ],
    );

    for selected in &declarations[1..] {
        assert_eq!(
            symbols.find_top_level_by_name_and_kinds_from_source_matching(
                "Cell::read",
                &[SymbolKind::Machine],
                reference,
                |candidate| candidate == *selected,
            ),
            Some(*selected),
            "each bound declaration can be selected when uniquely eligible",
        );
    }
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source_matching(
            "Cell::read",
            &[SymbolKind::Machine],
            reference,
            |candidate| candidate != declarations[0],
        ),
        None,
        "excluding the local alternative leaves two eligible bound declarations",
    );
}

fn method_symbols<const N: usize>(
    sources: SourceMap,
    bindings: Vec<SourceScopedTopLevelBinding>,
    declarations: [SourceSpan; N],
) -> (SymbolTable, Vec<SymbolHandle>) {
    let mut builder =
        SymbolTableBuilder::with_sources_and_top_level_bindings(Some(Arc::new(sources)), bindings);
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        declarations.map(|span| (SymbolKind::Machine, SymbolNameRef::Source(span))),
    ))
    .collect();

    (builder.finish(), declarations)
}
