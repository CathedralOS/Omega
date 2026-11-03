//! A query's issued Product dependencies, without an unfinished root owner.
//!
//! Dependency graphs already have their own source custody. Their aliases are
//! local to this invocation; mounting them neither completes the root Build nor
//! assigns a package identity to its captured source. Generated source and
//! executable bodies are not query inputs.

use super::bindings::{ImportOccurrence, ResolvedSourceImport, direct_source_import};
use super::paths::{is_bundled_core_path, is_bundled_omega_path, resolve_reconciled_import};
use super::{ReconciledPackageImport, SourceStorage, reconciled_package_import};
use crate::PackageCompilationInputs;
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use std::path::PathBuf;
use syntax_trees::{ParsedSources, item::Item};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductQueryDependency {
    pub alias: String,
    pub inputs: PackageCompilationInputs,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildDeclarationKind, PackageDependencyBinding, PackageSourceBinding};
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{CapturedSourceNode, CapturedSourceTree};
    use std::sync::Arc;

    fn package(number: u8, name: &str, text: &str) -> PackageSourceBinding {
        PackageSourceBinding::new(
            PackageKeyIdentity::from_digest([number; 32]).unwrap(),
            name,
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name),
        )
        .with_captured_source(Arc::new(
            CapturedSourceTree::new([(
                PathBuf::from("main.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(text.as_bytes()),
                    executable: false,
                },
            )])
            .unwrap(),
        ))
        .unwrap()
    }

    fn identity(number: u8) -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([number; 32]).unwrap()
    }

    fn graph(number: u8, package: PackageSourceBinding) -> PackageCompilationInputs {
        PackageCompilationInputs::new(
            identity(number),
            BuildDeclarationKind::Package,
            vec![package],
            Vec::new(),
        )
        .unwrap()
    }

    fn dependency(alias: &str, inputs: PackageCompilationInputs) -> ProductQueryDependency {
        ProductQueryDependency {
            alias: alias.into(),
            inputs,
        }
    }

    #[test]
    fn query_graphs_require_consistent_shared_source_custody() {
        let original = graph(1, package(1, "query_library", "original"));
        ProductQueryInputs::new(
            None,
            vec![
                dependency("first", original.clone()),
                dependency("second", original.clone()),
            ],
        )
        .unwrap();
        for changed in [
            graph(1, package(1, "query_library", "changed bytes")),
            graph(2, package(2, "query_library", "original")),
        ] {
            assert!(
                ProductQueryInputs::new(
                    None,
                    vec![
                        dependency("first", original.clone()),
                        dependency("second", changed),
                    ]
                )
                .is_err()
            );
        }
        assert!(
            ProductQueryInputs::new(
                None,
                vec![
                    dependency("same", original.clone()),
                    dependency("same", original),
                ]
            )
            .is_err()
        );
    }

    #[test]
    fn query_dependencies_cannot_replace_an_admitted_product_alias() {
        let owner = package(1, "query_owner", "owner");
        let library = package(2, "query_library", "library");
        let original = PackageCompilationInputs::new(
            identity(1),
            BuildDeclarationKind::Application,
            vec![owner, library.clone()],
            vec![PackageDependencyBinding::for_purpose(
                identity(1),
                "library",
                identity(2),
                DependencyPurpose::Product,
            )],
        )
        .unwrap();
        let library = graph(2, library);
        ProductQueryInputs::new(
            Some(original.clone()),
            vec![dependency("library", library.clone())],
        )
        .unwrap();
        assert!(
            ProductQueryInputs::new(
                Some(original.clone()),
                vec![dependency("other_alias", library),]
            )
            .is_err()
        );
        assert!(
            ProductQueryInputs::new(
                Some(original),
                vec![dependency(
                    "library",
                    graph(3, package(3, "other_library", "different"))
                ),]
            )
            .is_err()
        );
    }
}

/// Source routing supplied to one paused query. No execution or final root
/// declaration can be obtained from this value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductQueryInputs {
    original: Option<PackageCompilationInputs>,
    dependencies: Vec<ProductQueryDependency>,
}

