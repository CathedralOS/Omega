//! Import occurrence custody and an I/O-free join to the loaded source frontier.

use super::{ReconciledPackageImportRequest, identifier_path_text, source_path_candidates};
use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::source::SourceStorage;
use diagnostics::Diagnostic;
use source::SourceId;
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
pub(in crate::pipeline) struct ResolvedSourceImport {
    pub(super) occurrence: ImportOccurrence,
    pub(in crate::pipeline) path: PathBuf,
    pub(super) requires_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::pipeline) struct PendingPackageImport {
    pub(super) occurrence: ImportOccurrence,
    pub(super) request: ReconciledPackageImportRequest,
}

impl PendingPackageImport {
    pub(in crate::pipeline) fn physical_source(&self) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
        self.request.physical_source()
    }

    pub(in crate::pipeline) fn resolve_for_exact_target(
        &self,
        packages: &PackageCompilationInputs,
    ) -> Result<ResolvedSourceImport, Vec<Diagnostic>> {
        let path = self.request.resolve_for_exact_target(packages)?;
        let direct = source_path_candidates(&self.request.relative_path);
        // Generated paths are virtual; direct-candidate membership comes from
        // the exact bundle, not filesystem existence.
        let generated = packages
            .generated_source_import_path(self.request.package, &direct)
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

pub(in crate::pipeline) fn retain_module_import_bindings(
    storage: &SourceStorage,
) -> Result<Vec<symbols::SourceScopedTopLevelBinding>, Vec<Diagnostic>> {
    if storage.resolved_imports.is_empty() {
        return Ok(Vec::new());
    }
    // One borrowed path index replaces a full frontier scan per import.
    // Binding order remains authored order, independent of discovery waves.
    let mut sources = storage
        .files
        .iter()
        .map(|(_, source)| {
            let declares_module = source
                .root_items
                .iter()
                .any(|item| matches!(storage.syntax_trees.root_item(*item), Item::Module(_)));
            (source, declares_module)
        })
        .collect::<Vec<_>>();
    sources.sort_by(|(left, _), (right, _)| left.path.cmp(&right.path));
    let mut imports = storage.resolved_imports.iter().collect::<Vec<_>>();
    imports.sort_by_key(|import| (import.occurrence.source.0, import.occurrence.ordinal));
    let mut bindings = Vec::with_capacity(imports.len());
    for import in imports {
        let occurrence = &import.occurrence;
        let position = sources.partition_point(|(source, _)| source.path < import.path);
        let (declaration, declares_module) = sources
            .get(position)
            .filter(|(source, _)| source.path == import.path)
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
