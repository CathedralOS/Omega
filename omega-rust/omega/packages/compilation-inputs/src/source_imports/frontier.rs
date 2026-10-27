//! Source instances acquire exact package and toolchain ownership before parsing.

use arena::Arena;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use source::SourceMap;
use source::SourceOrigin;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use syntax_trees::ParsedSources;
use syntax_trees::SyntaxTrees;

#[derive(Clone, Default)]
pub(crate) struct SourceStorage {
    pub filesystem: source::SourceFileSystem,
    pub source_roots: Arena<SourceRoots>,
    pub sources: SourceMap,
    pub syntax_trees: SyntaxTrees,
    pub resolved_imports: Vec<super::ResolvedSourceImport>,
    default_package_root: PathBuf,
    default_package_identity: Option<PackageKeyIdentity>,
    anonymous_capture: Option<std::sync::Arc<source::CapturedSourceTree>>,
    anonymous_core_capture: Option<(PathBuf, std::sync::Arc<source::CapturedSourceTree>)>,
    package_roots: Vec<RegisteredPackageRoot>,
    toolchain_root: Option<PathBuf>,
    // Import reconciliation borrows custody independently of its mutable results.
    pub toolchain_contract_custody: ToolchainContractCustody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegisteredPackageRoot {
    root: PathBuf,
    identity: Option<PackageKeyIdentity>,
}

impl SourceStorage {
    /// Derive default-Build ownership from captured custody, then admit a new
    /// named cohort before parser coordinates or bindings leave this owner.
    pub(crate) fn seal_default_owner(
        &mut self,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<Option<Vec<source::LoadedSource>>, Vec<Diagnostic>> {
        let Some(capture) = self.retain_anonymous_owner(&[])? else {
            return Ok(None);
        };
        let sealed = capture.complete_without_dependencies()?;
        let identity = sealed
            .selected_files()
            .find(|file| {
                file.origin == SourceOrigin::User && file.package_root == self.default_package_root
            })
            .and_then(|file| file.package_identity)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "default Build lost its derived source owner",
                )]
            })?;
        self.sources = std::sync::Arc::unwrap_or_clone(sealed);
        self.default_package_identity = Some(identity);
        for root in &mut self.package_roots {
            if root.root == self.default_package_root && root.identity.is_none() {
                root.identity = Some(identity);
            }
        }
        self.reselect_current_sources(catalogue).map(Some)
    }

    pub(crate) fn root_sources(&self) -> Vec<source::SourceId> {
        self.sources
            .selected_files()
            .filter(|file| {
                file.package_root == self.default_package_root
                    && file.package_identity == self.default_package_identity
            })
            .map(|file| file.source_id)
            .collect()
    }

    /// Initialize exact package roots and product dependency authority before loading.
    /// Build-only dependencies never authorize product source selection.
    pub fn prepare(
        root_path: &Path,
        package_inputs: Option<&crate::PackageCompilationInputs>,
        source_capture: Option<&crate::CapturedLocalSource>,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<Self, Vec<Diagnostic>> {
        if package_inputs.is_some() && source_capture.is_some() {
            return Err(vec![Diagnostic::error(
                "source preparation cannot combine an unbound capture with a supplied package graph",
            )]);
        }
        match package_inputs {
            Some(package_inputs) => {
                package_inputs.validate_for_compilation(root_path, &super::bundled_core_root()?)?;
                let root_package = package_inputs
                    .package_root(package_inputs.root())
                    .expect("validated package inputs retain their root")
                    .to_path_buf();
                let mut storage = SourceStorage::for_package_compilation(
                    root_package,
                    package_inputs.root(),
                    super::bundled_core_root()?,
                    package_inputs.source_filesystem().clone(),
                );
                storage.sources = catalogue.begin_selection();
                for (identity, source_root) in package_inputs.packages() {
                    storage.register_reconciled_package_root(source_root.to_path_buf(), identity);
                }
                // The authorized product dependency roster rides with the
                // retained source map so later evaluator queries resolve
                // `alias::path` under each query occurrence's own product scope.
                // `dependencies()` yields product-purpose edges only; build edges
                // never authorize product selection.
                storage
                    .sources
                    .retain_product_dependency_scope(package_inputs.dependencies());
                Ok(storage)
            }
            None => {
                let root_package = if root_path.is_dir() {
                    root_path.to_path_buf()
                } else {
                    root_path
                        .parent()
                        .filter(|path| !path.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."))
                        .to_path_buf()
                };
                // The content owner and every compiler source read must see
                // the same complete snapshot, including negative observations.
                let captured = match source_capture {
                    Some(captured) => captured.clone(),
                    None => package_source::local::operations::capture_local_source_input(
                        &root_package,
                        package_source::LocalSourceLimits::default(),
                    )
                    .map_err(|error| {
                        vec![Diagnostic::error(format!(
                            "cannot capture anonymous source: {error}"
                        ))]
                    })?,
                };
                let root = captured.canonical_live_root().to_path_buf();
                if source_capture.is_some() && !root_path.starts_with(&root) {
                    return Err(vec![Diagnostic::error(
                        "compilation root is outside its acquired source capture",
                    )]);
                }
                let mut storage =
                    SourceStorage::for_compilation(root.clone(), super::bundled_omega_root()?);
                storage
                    .filesystem
                    .add_capture(root, captured.tree().clone())
                    .map_err(|error| {
                        vec![Diagnostic::error(format!(
                            "cannot mount anonymous source capture: {error}"
                        ))]
                    })?;
                storage.sources = catalogue.begin_selection();
                storage.anonymous_capture = Some(captured.tree().clone());
                // Core is a closed toolchain dependency, not the subset of
                // declarations demanded by one exported machine.
                let core = package_source::local::operations::capture_local_source_input(
                    super::bundled_core_root()?,
                    package_source::LocalSourceLimits::default(),
                )
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "cannot capture anonymous core dependency: {error}"
                    ))]
                })?;
                let core_root = core.canonical_live_root().to_path_buf();
                let core_tree = if core_root == storage.default_package_root {
                    captured.tree().clone()
                } else {
                    storage
                        .filesystem
                        .add_capture(core_root.clone(), core.tree().clone())
                        .map_err(|error| {
                            vec![Diagnostic::error(format!(
                                "cannot mount captured core dependency: {error}"
                            ))]
                        })?;
                    core.tree().clone()
                };
                storage.anonymous_core_capture = Some((core_root, core_tree));
                Ok(storage)
            }
        }
    }

    /// Establish ownership on a separate parsed Product prefix, never on the
    /// checked Build program. The original complete capture must survive the join.
    pub(crate) fn join_package_inputs(
        &mut self,
        inputs: &crate::PackageCompilationInputs,
    ) -> Result<(), Vec<Diagnostic>> {
        let refuse = |reason| vec![Diagnostic::error(reason)];
        let capture = self
            .anonymous_capture
            .as_ref()
            .ok_or_else(|| refuse("source join requires an unbound Build capture".to_owned()))?;
        inputs
            .validate_unbound_source_join(&self.default_package_root, capture)
            .map_err(refuse)?;
        let mut filesystem = inputs.source_filesystem().clone();
        if let Some((root, tree)) = &self.anonymous_core_capture
            && *root != self.default_package_root
        {
            filesystem
                .add_capture(root.clone(), tree.clone())
                .map_err(|error| {
                    refuse(format!(
                        "cannot retain Build core capture at Product join: {error}"
                    ))
                })?;
        }
        for file in self
            .sources
            .selected_files()
            .filter(|file| file.origin == SourceOrigin::User && file.package_identity.is_none())
        {
            let path = filesystem.canonicalize(&file.path).map_err(|error| {
                refuse(format!(
                    "cannot resolve anonymous source {} at Product join: {error}",
                    file.path.display()
                ))
            })?;
            if inputs.package_for_source(&path) != Some(inputs.root()) {
                return Err(refuse(format!(
                    "anonymous source {} belongs to a different reconciled package at Product join",
                    path.display(),
                )));
            }
        }
        self.sources
            .bind_anonymous_owner(&self.default_package_root, inputs.root())
            .map_err(|reason| refuse(reason.to_owned()))?;
        self.filesystem = filesystem;
        self.default_package_identity = Some(inputs.root());
        for (identity, root) in inputs.packages() {
            self.register_reconciled_package_root(root.to_path_buf(), identity);
        }
        self.sources
            .retain_product_dependency_scope(inputs.dependencies());
        self.anonymous_capture = None;
        self.anonymous_core_capture = None;
        Ok(())
    }

    /// Publish the newly sealed Product selection without changing coordinates
    /// retained by its anonymous Build. Parser pages are recreated from these
    /// captured bytes; the checked Build keeps its own immutable source map.
    pub(crate) fn reselect_current_sources(
        &mut self,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<Vec<source::LoadedSource>, Vec<Diagnostic>> {
        let current = self.sources.selected_files().cloned().collect::<Vec<_>>();
        self.sources = catalogue.begin_selection();
        let admitted = current
            .into_iter()
            .map(|file| {
                let original = file.source_id;
                let admitted = self.admit_source(
                    source::SourceContents {
                        path: file.path,
                        source: file.source,
                        scope: file.dependency_scope,
                    },
                    Some(file.origin),
                    file.resolution_stratum,
                    catalogue,
                )?;
                self.sources
                    .retain_source_rejoin(original, admitted.source_id)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                Ok::<_, Vec<Diagnostic>>(admitted)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.syntax_trees = admitted
            .first()
            .map(|source| SyntaxTrees::new(source.source_id))
            .unwrap_or_default();
        self.source_roots = Arena::default();
        self.resolved_imports.clear();
        Ok(admitted)
    }

    /// Retain the captured inputs without assigning a declaration owner while
    /// an authored Build can still request dependencies.
    pub(crate) fn retain_anonymous_owner(
        &mut self,
        generated: &[(source::SourceId, build_output::PackageGeneratedSource)],
    ) -> Result<Option<crate::AnonymousSourceCapture>, Vec<Diagnostic>> {
        if self.default_package_identity.is_some() {
            return Ok(None);
        }
        let Some(capture) = &self.anonymous_capture else {
            return Ok(None);
        };
        // SourceFile preserves the caller's diagnostic spelling, including
        // relative paths. Resolve containment through this invocation's captured
        // namespace, never by hashing or trusting that display spelling.
        for file in self
            .sources
            .selected_files()
            .filter(|file| file.origin == SourceOrigin::User && file.package_identity.is_none())
        {
            if generated
                .iter()
                .any(|(source, _)| *source == file.source_id)
            {
                continue; // exact compiler-generated unit custody keeps its generating owner
            }
            let path = self.filesystem.canonicalize(&file.path).map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot resolve anonymous source {}: {error}",
                    file.path.display()
                ))]
            })?;
            if !path.starts_with(&self.default_package_root) {
                return Err(vec![Diagnostic::error(format!(
                    "source {} has no supplied owner outside the anonymous capture",
                    file.path.display()
                ))]);
            }
        }
        let (core_root, core_snapshot) = self.anonymous_core_capture.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "anonymous source capture lost its core dependency",
            )]
        })?;
        let core_identity = crate::anonymous_owner::core_snapshot_identity(core_snapshot)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        // Virtual and closed toolchain-contract inputs outside core remain
        // exact; checked-context duplication and core demand order do not
        // rename this complete captured source owner.
        let contracts = crate::toolchain_source_identities(
            self.sources
                .selected_files()
                .filter(|file| file.package_root != *core_root),
        )?;
        Ok(Some(crate::AnonymousSourceCapture {
            root: self.default_package_root.clone(),
            capture: capture.clone(),
            filesystem: self.filesystem.clone(),
            semantic_dependencies: std::iter::once(core_identity)
                .chain(contracts.into_iter().map(|(_, identity)| identity))
                .collect(),
            sources: std::sync::Arc::new(std::mem::take(&mut self.sources)),
        }))
    }

    pub(crate) fn for_compilation(default_package_root: PathBuf, toolchain_root: PathBuf) -> Self {
        Self {
            default_package_root: normalize_directory(default_package_root),
            toolchain_root: Some(normalize_directory(toolchain_root)),
            ..Self::default()
        }
    }

    pub(crate) fn for_package_compilation(
        default_package_root: PathBuf,
        default_package_identity: PackageKeyIdentity,
        toolchain_root: PathBuf,
        filesystem: source::SourceFileSystem,
    ) -> Self {
        Self {
            default_package_root: filesystem
                .canonicalize(&default_package_root)
                .unwrap_or(default_package_root),
            filesystem,
            default_package_identity: Some(default_package_identity),
            toolchain_root: Some(normalize_directory(toolchain_root)),
            ..Self::default()
        }
    }

    pub(crate) fn register_reconciled_package_root(
        &mut self,
        package_root: PathBuf,
        identity: PackageKeyIdentity,
    ) {
        self.register_package_source(package_root, Some(identity));
    }

    /// Register one exact source path as closed toolchain contract custody.
    /// Used only for the bundled hosted-entry fallback. The file keeps its
    /// contract root so `package_relative_source` retains the canonical path.
    /// A reconciled supplier must not be registered here: doing so would erase
    /// its ordinary package identity and defeat accepted-binding replay.
    pub(crate) fn register_toolchain_contract_root(
        &mut self,
        path: PathBuf,
        contract_root: PathBuf,
    ) {
        self.toolchain_contract_custody.roots.insert(
            normalize_directory(path),
            normalize_directory(contract_root),
        );
    }

    /// The bundled contract fallback owns an entire closed toolchain subtree:
    /// its transitive `use` closure resolves inside the same root, so every
    /// file under it inherits the contract's toolchain custody rather than a
    /// package identity it cannot have.
    pub(crate) fn register_toolchain_contract_dir(&mut self, root: PathBuf) {
        let root = normalize_directory(root);
        if !self.toolchain_contract_custody.dirs.contains(&root) {
            self.toolchain_contract_custody.dirs.push(root);
            self.toolchain_contract_custody
                .dirs
                .sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
        }
    }

    /// A path registered for toolchain contract custody clears the package
    /// source frontier even when it sits outside every reconciled root: its
    /// origin and identity are fixed toolchain by registration, so loading it
    /// cannot launder package content into the compiled program.
    pub(crate) fn is_toolchain_contract_source(&self, path: &Path) -> bool {
        self.toolchain_contract_custody.root_for(path).is_some()
    }

    fn register_package_source(
        &mut self,
        package_root: PathBuf,
        identity: Option<PackageKeyIdentity>,
    ) {
        let package_root = RegisteredPackageRoot {
            root: self
                .filesystem
                .canonicalize(&package_root)
                .unwrap_or(package_root),
            identity,
        };
        if !self.package_roots.contains(&package_root) {
            self.package_roots.push(package_root);
            self.package_roots
                .sort_by_key(|registered| std::cmp::Reverse(registered.root.components().count()));
        }
    }

    /// Assign custody to captured bytes before the parser stamps source spans.
    /// Retained exact instances keep their coordinates even when the current
    /// import frontier visits them in a different order.
    pub fn admit_source(
        &mut self,
        contents: source::SourceContents,
        origin_override: Option<SourceOrigin>,
        resolution_stratum: source::SourceResolutionStratum,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<source::LoadedSource, Vec<Diagnostic>> {
        let (package_root, package_identity, origin) =
            self.source_metadata(&contents.path, origin_override);
        let admitted = catalogue
            .admit_checked_instance(
                &mut self.sources,
                contents.path,
                contents.source,
                package_root,
                package_identity,
                origin,
                resolution_stratum,
                contents.scope,
            )
            .map_err(|message| vec![Diagnostic::error(message)])?;
        Ok(source::LoadedSource {
            source_id: admitted.source_id,
            path: admitted.path.clone(),
            source: admitted.source.clone(),
            origin: Some(admitted.origin),
            scope: admitted.dependency_scope,
        })
    }

    /// Parser roots already name the admitted records. Attaching them allocates
    /// no source coordinates and cannot change their custody.
    pub fn attach_parsed(&mut self, mut parsed: ParsedSources) {
        for parsed_source in parsed.sources.span_mut_or_empty(parsed.batch) {
            // Import discovery has finished with this consumed batch. Keep its
            // per-source associations without copying their owned buffers.
            self.source_roots.append(SourceRoots {
                source_id: parsed_source.source_id,
                root_items: std::mem::take(&mut parsed_source.root_items),
            });
        }
    }

    pub fn file_count(&self) -> usize {
        self.source_roots.len()
    }

    fn source_metadata(
        &self,
        path: &Path,
        origin_override: Option<SourceOrigin>,
    ) -> (PathBuf, Option<PackageKeyIdentity>, SourceOrigin) {
        if let Some(contract_root) = self.toolchain_contract_custody.root_for(path) {
            return (contract_root.to_path_buf(), None, SourceOrigin::Toolchain);
        }
        if let Some(toolchain_root) = &self.toolchain_root
            && path.starts_with(toolchain_root)
        {
            let toolchain_is_core =
                toolchain_root.file_name().and_then(|name| name.to_str()) == Some("core");
            let core_roots = if toolchain_is_core {
                vec![toolchain_root.clone()]
            } else {
                vec![
                    toolchain_root.join("core"),
                    toolchain_root.join("language").join("core"),
                ]
            };
            if let Some(core_root) = core_roots
                .into_iter()
                .filter(|root| path.starts_with(root))
                .max_by_key(|root| root.components().count())
            {
                return (core_root, None, SourceOrigin::Toolchain);
            }
        }

        // Explicit parser-origin tags must not rebase core onto the broader
        // library root. Implicit and explicit imports keep one core owner.
        if origin_override == Some(SourceOrigin::Toolchain) {
            return (
                self.toolchain_root.clone().unwrap_or_default(),
                None,
                SourceOrigin::Toolchain,
            );
        }

        let package = self
            .package_roots
            .iter()
            .find(|package| path.starts_with(&package.root));
        match package {
            Some(package) => (package.root.clone(), package.identity, SourceOrigin::User),
            None => (
                self.default_package_root.clone(),
                self.default_package_identity,
                SourceOrigin::User,
            ),
        }
    }
}