impl ProductQueryInputs {
    pub fn new(
        original: Option<PackageCompilationInputs>,
        dependencies: Vec<ProductQueryDependency>,
    ) -> Result<Self, String> {
        for (position, dependency) in dependencies.iter().enumerate() {
            if dependency.alias.is_empty()
                || dependencies[..position]
                    .iter()
                    .any(|previous| previous.alias == dependency.alias)
            {
                return Err("Product query dependency aliases must be nonempty and unique".into());
            }
            if dependency.inputs.root_role() != build_declarations::BuildDeclarationKind::Package {
                return Err("Product query dependency must declare a package".into());
            }
            if let Some(original) = &original {
                if original.dependency_target_for_purpose(
                    original.root(),
                    DependencyPurpose::Product,
                    &dependency.alias,
                ) != Some(dependency.inputs.root())
                {
                    return Err(
                        "Product query dependency differs from its originally admitted alias"
                            .into(),
                    );
                }
            }
        }
        let inputs = Self {
            original,
            dependencies,
        };
        let graphs = inputs.graphs().collect::<Vec<_>>();
        for (position, graph) in graphs.iter().enumerate() {
            for previous in &graphs[..position] {
                graph.require_same_shared_product_sources(previous)?;
            }
        }
        Ok(inputs)
    }

    pub(crate) fn original(&self) -> Option<&PackageCompilationInputs> {
        self.original.as_ref()
    }

    fn graphs(&self) -> impl Iterator<Item = &PackageCompilationInputs> {
        self.original.iter().chain(
            self.dependencies
                .iter()
                .map(|dependency| &dependency.inputs),
        )
    }

