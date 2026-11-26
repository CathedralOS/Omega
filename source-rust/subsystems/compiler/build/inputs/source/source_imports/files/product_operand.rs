//! Select the file named by an executed Build's Product operand in the author's
//! Product dependency graph. A static binding and a declaration query use the same import.

use super::super::SourceStorage;
use super::super::bindings::{ImportOccurrence, PendingPackageImport};
use super::super::package_selection::ReconciledPackageImportRequest;
use super::{captured_file, core_relative_path};
use crate::PackageCompilationInputs;
use diagnostics::Diagnostic;
use source_compiler::SourceImportTarget;
use source_text::SourceSpan;
use std::path::PathBuf;

pub(crate) fn product_file_sources<'graph>(
    storage: &mut SourceStorage,
    occurrence: SourceSpan,
    alias: &str,
    graphs: impl IntoIterator<Item = &'graph PackageCompilationInputs>,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let author = occurrence.source_id;
    let import = storage.source_roots.iter().find(|(_, roots)| roots.source_id == author)
        .and_then(|(_, roots)| roots.build_imports.iter().find(|import| matches!(&import.target, SourceImportTarget::File { alias: name, .. } if name == alias)));
    let Some(import) = import else {
        return Ok(Vec::new());
    };
    let SourceImportTarget::File { root, path, .. } = &import.target else {
        unreachable!()
    };
    let capture = |root: &std::path::Path, logical: &str| {
        captured_file(root, std::path::Path::new(logical), &storage.filesystem)?
            .map(|file| vec![file])
            .ok_or_else(|| {
                vec![
                    Diagnostic::error(format!("no admitted logical file `{logical}`"))
                        .with_source_span(SourceSpan::new(author, import.span)),
                ]
            })
    };
    if root.as_deref() == Some("omega") {
        return capture(
            &super::super::bundled_core_root()?,
            core_relative_path(path).map_err(|reason| vec![Diagnostic::error(reason)])?,
        );
    }
    let requester = storage
        .sources
        .get(author)
        .expect("the Build author was just rejoined");
    let package = match root.as_deref() {
        None => requester.package_identity,
        Some(alias) => storage.sources.product_dependency_at(occurrence, alias),
    };
    let selected = package.and_then(|owner| {
        graphs
            .into_iter()
            .find_map(|graph| graph.package_root(owner).map(|root| (owner, root, graph)))
    });
    if let Some((owner, source_root, graph)) = selected {
        let request = PendingPackageImport {
            occurrence: ImportOccurrence::file(
                author,
                import,
                import.target.to_string(),
                alias,
                root.as_deref(),
            ),
            scope: source_provenance::DependencyScope::Product,
            request: ReconciledPackageImportRequest::file(
                owner,
                source_root,
                path,
                &requester.path,
                root.is_some(),
            ),
        };
        return request
            .resolve_for_exact_target(graph, &mut storage.package_roots, &storage.sources)
            .map(|resolved| resolved.into_iter().map(|file| file.path).collect());
    }
    if root.is_none() {
        capture(&requester.package_root, path)
    } else {
        Ok(Vec::new())
    }
}
