//! Test-only fixture helpers for supplying package and core import edges.
use crate::source_inputs::SourceMap;
use syntax_trees::SyntaxTrees;

/// Supply the named file edges a package driver would admit for a stage fixture.
/// Both paths name files in the fixture's source map, independent of insertion order.
pub fn fixture_file_bindings(
    sources: &SourceMap,
    edges: &[(&str, &str, &str)],
) -> Vec<crate::source_symbols::SourceScopedTopLevelBinding> {
    let id = |path: &str| {
        sources
            .files()
            .find(|file| file.path == std::path::Path::new(path))
            .unwrap_or_else(|| panic!("missing fixture source {path}"))
            .source_id
    };
    edges
        .iter()
        .map(|&(requester, target, alias)| {
            crate::source_symbols::SourceScopedTopLevelBinding::file_import(
                id(requester),
                id(target),
                alias,
            )
        })
        .collect()
}

/// Bind local and bundled-core imports to the files loaded by a stage fixture.
/// External package edges are supplied explicitly by their owning tests.
pub fn fixture_local_and_core_imports(
    syntax: &SyntaxTrees,
    sources: &SourceMap,
    core_root: &std::path::Path,
) -> Vec<crate::source_symbols::SourceScopedTopLevelBinding> {
    syntax
        .root_items()
        .filter_map(|item| {
            let syntax_trees::item::Item::Use(syntax_trees::item::UseItem {
                root,
                components,
                alias,
                span,
                ..
            }) = item
            else {
                return None;
            };
            let requester = sources.get(span.source_id)?;
            let mut names = syntax
                .items
                .identifier_path_members(*components)
                .iter()
                .map(|name| name.as_str(syntax));
            let package = match root.as_ref().map(|name| name.as_str(syntax)) {
                None => requester.package_root.as_path(),
                Some("omega")
                    if names.next() == Some("language") && names.next() == Some("core") =>
                {
                    core_root
                }
                Some(_) => return None,
            };
            let mut path = package.join(names.collect::<std::path::PathBuf>());
            path.set_extension("omg");
            let target = sources.files().find(|file| {
                file.path == path
                    && file.package_root == package
                    && file.dependency_scope == requester.dependency_scope
            })?;
            Some(
                crate::source_symbols::SourceScopedTopLevelBinding::file_import(
                    requester.source_id,
                    target.source_id,
                    alias.as_str(syntax),
                ),
            )
        })
        .collect()
}
