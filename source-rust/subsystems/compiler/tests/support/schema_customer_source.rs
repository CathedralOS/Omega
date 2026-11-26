//! Load the real core and allocator units into an explicit retained test package.
//! The compiler catalog and the loader read the same candidate library files.
//! This supplies no package-lock admission or private policy-selection authority,
//! and inserts no closed application, checked tree or catalogue row.

use crate::source_state::TypedTrees;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

// The constructor control uses the same real library loader, but it does not
// demand a policy body merely to obtain typed source. Package and source
// custody stay intact; all library bytes match the compiler catalog.
pub(crate) fn actual_library_source(
    library: &Path,
    query_text: &str,
    package: semantic_vocabulary::PackageKeyIdentity,
) -> Result<TypedTrees, String> {
    source_with_core(
        library,
        query_text,
        package,
        &["main.omg", "bump/bump.omg", "vec/vec.omg"],
    )
}

/// Give the subject exactly its authored query and any requested allocator
/// units. The real core stays a distinct toolchain dependency in both forms.
pub(crate) fn source_with_core(
    library: &Path,
    query_text: &str,
    package: semantic_vocabulary::PackageKeyIdentity,
    allocator_units: &[&str],
) -> Result<TypedTrees, String> {
    let resolved = resolved_library_source(library, query_text, package, allocator_units)?;
    crate::test_support::typing::lower_symbol_resolved_trees(&resolved)
        .map_err(|errors| format!("type actual library source: {errors:?}"))
}

fn resolved_library_source(
    library: &Path,
    query_text: &str,
    package: semantic_vocabulary::PackageKeyIdentity,
    allocator_units: &[&str],
) -> Result<symbol_resolved_trees::source_resolved::SymbolResolvedTrees, String> {
    let core_root = library
        .join("core")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let allocation_root = library
        .join("allocation")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut sources = crate::source_inputs::SourceMap::default();
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
            source_admission::SourceOrigin::Toolchain,
        );
        units.push((file.source_id, Arc::clone(&file.source)));
    }
    for name in allocator_units {
        let path = allocation_root.join(name);
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let file = sources.add_with_metadata(
            path,
            text,
            allocation_root.clone(),
            Some(package),
            source_admission::SourceOrigin::User,
        );
        units.push((file.source_id, Arc::clone(&file.source)));
    }
    let file = sources.add_with_metadata(
        allocation_root.join("query.omg"),
        query_text.into(),
        allocation_root,
        Some(package),
        source_admission::SourceOrigin::User,
    );
    let requester = file.source_id;
    units.push((requester, Arc::clone(&file.source)));
    let mut syntax = syntax_trees::SyntaxTrees::new(requester);
    for (id, text) in units {
        let tokens = crate::test_support::frontend_fixture::Lexer::new(&text)
            .tokenize()
            .map_err(|errors| format!("lex source {id:?}: {errors:?}"))?;
        crate::test_support::frontend_fixture::parse(&mut syntax, id, &tokens)
            .map_err(|errors| format!("parse source {id:?}: {errors:?}"))?;
    }
    let bindings = crate::test_support::resolution::fixture_local_and_core_imports(
        &syntax, &sources, &core_root,
    );
    let mut request = crate::test_support::resolution::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    request.top_level_bindings = bindings;
    let resolved = crate::test_support::resolution::resolve(request)
        .map_err(|errors| format!("resolve actual allocation/core: {errors:?}"))?;
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
