//! Import occurrence custody and an I/O-free join to the loaded source frontier.

use super::{ReconciledPackageImportRequest, identifier_path_text, source_path_candidates};
use crate::source::SourceStorage;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use source::{DependencyScope, SourceId};
use std::path::{Path, PathBuf};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::Item;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ImportOccurrence {
    source: SourceId,
    ordinal: usize,
    authored_path: String,
    package_prefix_members: usize,
}

impl ImportOccurrence {
    pub(super) fn new(
        source: SourceId,
        ordinal: usize,
        members: &[Identifier],
        prefix: usize,
    ) -> Self {
        Self {
            source,
            ordinal,
            authored_path: identifier_path_text(members),
            package_prefix_members: prefix,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedSourceImport {
    pub(super) occurrence: ImportOccurrence,
    pub(crate) path: PathBuf,
    pub(super) requires_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingPackageImport {
    pub(super) occurrence: ImportOccurrence,
    /// The importer's checked instance. Retained so exact-target resolution
    /// joins the resolved target under the same instance even when the
    /// requesting path has a second instance.
    pub(super) scope: DependencyScope,
    pub(super) request: ReconciledPackageImportRequest,
}

impl PendingPackageImport {
    /// The checked instance this request was discovered under. The exact
    /// resolution joins its target under the importer's instance.
    pub(crate) fn importer_scope(&self) -> DependencyScope {
        self.scope
    }

    pub(crate) fn physical_source(&self) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
        self.request.physical_source()
    }

    pub(crate) fn resolve_for_exact_target(
        &self,
        packages: &PackageCompilationInputs,
    ) -> Result<ResolvedSourceImport, Vec<Diagnostic>> {
        let path = self
            .request
            .resolve_for_exact_target(packages, self.scope)?;
        let direct = source_path_candidates(&self.request.relative_path);
        // Generated paths are virtual; direct-candidate membership comes from
        // the exact bundle, not filesystem existence.
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
                same_source_path(&self.request.expected_root.join(candidate), &path)
            });
        Ok(ResolvedSourceImport {
            occurrence: self.occurrence.clone(),
            path,
            requires_module,
        })
    }
}

pub(super) fn direct_source_import(root: &Path, members: &[Identifier], resolved: &Path) -> bool {
    let relative = members.iter().fold(PathBuf::new(), |mut path, member| {
        path.push(member.as_str());
        path
    });
    source_path_candidates(&relative)
        .iter()
        .any(|candidate| same_source_path(&root.join(candidate), resolved))
}

fn same_source_path(candidate: &Path, resolved: &Path) -> bool {
    candidate == resolved || candidate.canonicalize().is_ok_and(|path| path == resolved)
}

pub(crate) fn retain_module_import_bindings(
    storage: &SourceStorage,
) -> Result<Vec<symbols::SourceScopedTopLevelBinding>, Vec<Diagnostic>> {
    if storage.resolved_imports.is_empty() {
        return Ok(Vec::new());
    }
    // One borrowed path index replaces a full frontier scan per import.
    // Binding order remains authored order, independent of discovery waves.
    // The sort keys scope so each path's at-most-two instances sit in a
    // deterministic order inside one contiguous run.
    let mut sources = storage
        .files
        .iter()
        .map(|(_, source)| {
            let declares_module = source
                .root_items
                .iter()
                .any(|item| matches!(storage.syntax_trees.root_item(*item), Item::Module(_)));
            let scope = storage
                .sources
                .get(source.source_id)
                .map(|file| file.dependency_scope)
                .unwrap_or(DependencyScope::Product);
            (source, declares_module, scope)
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
        // Physical and generated imports bind the exact checked scope. A
        // missing instance cannot borrow the other scope's declaration.
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
