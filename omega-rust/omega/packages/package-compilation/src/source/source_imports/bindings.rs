// Import occurrence custody (`ImportOccurrence`, `ResolvedSourceImport`, `PendingPackageImport`)
// and I/O-free joining to the loaded source frontier (`retain_module_import_bindings`).
//
// `retain_module_import_bindings` sorts loaded sources by `(path, scope)` once and
// binary-searches each import's target path within the importer's exact `DependencyScope`,
// preserving authored `(source_id, ordinal)` binding order independent of discovery waves
// and verifying that non-direct imports resolve to a file declaring `module`.

use super::ReconciledPackageImportRequest;
use super::SourceStorage;
use super::paths::{identifier_path_text, source_path_candidates};
use crate::PackageCompilationInputs;
use diagnostics::Diagnostic;
use source::{DependencyScope, SourceId};
use std::path::{Path, PathBuf};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::Item;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub(super) struct ImportOccurrence {
    source: SourceId,
    ordinal: usize,
    authored_path: String,
    package_prefix_members: usize,
}

impl ImportOccurrence {
    pub(super) fn new(
        syntax_trees: &syntax_trees::SyntaxTrees,
        source: SourceId,
        ordinal: usize,
        members: &[Identifier],
        prefix: usize,
    ) -> Self {
        Self {
            source,
            ordinal,
            authored_path: identifier_path_text(syntax_trees, members),
            package_prefix_members: prefix,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub(crate) struct ResolvedSourceImport {
    pub(super) occurrence: ImportOccurrence,
    pub path: PathBuf,
    pub(super) requires_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub(crate) struct PendingPackageImport {
    pub(super) occurrence: ImportOccurrence,
    pub(super) scope: DependencyScope, // Checked instance under which the import was discovered.
    pub(super) request: ReconciledPackageImportRequest,
}

impl PendingPackageImport {
    pub(crate) fn importer_scope(&self) -> DependencyScope {
        self.scope
    }

    pub fn physical_source(
        &self,
        packages: &PackageCompilationInputs,
    ) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
        self.request.physical_source(packages)
    }

    pub(crate) fn resolve_for_exact_target(
        &self,
        packages: &PackageCompilationInputs,
    ) -> Result<ResolvedSourceImport, Vec<Diagnostic>> {
        let path = self
            .request
            .resolve_for_exact_target(packages, self.scope)?;
        let direct = source_path_candidates(&self.request.relative_path);
        let generated = packages
            .generated_source_import_path(
                self.request.package,
                match self.scope {
                    DependencyScope::Product => build_declarations::DependencyPurpose::Product,
                    DependencyScope::Build => build_declarations::DependencyPurpose::Build,
                },
                &direct,
            )
            .map_err(|error| vec![Diagnostic::error(error)])?;
        let requires_module = generated.as_ref() != Some(&path)
            && !direct.iter().any(|candidate| {
                same_source_path(
                    &self.request.expected_root.join(candidate),
                    &path,
                    packages.source_filesystem(),
                )
            });
        Ok(ResolvedSourceImport {
            occurrence: self.occurrence.clone(),
            path,
            requires_module,
        })
    }
}

pub(super) fn direct_source_import(
    syntax_trees: &syntax_trees::SyntaxTrees,
    root: &Path,
    members: &[Identifier],
    resolved: &Path,
    filesystem: &source::SourceFileSystem,
) -> bool {
    let relative = members.iter().fold(PathBuf::new(), |mut path, member| {
        path.push(member.as_str(syntax_trees));
        path
    });
    source_path_candidates(&relative)
        .iter()
        .any(|candidate| same_source_path(&root.join(candidate), resolved, filesystem))
}

fn same_source_path(
    candidate: &Path,
    resolved: &Path,
    filesystem: &source::SourceFileSystem,
) -> bool {
    candidate == resolved
        || filesystem
            .canonicalize(candidate)
            .is_ok_and(|path| path == resolved)
}

pub(crate) fn retain_module_import_bindings(
    storage: &SourceStorage,
) -> Result<Vec<symbols::SourceScopedTopLevelBinding>, Vec<Diagnostic>> {
    if storage.resolved_imports.is_empty() {
        return Ok(Vec::new());
    }
    let mut sources = storage
        .source_roots
        .iter()
        .map(|(_, roots)| {
            let declares_module = roots
                .root_items
                .iter()
                .any(|item| matches!(storage.syntax_trees.root_item(*item), Item::Module(_)));
            let source = storage
                .sources
                .get(roots.source_id)
                .expect("parsed roots retain their source-map instance");
            (source, declares_module, source.dependency_scope)
        })
        .collect::<Vec<_>>();
    sources.sort_by(|(left, _, left_scope), (right, _, right_scope)| {
        left.path
            .cmp(&right.path)
            .then_with(|| scope_order(*left_scope).cmp(&scope_order(*right_scope)))
    });
    let mut imports = storage.resolved_imports.iter().collect::<Vec<_>>();
    imports.sort_by_key(|import| (import.occurrence.source.0, import.occurrence.ordinal));
    let mut bindings = Vec::with_capacity(imports.len());
    for import in imports {
        let occurrence = &import.occurrence;
        let importer_scope = storage
            .sources
            .get(occurrence.source)
            .map(|file| file.dependency_scope)
            .unwrap_or(DependencyScope::Product);
        let position = sources.partition_point(|(source, _, _)| source.path < import.path);
        let run_end = sources[position..]
            .iter()
            .position(|(source, _, _)| source.path != import.path)
            .map(|offset| position + offset)
            .unwrap_or(sources.len());
        let candidates = &sources[position..run_end];
        let (declaration, declares_module, _) = candidates
            .iter()
            .find(|(_, _, scope)| *scope == importer_scope)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "import `{}` no longer resolves to its parsed source frontier",
                    occurrence.authored_path,
                ))]
            })?;
        if import.requires_module && !declares_module {
            return Err(vec![Diagnostic::error(format!(
                "import `{}` requires a declared module in {}",
                occurrence.authored_path,
                import.path.display(),
            ))]);
        }
        bindings.push(symbols::SourceScopedTopLevelBinding::module_import(
            occurrence.source,
            declaration.source_id,
            &occurrence.authored_path,
            occurrence.package_prefix_members,
        ));
    }
    Ok(bindings)
}

fn scope_order(scope: DependencyScope) -> u8 {
    match scope {
        DependencyScope::Product => 0,
        DependencyScope::Build => 1,
    }
}
