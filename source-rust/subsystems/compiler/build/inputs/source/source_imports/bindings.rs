// Import occurrence custody (`ImportOccurrence`, `ResolvedSourceImport`, `PendingPackageImport`)
// and I/O-free joining to the loaded source frontier (`retain_module_import_bindings`).
//
// Join only selected source instances in the importer's checked scope. Several
// acquired files can contribute to one logical module, but one exact package
// anchor suffices: semantic lookup validates the whole selected declaration roster.

use super::ReconciledPackageImportRequest;
use super::SourceStorage;
use super::paths::identifier_path_text;
use crate::PackageCompilationInputs;
use diagnostics::Diagnostic;
use source_provenance::DependencyScope;
use source_text::SourceId;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub(super) struct ImportOccurrence {
    source: SourceId,
    ordinal: usize,
    authored_path: String,
    package_prefix_members: usize,
    file_alias: Option<String>,
}

impl ImportOccurrence {
    pub(super) fn file(
        source: SourceId,
        ordinal: usize,
        authored_path: String,
        alias: &str,
    ) -> Self {
        Self {
            source,
            ordinal,
            authored_path,
            package_prefix_members: 0,
            file_alias: Some(alias.into()),
        }
    }

    pub(super) fn new(source: SourceId, ordinal: usize, members: &[String], prefix: usize) -> Self {
        Self {
            source,
            ordinal,
            authored_path: identifier_path_text(members),
            package_prefix_members: prefix,
            file_alias: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub(crate) struct ResolvedSourceImport {
    pub(super) occurrence: ImportOccurrence,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub(crate) struct PendingPackageImport {
    pub(super) occurrence: ImportOccurrence,
    pub(super) scope: DependencyScope, // Checked instance under which the import was discovered.
    pub(super) request: ReconciledPackageImportRequest,
}

impl PendingPackageImport {
    pub(crate) fn importer_scope(&self) -> DependencyScope {
        self.scope
    }

    pub fn physical_sources(
        &self,
        packages: &PackageCompilationInputs,
        discovery: &mut impl super::LogicalSourceDiscovery,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        if self.occurrence.file_alias.is_some() {
            return Ok(self.request.exact_file(packages)?.into_iter().collect());
        }
        self.request
            .logical_sources(packages, discovery, self.scope)
    }

    pub(crate) fn resolve_for_exact_target(
        &self,
        packages: &PackageCompilationInputs,
        discovery: &mut impl super::LogicalSourceDiscovery,
    ) -> Result<Vec<ResolvedSourceImport>, Vec<Diagnostic>> {
        self.resolve_with_prepared_build(packages, discovery, None)
    }

    pub(super) fn resolve_with_prepared_build(
        &self,
        packages: &PackageCompilationInputs,
        discovery: &mut impl super::LogicalSourceDiscovery,
        prepared: Option<&crate::PackageBuildHandoff>,
    ) -> Result<Vec<ResolvedSourceImport>, Vec<Diagnostic>> {
        let mut paths = self.physical_sources(packages, discovery)?;
        let exact_file = self.occurrence.file_alias.is_some();
        if exact_file || paths.is_empty() {
            let purpose = match self.scope {
                DependencyScope::Product => build_declarations::DependencyPurpose::Product,
                DependencyScope::Build => build_declarations::DependencyPurpose::Build,
            };
            let admitted = packages.dependency_build_handoff_instances()
                .filter(|(scope, bundle)| *scope == purpose && bundle.package() == self.request.package)
                .map(|(_, bundle)| bundle);
            // A prepared Build can supply its own output separately from the
            // package graph. If both retain it, they name one issued handoff.
            for bundle in admitted.filter(|bundle| Some(*bundle) != prepared).chain(prepared) {
                if bundle.package() != self.request.package { continue; }
                let unit = if exact_file {
                    bundle.generated_source(std::slice::from_ref(&self.request.relative_path))
                        .map_err(|reason| vec![Diagnostic::error(reason)])?
                } else {
                    bundle.sources().first()
                };
                if let Some(unit) = unit {
                    paths.push(super::generated::dependency_generated_source_logical_path(&self.request.expected_root, unit)?);
                }
            }
        }
        if exact_file && paths.len() != 1 {
            return Err(vec![Diagnostic::error(if paths.is_empty() {
                format!(
                    "no admitted logical file `{}`",
                    self.occurrence.authored_path
                )
            } else {
                format!(
                    "more than one source was admitted as logical file `{}`",
                    self.occurrence.authored_path
                )
            })]);
        }
        if paths.is_empty() {
            if self.request.defers_in(self.scope) {
                return Ok(Vec::new());
            }
            return Err(vec![Diagnostic::error(format!(
                "import `{}` selects no authored or generated logical source namespace",
                self.occurrence.authored_path,
            ))]);
        }
        Ok(paths
            .into_iter()
            .map(|path| ResolvedSourceImport {
                occurrence: self.occurrence.clone(),
                path,
            })
            .collect())
    }
}

/// An authored Build import without a selected declaration can become visible
/// at a later preparation. Keep its importer so another file cannot use its scope.
pub(crate) fn pending_build_imports(
    storage: &SourceStorage,
) -> Vec<source_compiler::PendingBuildImport> {
    let mut selected = storage
        .resolved_imports
        .iter()
        .map(|binding| (binding.occurrence.source.0, binding.occurrence.ordinal))
        .collect::<Vec<_>>();
    selected.sort_unstable();
    let mut pending = Vec::new();
    for (_, roots) in storage.source_roots.iter() {
        if !storage.sources.source_is_selected(roots.source_id) {
            continue;
        }
        for import in &roots.build_imports {
            if selected
                .binary_search(&(roots.source_id.0, import.ordinal))
                .is_err()
            {
                pending.push(source_compiler::PendingBuildImport {
                    source: roots.source_id,
                    span: import.span,
                    target: import.target.clone(),
                });
            }
        }
    }
    pending.sort_unstable_by(|a, b| (a.source.0, &a.target).cmp(&(b.source.0, &b.target)));
    pending.dedup_by(|a, b| a.source == b.source && a.target == b.target);
    pending
}

pub(crate) fn retain_module_import_bindings(
    storage: &SourceStorage,
) -> Result<
    Vec<source_compiler::representations::source_symbols::SourceScopedTopLevelBinding>,
    Vec<Diagnostic>,
> {
    if storage.resolved_imports.is_empty() {
        return Ok(Vec::new());
    }
    let mut sources = storage
        .source_roots
        .iter()
        .filter(|(_, roots)| storage.sources.source_is_selected(roots.source_id))
        .map(|(_, roots)| {
            storage
                .sources
                .get(roots.source_id)
                .expect("parsed roots retain their source-map instance")
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| {
                scope_order(left.dependency_scope).cmp(&scope_order(right.dependency_scope))
            })
            .then_with(|| left.source_id.0.cmp(&right.source_id.0))
    });
    sources.dedup_by_key(|file| file.source_id);
    let mut imports = storage
        .resolved_imports
        .iter()
        .filter(|import| storage.sources.source_is_selected(import.occurrence.source))
        .collect::<Vec<_>>();
    imports.sort_by_key(|import| (import.occurrence.source.0, import.occurrence.ordinal));
    let mut bindings = Vec::with_capacity(imports.len());
    let mut previous: Option<(&ImportOccurrence, &source_admission::SourceFile)> = None;
    for import in imports {
        let occurrence = &import.occurrence;
        let importer = storage
            .sources
            .get(occurrence.source)
            .expect("selected importer retains its source-map instance");
        let position = sources.partition_point(|source| source.path < import.path);
        let run_end = sources[position..]
            .iter()
            .position(|source| source.path != import.path)
            .map(|offset| position + offset)
            .unwrap_or(sources.len());
        let mut candidates = sources[position..run_end]
            .iter()
            .copied()
            .filter(|source| source.dependency_scope == importer.dependency_scope);
        let declaration = candidates.next().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "import `{}` no longer resolves to its selected source frontier",
                occurrence.authored_path,
            ))]
        })?;
        if candidates.next().is_some() {
            return Err(vec![Diagnostic::error(format!(
                "import `{}` resolves to multiple selected source instances",
                occurrence.authored_path,
            ))]);
        }
        if let Some((prior, anchor)) = previous
            && prior.source == occurrence.source
            && prior.ordinal == occurrence.ordinal
        {
            if prior != occurrence
                || !storage.sources.same_checked_instance(
                    anchor.source_span(source_text::Span::default()),
                    declaration.source_span(source_text::Span::default()),
                )
            {
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` acquired more than one checked package namespace",
                    occurrence.authored_path,
                ))]);
            }
            continue;
        }
        use source_compiler::representations::source_symbols::SourceScopedTopLevelBinding;
        bindings.push(match &occurrence.file_alias {
            Some(alias) => SourceScopedTopLevelBinding::file_import(occurrence.source, declaration.source_id, alias.as_str()),
            None => SourceScopedTopLevelBinding::logical_module_import(occurrence.source, declaration.source_id, &occurrence.authored_path, occurrence.package_prefix_members),
        });
        previous = Some((occurrence, declaration));
    }
    Ok(bindings)
}

fn scope_order(scope: DependencyScope) -> u8 {
    match scope {
        DependencyScope::Product => 0,
        DependencyScope::Build => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::super::frontier::SourceRoots;
    use super::*;
    use source_admission::SourceFile;
    use source_admission::SourceOrigin;
    use source_admission::SourceResolutionStratum;
    use source_compiler::representations::source_inputs::{SourceCatalogue, SourceMap};

    fn file(path: &str, root: &str, text: &str) -> SourceFile {
        SourceFile {
            source_id: SourceId(0),
            path: PathBuf::from(path),
            package_root: PathBuf::from(root),
            package_identity: None,
            dependency_scope: DependencyScope::Product,
            origin: SourceOrigin::User,
            resolution_stratum: SourceResolutionStratum::Base,
            source: text.into(),
        }
    }

    fn import(source: SourceId, path: &str) -> ResolvedSourceImport {
        ResolvedSourceImport {
            occurrence: ImportOccurrence {
                source,
                ordinal: 0,
                authored_path: "dep::Thing".into(),
                package_prefix_members: 1,
                file_alias: None,
            },
            path: path.into(),
        }
    }

    #[test]
    fn logical_anchor_uses_selected_replacement_and_one_binding_per_occurrence() {
        let mut catalogue = SourceCatalogue::default();
        let mut old = catalogue.begin_selection();
        let stale = catalogue
            .admit_source_occurrence(&mut old, &file("/dep/a.omg", "/dep", "old"))
            .unwrap()
            .source_id;
        let mut selected = catalogue.begin_selection();
        let importer = catalogue
            .admit_source_occurrence(
                &mut selected,
                &file("/app/main.omg", "/app", "use dep\\Thing;"),
            )
            .unwrap()
            .source_id;
        let current = catalogue
            .admit_source_occurrence(&mut selected, &file("/dep/a.omg", "/dep", "new"))
            .unwrap()
            .source_id;
        let sibling = catalogue
            .admit_source_occurrence(&mut selected, &file("/dep/b.omg", "/dep", "sibling"))
            .unwrap()
            .source_id;
        let mut storage = SourceStorage::default();
        storage.sources = selected;
        for source_id in [stale, importer, current, sibling] {
            storage.source_roots.append(SourceRoots {
                source_id,
                root_items: Default::default(),
                build_imports: Vec::new(),
            });
        }
        storage.resolved_imports = vec![
            import(importer, "/dep/a.omg"),
            import(importer, "/dep/b.omg"),
        ];
        assert_eq!(
            retain_module_import_bindings(&storage).unwrap(),
            [
                source_compiler::representations::source_symbols::SourceScopedTopLevelBinding::logical_module_import(
                    importer,
                    current,
                    "dep::Thing",
                    1
                )
            ]
        );
    }

    #[test]
    fn logical_anchor_refuses_ambiguous_selected_instances() {
        let mut files = vec![
            file("/app/main.omg", "/app", "use dep\\Thing;"),
            file("/dep/a.omg", "/dep", "first"),
            file("/dep/a.omg", "/dep", "second"),
        ];
        for (index, source) in files.iter_mut().enumerate() {
            source.source_id = SourceId(index);
        }
        let mut storage = SourceStorage::default();
        storage.sources = SourceMap::from_files(files);
        for index in 0..3 {
            storage.source_roots.append(SourceRoots {
                source_id: SourceId(index),
                root_items: Default::default(),
                build_imports: Vec::new(),
            });
        }
        storage.resolved_imports = vec![import(SourceId(0), "/dep/a.omg")];
        assert!(retain_module_import_bindings(&storage).is_err());
    }

    #[test]
    fn logical_anchor_refuses_mixed_package_custody() {
        let mut files = vec![
            file("/app/main.omg", "/app", "use dep\\Thing;"),
            file("/dep/a.omg", "/dep", "first"),
            file("/other/b.omg", "/other", "second"),
        ];
        for (index, source) in files.iter_mut().enumerate() {
            source.source_id = SourceId(index);
        }
        let mut storage = SourceStorage::default();
        storage.sources = SourceMap::from_files(files);
        for index in 0..3 {
            storage.source_roots.append(SourceRoots {
                source_id: SourceId(index),
                root_items: Default::default(),
                build_imports: Vec::new(),
            });
        }
        storage.resolved_imports = vec![
            import(SourceId(0), "/dep/a.omg"),
            import(SourceId(0), "/other/b.omg"),
        ];
        assert!(retain_module_import_bindings(&storage).is_err());
    }
}
