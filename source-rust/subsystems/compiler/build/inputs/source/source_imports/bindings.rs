//! Connect each import to the admitted source it selected. Build and Product
//! can load the same path with different permissions. Keep both the importing
//! file and its selected target when handing the names to the compiler.

use super::ReconciledPackageImportRequest;
use super::SourceStorage;
use crate::PackageCompilationInputs;
use diagnostics::Diagnostic;
use source_compiler::SourceImport;
use source_provenance::DependencyScope;
use source_text::SourceId;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub(super) struct ImportOccurrence {
    source: SourceId,
    span: source_text::Span,
    ordinal: usize,
    authored_path: String,
    alias: String,
    package_alias: Option<String>,
}

impl ImportOccurrence {
    pub(super) fn file(
        source: SourceId,
        import: &SourceImport,
        authored_path: String,
        alias: &str,
        package_alias: Option<&str>,
    ) -> Self {
        Self {
            source,
            span: import.span,
            ordinal: import.ordinal,
            authored_path,
            alias: alias.into(),
            package_alias: package_alias
                .filter(|root| *root != "omega")
                .map(str::to_owned),
        }
    }

    fn error(&self, message: String) -> Diagnostic {
        Diagnostic::error(message)
            .with_source_span(source_text::SourceSpan::new(self.source, self.span))
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
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        Ok(self.request.exact_file(packages)?.into_iter().collect())
    }

    pub(crate) fn resolve_for_exact_target(
        &self,
        packages: &PackageCompilationInputs,
        sources: &source_compiler::representations::source_inputs::SourceMap,
    ) -> Result<Vec<ResolvedSourceImport>, Vec<Diagnostic>> {
        self.resolve_with_prepared_build(packages, sources, None)
    }

