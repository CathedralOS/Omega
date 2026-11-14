//! Load the real core and allocator units in an explicit retained test package.
//! The compiler catalog and loader read the same candidate library files.
//! No package-lock admission or private policy selection authority is supplied.
//! No closed application, checked tree or catalogue row is inserted here.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use symbols::SourceScopedTopLevelBinding;
use typed_trees::TypedTrees;

// The constructor control uses the same real library loader, but it does not
// demand a policy body merely to obtain typed source. Package and source
// custody stay intact; all library bytes match the compiler catalog.
pub(super) fn actual_library_source(
    library: &Path,
    query_text: &str,
    package: semantic_vocabulary::PackageKeyIdentity,
) -> Result<TypedTrees, String> {
    let resolved = resolved_library_source(library, query_text, package)?;
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|errors| format!("type actual alloc/core: {errors:?}"))
}

fn resolved_library_source(
    library: &Path,
    query_text: &str,
    package: semantic_vocabulary::PackageKeyIdentity,
) -> Result<symbol_resolved_trees::SymbolResolvedTrees, String> {
    let core_root = library
        .join("core")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let alloc_root = library
        .join("alloc")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut sources = source::SourceMap::default();
    let mut units = Vec::new();
    let mut paths = Vec::new();
    retained_units(&core_root, &mut paths)?;
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let file = sources.add_with_metadata(
            path,
            text,
            core_root.clone(),
            None,
            source::SourceOrigin::Toolchain,
        );
        units.push((file.source_id, Arc::clone(&file.source)));
    }
    let core_anchor = units
        .first()
        .ok_or("real core source selection is empty")?
        .0;
    for name in ["main.omg", "bump.omg", "vec.omg"] {
        let path = alloc_root.join(name);
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let file = sources.add_with_metadata(
            path,
            text,
            alloc_root.clone(),
            Some(package),
            source::SourceOrigin::User,
        );
        units.push((file.source_id, Arc::clone(&file.source)));
    }
    let file = sources.add_with_metadata(
        alloc_root.join("query.omg"),
        query_text.into(),
        alloc_root,
        Some(package),
        source::SourceOrigin::User,
    );
    let requester = file.source_id;
    units.push((requester, Arc::clone(&file.source)));
    let mut syntax = syntax_trees::SyntaxTrees::new(requester);
    let mut bindings = Vec::new();
    for (id, text) in units {
        let previous = syntax.root_item_handles().len();
        let tokens = source_files_to_tokens::Lexer::new(&text)
            .tokenize()
            .map_err(|errors| format!("lex source {id:?}: {errors:?}"))?;
        tokens_to_syntax_trees::parse(&mut syntax, id, &tokens)
            .map_err(|errors| format!("parse source {id:?}: {errors:?}"))?;
        for root in &syntax.root_item_handles()[previous..] {
            let syntax_trees::item::Item::Use(import) = syntax.root_item(*root) else {
                continue;
            };
            let members = syntax.items.identifier_path_members(import.path);
            if members.len() >= 3
                && members[0].as_str(&syntax) == "omega"
                && members[1].as_str(&syntax) == "language"
                && members[2].as_str(&syntax) == "core"
            {
                let path = members
                    .iter()
                    .map(|member| member.as_str(&syntax))
                    .collect::<Vec<_>>()
                    .join("::");
                // The same loader binding selects declarations across this
                // exact admitted core package; its anchor creates no module.
                bindings.push(SourceScopedTopLevelBinding::logical_module_import(
                    id,
                    core_anchor,
                    &path,
                    3,
                ));
            }
        }
    }
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    request.top_level_bindings = bindings;
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request)
        .map_err(|errors| format!("resolve actual alloc/core: {errors:?}"))?;
    Ok(resolved)
}

fn retained_units(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            retained_units(&path, paths)?;
        } else if path.extension().is_some_and(|extension| extension == "omg")
            && path.file_name().is_some_and(|name| name != "build.omg")
        {
            paths.push(path);
        }
    }
    Ok(())
}