    pub(crate) fn mount(&self, storage: &mut SourceStorage) -> Result<(), Vec<Diagnostic>> {
        for graph in self.graphs() {
            storage
                .filesystem
                .join_captures(graph.source_filesystem())
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "cannot join query source snapshots: {error}"
                    ))]
                })?;
            for (identity, root) in graph.packages() {
                storage.register_reconciled_package_root(root.to_path_buf(), identity);
            }
            storage
                .sources
                .retain_product_dependency_scope(graph.dependencies());
        }
        if self.original.is_none() {
            storage
                .sources
                .retain_query_dependencies(
                    storage.default_package_root().to_path_buf(),
                    self.dependencies
                        .iter()
                        .map(|dependency| (dependency.alias.clone(), dependency.inputs.root())),
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        }
        Ok(())
    }

    pub(crate) fn discover(
        &self,
        parsed: &ParsedSources,
        storage: &mut SourceStorage,
    ) -> Result<Vec<(PathBuf, source::DependencyScope)>, Vec<Diagnostic>> {
        let syntax = &storage.syntax_trees;
        let mut imports = Vec::new();
        for authored in parsed.sources.span_or_empty(parsed.batch) {
            let owner = self
                .graphs()
                .find(|inputs| inputs.package_for_source(&authored.path).is_some());
            for (offset, item) in authored.root_items[authored.import_root_range.clone()]
                .iter()
                .enumerate()
            {
                let Item::Use(import) = syntax.root_item(*item) else {
                    continue;
                };
                let members = syntax.items.identifier_path_members(import.path);
                let (root, path, prefix) = if let Some(inputs) = owner {
                    let requester = inputs.package_for_source(&authored.path);
                    let selected = reconciled_package_import(
                        syntax,
                        &authored.path,
                        members,
                        requester,
                        super::source_import_scope(inputs, authored.scope, &authored.path),
                        storage.toolchain_contract_custody.root_for(&authored.path),
                        inputs,
                    )?;
                    match selected {
                        ReconciledPackageImport::Toolchain(path) => {
                            (super::bundled_omega_root()?, path, 2)
                        }
                        ReconciledPackageImport::ToolchainContract { root, path } => {
                            (root, path, 0)
                        }
                        ReconciledPackageImport::Package(request) => {
                            let prefix = members.len() - request.relative_path.components().count();
                            let path = request.physical_source(inputs)?.ok_or_else(|| vec![Diagnostic::error(
                                "Product query depends on unavailable or generated source before Build completion",
                            )])?;
                            (request.expected_root, path, prefix)
                        }
                    }
                } else {
                    let first = members
                        .first()
                        .ok_or_else(|| vec![Diagnostic::error("empty Product query import")])?;
                    let (root, prefix) = if is_bundled_core_path(syntax, members) {
                        (super::bundled_omega_root()?, 2)
                    } else if is_bundled_omega_path(syntax, members) {
                        return Err(vec![Diagnostic::error(
                            "Product query library imports require an issued Product dependency",
                        )]);
                    } else if let Some(root) =
                        storage.toolchain_contract_custody.root_for(&authored.path)
                    {
                        (root.to_path_buf(), 0)
                    } else if !authored.path.starts_with(storage.default_package_root()) {
                        // Core and contract files have their own closed namespace;
                        // neither may inherit the anonymous consumer's aliases.
                        let core = super::bundled_core_root()?;
                        if !authored.path.starts_with(&core) {
                            return Err(vec![Diagnostic::error(
                                "Product query source has no admitted package scope",
                            )]);
                        }
                        (core, 0)
                    } else if let Some(dependency) = self
                        .dependencies
                        .iter()
                        .find(|dependency| dependency.alias == first.as_str(syntax))
                    {
                        let root = dependency
                            .inputs
                            .package_root(dependency.inputs.root())
                            .ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "Product query dependency lost its source root",
                                )]
                            })?;
                        (root.to_path_buf(), 1)
                    } else {
                        (storage.default_package_root().to_path_buf(), 0)
                    };
                    let path = resolve_reconciled_import(
                        syntax,
                        root.clone(),
                        &members[prefix..],
                        "Product query",
                        &storage.filesystem,
                    )?;
                    if prefix == 1 && path.file_name().is_some_and(|name| name == "build.omg") {
                        return Err(vec![Diagnostic::error(
                            "Product query cannot import a dependency Build entry",
                        )]);
                    }
                    (root, path, prefix)
                };
                storage.resolved_imports.push(ResolvedSourceImport {
                    occurrence: ImportOccurrence::new(
                        syntax,
                        authored.source_id,
                        authored.import_root_range.start + offset,
                        members,
                        prefix,
                    ),
                    requires_module: !direct_source_import(
                        syntax,
                        &root,
                        &members[prefix..],
                        &path,
                        &storage.filesystem,
                    ),
                    path: path.clone(),
                });
                imports.push((path, authored.scope));
            }
        }
        Ok(imports)
    }

    pub(crate) fn validate_frontier(
        &self,
        frontier: Vec<(PathBuf, source::DependencyScope)>,
        storage: &SourceStorage,
    ) -> Result<Vec<(PathBuf, source::DependencyScope)>, Vec<Diagnostic>> {
        let core = super::bundled_core_root()?;
        let mut validated = Vec::with_capacity(frontier.len());
        for (path, scope) in frontier {
            let canonical = storage.filesystem.canonicalize(&path).map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot load Product query source {}: {error}",
                    path.display()
                ))]
            })?;
            let local = canonical.starts_with(storage.default_package_root());
            let package = self
                .graphs()
                .any(|inputs| inputs.package_for_source(&canonical).is_some());
            if !local
                && !package
                && !canonical.starts_with(&core)
                && !storage.is_toolchain_contract_source(&canonical)
            {
                return Err(vec![Diagnostic::error(
                    "Product query source escapes its admitted source frontier",
                )]);
            }
            if !local
                && package
                && canonical
                    .file_name()
                    .is_some_and(|name| name == "build.omg")
            {
                return Err(vec![Diagnostic::error(
                    "Product query cannot import a dependency Build entry",
                )]);
            }
            validated.push((canonical, scope));
        }
        Ok(validated)
    }

    pub(crate) fn entry_contract_seeds(
        &self,
        storage: &SourceStorage,
    ) -> Result<Vec<super::entry_contract_seed::EntryContractSeed>, Vec<Diagnostic>> {
        super::entry_contract_seed::entry_contract_seeds_from_graphs(
            self.original.as_ref(),
            &self.graphs().collect::<Vec<_>>(),
            storage,
        )
    }
}