    pub(super) fn resolve_with_prepared_build(
        &self,
        packages: &PackageCompilationInputs,
        sources: &source_compiler::representations::source_inputs::SourceMap,
        prepared: Option<&crate::PackageBuildHandoff>,
    ) -> Result<Vec<ResolvedSourceImport>, Vec<Diagnostic>> {
        let mut paths = self.physical_sources(packages)?;
        // The ordinary import uses the file already selected for this exact
        // package and scope. Authored/issued name collisions are refused when
        // mounting the view, before any of its files is parsed.
        if paths.is_empty() {
            let emitted = self
                .request
                .expected_root
                .join(source_admission::GENERATED_SOURCE_DIRECTORY)
                .join(&self.request.relative_path);
            if sources.selected_files().any(|file| {
                file.path == emitted
                    && file.package_identity == Some(self.request.package)
                    && file.dependency_scope == self.scope
                    && file.resolution_stratum != source_admission::SourceResolutionStratum::Base
            }) {
                paths.push(emitted);
            }
        }
        let purpose = match self.scope {
            DependencyScope::Product => crate::declarations::DependencyPurpose::Product,
            DependencyScope::Build => crate::declarations::DependencyPurpose::Build,
        };
        let admitted = packages
            .dependency_build_handoff_instances()
            .filter(|(scope, bundle)| *scope == purpose && bundle.package() == self.request.package)
            .map(|(_, bundle)| bundle);
        // A prepared Build can supply its own output separately from the
        // package graph. If both retain it, they name one issued handoff.
        for bundle in admitted
            .filter(|bundle| Some(*bundle) != prepared)
            .chain(prepared)
        {
            if bundle.package() != self.request.package {
                continue;
            }
            let unit = bundle
                .generated_source(std::slice::from_ref(&self.request.relative_path))
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if let Some(unit) = unit {
                let path =
                    super::generated::generated_file_path(&self.request.expected_root, unit)?;
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        if paths.len() != 1 {
            return Err(vec![self.occurrence.error(if paths.is_empty() {
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
        Ok(paths
            .into_iter()
            .map(|path| ResolvedSourceImport {
                occurrence: self.occurrence.clone(),
                path,
            })
            .collect())
    }
}

/// Report imports that the Build itself needs and its caller has not supplied.
/// A file used exclusively as a Product operand resolves in the Product scope.
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
            if import.requires_build_binding()
                && selected
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

pub(crate) fn retain_import_bindings(
    storage: &SourceStorage,
    query: Option<&super::ProductQueryInputs>,
) -> Result<
    Vec<source_compiler::representations::source_symbols::SourceScopedTopLevelBinding>,
    Vec<Diagnostic>,
> {
    let mut bindings = super::files::build_product_file_imports(storage, query)?;
    if storage.resolved_imports.is_empty() {
        return Ok(bindings);
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
    bindings.reserve(imports.len());
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
            if prior != occurrence || anchor.source_id != declaration.source_id {
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` acquired more than one checked import target",
                    occurrence.authored_path,
                ))]);
            }
            continue;
        }
        use source_compiler::representations::source_symbols::SourceScopedTopLevelBinding;
        bindings.push(
            SourceScopedTopLevelBinding::file_import(
                occurrence.source,
                declaration.source_id,
                occurrence.alias.as_str(),
            )
            .from_dependency(
                occurrence
                    .package_alias
                    .as_deref()
                    .zip(declaration.package_identity),
            ),
        );
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
                span: Default::default(),
                ordinal: 0,
                authored_path: "{dep}/a.omg".into(),
                alias: "chosen".into(),
                package_alias: Some("dep".into()),
            },
            path: path.into(),
        }
    }

    #[test]
    fn file_import_uses_selected_replacement_and_one_binding_per_occurrence() {
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
                &file("/app/main.omg", "/app", "use {dep}/a.omg as chosen;"),
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
            import(importer, "/dep/a.omg"),
        ];
        assert_eq!(
            retain_import_bindings(&storage, None).unwrap(),
            [
                source_compiler::representations::source_symbols::SourceScopedTopLevelBinding::file_import(
                    importer,
                    current,
                    "chosen",
                )
            ]
        );
    }

    #[test]
    fn a_build_file_alias_selects_separate_build_and_product_dependencies() {
        use semantic_vocabulary::PackageKeyIdentity;
        use source_compiler::representations::source_symbols::SourceScopedTopLevelBinding;
        use source_compiler::{SourceInput, SourceSyntax};

        let [app, build, product] =
            [41, 42, 43].map(|byte| PackageKeyIdentity::from_digest([byte; 32]).unwrap());
        let mut storage = SourceStorage::default();
        let mut add = |path: &str, root: &str, key, scope| {
            storage
                .sources
                .add_checked_instance(
                    path.into(),
                    "",
                    root.into(),
                    Some(key),
                    SourceOrigin::User,
                    SourceResolutionStratum::Base,
                    scope,
                )
                .source_id
        };
        let author = add("/app/build.omg", "/app", app, DependencyScope::Build);
        let build_target = add("/build/api.omg", "/build", build, DependencyScope::Build);
        let product_target = add(
            "/product/api.omg",
            "/product",
            product,
            DependencyScope::Product,
        );
        let parsed = SourceInput(
            [source_admission::LoadedSource {
                source_id: author,
                path: "/app/build.omg".into(),
                source: "use {kit}/api.omg as chosen; machine build() { |chosen|::run(); }".into(),
                scope: DependencyScope::Build,
                ..Default::default()
            }]
            .into_iter()
            .collect(),
        )
        .append(&mut SourceSyntax::default(), &mut Default::default())
        .unwrap();
        let import = parsed.sources.span_or_empty(parsed.batch)[0].imports[0].clone();
        for source_id in [author, build_target, product_target] {
            storage.source_roots.append(SourceRoots {
                source_id,
                root_items: Default::default(),
                build_imports: if source_id == author {
                    vec![import.clone()]
                } else {
                    Vec::new()
                },
            });
        }
        storage.resolved_imports.push(ResolvedSourceImport {
            occurrence: ImportOccurrence::file(
                author,
                &import,
                "{kit}/api.omg".into(),
                "chosen",
                Some("kit"),
            ),
            path: "/build/api.omg".into(),
        });
        let build_binding =
            SourceScopedTopLevelBinding::file_import(author, build_target, "chosen")
                .from_dependency(Some(("kit", build)));
        assert_eq!(
            retain_import_bindings(&storage, None).unwrap(),
            std::slice::from_ref(&build_binding)
        );
        storage
            .sources
            .retain_product_dependency_scope([(app, "kit", product)]);
        assert_eq!(
            retain_import_bindings(&storage, None).unwrap(),
            [
                SourceScopedTopLevelBinding::product_file_import(author, product_target, "chosen")
                    .from_dependency(Some(("kit", product))),
                build_binding,
            ]
        );
    }

    #[test]
    fn file_import_refuses_ambiguous_selected_instances() {
        let mut files = vec![
            file("/app/main.omg", "/app", "use {dep}/a.omg as chosen;"),
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
        assert!(retain_import_bindings(&storage, None).is_err());
    }

    #[test]
    fn file_import_refuses_mixed_package_custody() {
        let mut files = vec![
            file("/app/main.omg", "/app", "use {dep}/a.omg as chosen;"),
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
        assert!(retain_import_bindings(&storage, None).is_err());
    }
}
