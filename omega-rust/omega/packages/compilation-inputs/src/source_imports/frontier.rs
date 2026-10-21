//! Parsed source instances with exact package and toolchain ownership.

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
    pub files: Arena<SourceFile>,
    pub sources: SourceMap,
    pub syntax_trees: SyntaxTrees,
    pub resolved_imports: Vec<super::ResolvedSourceImport>,
    default_package_root: PathBuf,
    default_package_identity: Option<PackageKeyIdentity>,
    package_roots: Vec<RegisteredPackageRoot>,
    toolchain_root: Option<PathBuf>,
    toolchain_contract_roots: BTreeMap<PathBuf, PathBuf>,
    toolchain_contract_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegisteredPackageRoot {
    root: PathBuf,
    identity: Option<PackageKeyIdentity>,
}

impl SourceStorage {
    /// Initialize exact package roots and product dependency authority before loading.
    /// Build-only dependencies never authorize product source selection.
    pub fn prepare(
        root_path: &Path,
        package_inputs: Option<&crate::PackageCompilationInputs>,
    ) -> Result<Self, Vec<Diagnostic>> {
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
                let root_package = root_path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."));
                Ok(SourceStorage::for_compilation(
                    root_package,
                    super::bundled_omega_root()?,
                ))
            }
        }
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
        self.toolchain_contract_roots.insert(
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
        if !self.toolchain_contract_dirs.contains(&root) {
            self.toolchain_contract_dirs.push(root);
            self.toolchain_contract_dirs
                .sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
        }
    }

    /// A path registered for toolchain contract custody clears the package
    /// source frontier even when it sits outside every reconciled root: its
    /// origin and identity are fixed toolchain by registration, so loading it
    /// cannot launder package content into the compiled program.
    pub(crate) fn is_toolchain_contract_source(&self, path: &Path) -> bool {
        self.toolchain_contract_root_for(path).is_some()
    }

    /// Snapshot toolchain contract custody for import reconciliation, where a
    /// mutable `resolved_imports` borrow forbids retaining `&self`.
    pub(crate) fn toolchain_contract_custody(&self) -> ToolchainContractCustody {
        ToolchainContractCustody {
            roots: self.toolchain_contract_roots.clone(),
            dirs: self.toolchain_contract_dirs.clone(),
        }
    }

    fn toolchain_contract_root_for(&self, path: &Path) -> Option<&Path> {
        toolchain_contract_root_for(
            &self.toolchain_contract_roots,
            &self.toolchain_contract_dirs,
            path,
        )
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

    pub fn extend(&mut self, parsed: ParsedSources) -> Result<(), Vec<Diagnostic>> {
        self.extend_in_stratum(parsed, source::SourceResolutionStratum::Base)
    }

    /// Join a dependency's generated-source bundle units: assembled with the
    /// base like any loaded source, but marked so their own package's
    /// authored units cannot select them (`SourceResolutionStratum`).
    pub fn extend_dependency_generated(
        &mut self,
        parsed: ParsedSources,
    ) -> Result<(), Vec<Diagnostic>> {
        self.extend_in_stratum(
            parsed,
            source::SourceResolutionStratum::DependencyActivationExtension,
        )
    }

    fn extend_in_stratum(
        &mut self,
        parsed: ParsedSources,
        resolution_stratum: source::SourceResolutionStratum,
    ) -> Result<(), Vec<Diagnostic>> {
        for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
            let (package_root, package_identity, origin) =
                self.source_metadata(&parsed_source.path, parsed_source.origin);
            let added = self.sources.add_checked_instance(
                parsed_source.path.clone(),
                parsed_source.source.clone(),
                package_root,
                package_identity,
                origin,
                resolution_stratum,
                parsed_source.scope,
            );

            debug_assert_eq!(added.source_id, parsed_source.source_id);

            self.files.append(SourceFile {
                source_id: parsed_source.source_id,
                path: parsed_source.path.clone(),
                root_items: parsed_source.root_items.clone(),
            });
        }

        Ok(())
    }

    pub(crate) fn next_source_id(&self) -> usize {
        self.sources.len()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    fn source_metadata(
        &self,
        path: &Path,
        origin_override: Option<SourceOrigin>,
    ) -> (PathBuf, Option<PackageKeyIdentity>, SourceOrigin) {
        if let Some(contract_root) = self.toolchain_contract_root_for(path) {
            return (contract_root.to_path_buf(), None, SourceOrigin::Toolchain);
        }
        if origin_override == Some(SourceOrigin::Toolchain) {
            return (
                self.toolchain_root.clone().unwrap_or_default(),
                None,
                SourceOrigin::Toolchain,
            );
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
        toolchain_contract_root_for(&self.roots, &self.dirs, path)
    }
}

fn toolchain_contract_root_for<'a>(
    roots: &'a BTreeMap<PathBuf, PathBuf>,
    dirs: &'a [PathBuf],
    path: &Path,
) -> Option<&'a Path> {
    roots.get(path).map(PathBuf::as_path).or_else(|| {
        dirs.iter()
            .find(|dir| path.starts_with(dir.as_path()))
            .map(PathBuf::as_path)
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFile {
    pub source_id: source::SourceId,
    pub path: PathBuf,
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
        let mut parsed = ParsedSources::default();
        parsed.batch = parsed.sources.insert_many(
            [DependencyScope::Build, DependencyScope::Product]
                .into_iter()
                .enumerate()
                .map(|(index, scope)| ParsedSource {
                    source_id: SourceId(index),
                    path: PathBuf::from("shared.omg"),
                    source: text.clone(),
                    origin: Some(SourceOrigin::Toolchain),
                    root_items: Vec::new(),
                    scope,
                }),
        );
        let mut storage = SourceStorage::default();
        storage.extend(parsed).expect("retained source batch");
        let build = storage.sources.get(SourceId(0)).expect("build instance");
        let product = storage.sources.get(SourceId(1)).expect("product instance");
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
}