fn normalize_directory(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

/// Closed toolchain contract custody shared by source metadata, the package
/// source frontier, and import reconciliation: exact contract files map to
/// their registered root, while bundled contract fallback roots admit their
/// whole closed subtree.
#[derive(Clone, Default)]
pub(crate) struct ToolchainContractCustody {
    roots: BTreeMap<PathBuf, PathBuf>,
    dirs: Vec<PathBuf>,
}

impl ToolchainContractCustody {
    pub(crate) fn root_for(&self, path: &Path) -> Option<&Path> {
        self.roots.get(path).map(PathBuf::as_path).or_else(|| {
            self.dirs
                .iter()
                .find(|dir| path.starts_with(dir.as_path()))
                .map(PathBuf::as_path)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Root associations borrow path and custody through the existing source id.
/// SourceMap owns the only retained path for each checked source instance.
pub(crate) struct SourceRoots {
    pub source_id: source::SourceId,
    pub root_items: Vec<syntax_trees::item::ItemHandle>,
}

pub(crate) fn validate_package_source_frontier(
    frontier: Vec<(PathBuf, source::DependencyScope)>,
    package_inputs: &crate::PackageCompilationInputs,
    source_storage: &SourceStorage,
) -> Result<Vec<(PathBuf, source::DependencyScope)>, Vec<Diagnostic>> {
    let toolchain_root = super::bundled_core_root()?;
    let mut validated = Vec::with_capacity(frontier.len());
    let mut diagnostics = Vec::new();

    for (source, scope) in frontier {
        let canonical = match source_storage.filesystem.canonicalize(&source) {
            Ok(canonical) => canonical,
            Err(error) => {
                diagnostics.push(Diagnostic::error(format!(
                    "failed to canonicalize package source {} before loading: {error}",
                    source.display()
                )));
                continue;
            }
        };
        if canonical.starts_with(&toolchain_root)
            || source_storage.is_toolchain_contract_source(&canonical)
        {
            validated.push((canonical, scope));
            continue;
        }

        let Some(owner) = package_inputs.package_for_source(&canonical) else {
            diagnostics.push(Diagnostic::error(format!(
                "package source {} escapes every reconciled source root",
                canonical.display()
            )));
            continue;
        };
        if owner != package_inputs.root()
            && canonical.file_name().and_then(|name| name.to_str()) == Some("build.omg")
        {
            diagnostics.push(Diagnostic::error(format!(
                "dependency build file {} may not join the compiled program",
                canonical.display()
            )));
            continue;
        }
        validated.push((canonical, scope));
    }

    if diagnostics.is_empty() {
        Ok(validated)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::SourceStorage;
    use source::{
        DependencyScope, SourceId, SourceOrigin, SourceResolutionStratum, SourceSpan, Span,
    };
    use std::{path::PathBuf, sync::Arc};
    use syntax_trees::{ParsedSource, ParsedSources};

    #[test]
    fn parsed_instances_retain_source_bytes_with_independent_checked_scopes() {
        let text: Arc<str> = Arc::from("public data Shared {} // retained bytes");
        let retained = Arc::downgrade(&text);
        let mut storage = SourceStorage::default();
        let mut catalogue = source::SourceCatalogue::default();
        let loaded = [DependencyScope::Build, DependencyScope::Product].map(|scope| {
            storage
                .admit_source(
                    source::SourceContents {
                        path: PathBuf::from("shared.omg"),
                        source: text.clone(),
                        scope,
                    },
                    Some(SourceOrigin::Toolchain),
                    SourceResolutionStratum::Base,
                    &mut catalogue,
                )
                .unwrap()
        });
        let mut parsed = ParsedSources::default();
        parsed.batch = parsed
            .sources
            .insert_many(loaded.into_iter().map(|loaded| ParsedSource {
                source_id: loaded.source_id,
                path: loaded.path,
                source: loaded.source,
                origin: loaded.origin,
                root_items: Vec::new(),
                import_root_range: 0..0,
                scope: loaded.scope,
            }));
        storage.attach_parsed(parsed);
        let build = storage.sources.get(SourceId(0)).expect("build instance");
        let product = storage.sources.get(SourceId(1)).expect("product instance");
        assert_eq!(storage.source_roots.len(), 2);
        for (_, roots) in storage.source_roots.iter() {
            assert!(storage.sources.get(roots.source_id).is_some());
        }
        assert!(Arc::ptr_eq(&build.source, &text));
        assert!(Arc::ptr_eq(&product.source, &text));
        assert_ne!(build.source_id, product.source_id);
        assert_eq!(build.dependency_scope, DependencyScope::Build);
        assert_eq!(product.dependency_scope, DependencyScope::Product);
        assert_eq!(build.origin, SourceOrigin::Toolchain);
        assert_eq!(product.origin, SourceOrigin::Toolchain);
        assert_eq!(build.resolution_stratum, SourceResolutionStratum::Base);
        assert_eq!(product.resolution_stratum, SourceResolutionStratum::Base);
        let build_span = SourceSpan::new(build.source_id, Span::new(12, 18));
        let product_span = SourceSpan::new(product.source_id, Span::new(12, 18));
        assert!(
            !storage
                .sources
                .same_checked_instance(build_span, product_span)
        );
        assert!(
            !storage
                .sources
                .reference_can_see_declaration(build_span, product_span)
        );
        drop(text);
        assert!(retained.upgrade().is_some());
        assert_eq!(storage.sources.text_at(product_span), "Shared");
        drop(storage);
        assert!(retained.upgrade().is_none());
    }

    #[test]
    fn source_admission_keeps_retained_coordinates_in_current_frontier_order() {
        let mut catalogue = source::SourceCatalogue::default();
        let mut original = SourceStorage::default();
        let contents = |name: &str| source::SourceContents {
            path: PathBuf::from(name),
            source: Arc::from("retained contents"),
            scope: DependencyScope::Product,
        };
        for name in ["first.omg", "unused.omg", "last.omg"] {
            original
                .admit_source(
                    contents(name),
                    Some(SourceOrigin::Toolchain),
                    SourceResolutionStratum::Base,
                    &mut catalogue,
                )
                .unwrap();
        }
        let mut current = SourceStorage {
            sources: original.sources.fork_source_selection().unwrap(),
            ..SourceStorage::default()
        };
        let loaded = ["last.omg", "first.omg", "new.omg"].map(|name| {
            current
                .admit_source(
                    contents(name),
                    Some(SourceOrigin::Toolchain),
                    SourceResolutionStratum::Base,
                    &mut catalogue,
                )
                .unwrap()
        });
        assert_eq!(loaded.each_ref().map(|file| file.source_id.0), [2, 0, 3]);
        let mut parsed = ParsedSources::default();
        parsed.batch = parsed
            .sources
            .insert_many(loaded.into_iter().map(|file| ParsedSource {
                source_id: file.source_id,
                path: file.path,
                source: file.source,
                origin: file.origin,
                root_items: Vec::new(),
                import_root_range: 0..0,
                scope: file.scope,
            }));
        current.attach_parsed(parsed);
        assert_eq!(current.sources.len(), 4);
        assert_eq!(current.file_count(), 3);
        assert_eq!(
            current
                .source_roots
                .iter()
                .map(|(_, roots)| roots.source_id.0)
                .collect::<Vec<_>>(),
            [2, 0, 3]
        );
        assert!(!current.sources.source_is_selected(SourceId(1)));
        assert_eq!(
            current.sources.get(SourceId(1)),
            original.sources.get(SourceId(1))
        );
        assert!(current.sources.retains_source_storage(&original.sources));
    }
    #[test]
    fn implicit_explicit_and_package_core_imports_keep_the_same_owner() {
        let base = std::env::temp_dir().join("omega-core-source-metadata");
        let library = base.join("library");
        let core = library.join("core");
        let path = core.join("storage.omg");
        let loose = SourceStorage::for_compilation(base.join("application"), library);
        let implicit = loose.source_metadata(&path, None);
        let explicit = loose.source_metadata(&path, Some(SourceOrigin::Toolchain));
        assert_eq!(
            implicit, explicit,
            "a parser origin tag must not rename core"
        );
        assert_eq!(implicit.0, core);
        assert_eq!(implicit.1, None);
        assert_eq!(implicit.2, SourceOrigin::Toolchain);
        let supplied = SourceStorage::for_package_compilation(
            base.join("application"),
            semantic_vocabulary::PackageKeyIdentity::from_digest([29; 32]).unwrap(),
            core,
            source::SourceFileSystem::new(),
        );
        assert_eq!(
            implicit,
            supplied.source_metadata(&path, Some(SourceOrigin::Toolchain))
        );
    }
}
