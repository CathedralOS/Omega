// Source routing (`ProductQueryInputs`, `ProductQueryDependency`) for a paused `Build`'s product queries.
//
// While a `Build` machine is paused at a product query, it may inspect its frozen
// authored `Product` namespace alongside any already-settled `ProductQueryDependency`
// graphs. Mounting those dependency graphs into `SourceStorage` verifies that all
// shared packages agree on exact source custody (`require_same_shared_product_sources`)
// and that no query dependency overrides an already-admitted product alias, without
// settling the root `Build`. Anonymous roots remain unbound; an already-issued
// named declaration supplies the acquired owner for its exact captured source.

use super::LogicalSourceDiscovery as _;
use super::bindings::{ImportOccurrence, ResolvedSourceImport};
use super::paths::{is_bundled_core_path, is_bundled_omega_path};
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
    /// The handoff of the dependency's own Build, when the dependency is
    /// mounted for a Build stage: the source that Build generated is part of
    /// the package the stage may call. A Product query describes authored
    /// declarations and mounts none.
    pub generated: Option<crate::PackageBuildHandoff>,
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
            generated: None,
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
    #[test]
    fn named_build_stage_keeps_dependency_product_and_build_authority_distinct() {
        let root = graph(1, package(1, "root_app", "root"));
        let helper = PackageCompilationInputs::new_package(
            identity(2),
            vec![
                package(2, "helper", "helper"),
                package(3, "private_build_tool", "build"),
                package(4, "library", "product"),
            ],
            vec![
                PackageDependencyBinding::for_purpose(
                    identity(2),
                    "tool",
                    identity(3),
                    DependencyPurpose::Build,
                ),
                PackageDependencyBinding::new(identity(2), "library", identity(4)),
            ],
        )
        .unwrap();
        let inputs = ProductQueryInputs::for_build_stage(None, vec![dependency("helper", helper)])
            .unwrap()
            .with_named_source(Some(root))
            .unwrap()
            .build_stage_authority_inputs()
            .unwrap()
            .unwrap();
        assert_eq!(
            inputs.dependency_target_for_purpose(identity(1), DependencyPurpose::Build, "helper"),
            Some(identity(2))
        );
        assert_eq!(
            inputs.dependency_target_for_purpose(identity(1), DependencyPurpose::Product, "helper"),
            None
        );
        assert_eq!(
            inputs.dependency_target_for_purpose(
                identity(2),
                DependencyPurpose::Product,
                "library"
            ),
            Some(identity(4))
        );
        assert_eq!(
            inputs.package_root(identity(3)),
            None,
            "the helper's Build activation grants this stage no source"
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductQueryInputs {
    original: Option<PackageCompilationInputs>,
    /// Only the named root's acquired source. Dependency visibility still
    /// comes from the original graph or the requests joined for this query.
    named_source: Option<PackageCompilationInputs>,
    dependencies: Vec<ProductQueryDependency>,
    /// The build dependencies a staged Build has prepared, by the alias its
    /// Build sources import them under (`for_build_stage`). A Product query
    /// mounts none.
    build_dependencies: Vec<ProductQueryDependency>,
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
            if let Some(original) = &original
                && original.dependency_target_for_purpose(
                    original.root(),
                    DependencyPurpose::Product,
                    &dependency.alias,
                ) != Some(dependency.inputs.root())
            {
                return Err(
                    "Product query dependency differs from its originally admitted alias".into(),
                );
            }
        }
        Self {
            original,
            named_source: None,
            dependencies,
            build_dependencies: Vec::new(),
        }
        .with_consistent_sources()
    }

    /// The source routing of one stage of a staged Build after its first:
    /// the Build's own sources, with each build dependency an earlier stage
    /// prepared selectable from them under its alias
    /// (`wiki/spec/build/scoped_execution.md`, "Dependency declarations and
    /// binding": "After preparation, the compiler resolves and checks the
    /// dependent continuation before it executes"). No Product dependency is
    /// mounted: "Product dependencies supply no callable Build authority".
    pub fn for_build_stage(
        original: Option<PackageCompilationInputs>,
        build_dependencies: Vec<ProductQueryDependency>,
    ) -> Result<Self, String> {
        Self::new(original, Vec::new())?.with_prepared_build_dependencies(build_dependencies)
    }

    /// Reuse an existing complete graph when supplied. Otherwise a named
    /// capture and the exact prepared Build dependencies define this stage's
    /// source selections; they make no claim about later Product requests.
    pub fn build_stage_authority_inputs(&self) -> Result<Option<PackageCompilationInputs>, String> {
        if let Some(original) = &self.original {
            return Ok(Some(original.clone()));
        }
        self.named_source
            .as_ref()
            .map(|root| root.with_prepared_build_sources(&self.build_dependencies))
            .transpose()
    }

    /// A named root receives only the owner supplied by acquired-source
    /// admission. Its exact capture is joined at mounting; this data neither
    /// completes Build nor adds an unrequested dependency to its namespace.
    pub fn with_named_source(
        mut self,
        source: Option<PackageCompilationInputs>,
    ) -> Result<Self, String> {
        let Some(source) = source else {
            return Ok(self);
        };
        if let Some(original) = &self.original {
            if original.root() != source.root() || original.root_role() != source.root_role() {
                return Err("named query source differs from its admitted owner or role".into());
            }
            original.require_same_shared_product_sources(&source)?;
            return Ok(self);
        }
        self.named_source = Some(source);
        self.with_consistent_sources()
    }

    /// The same routing with `build_dependencies` also mounted for the
    /// root's Build sources. A query asked by a later stage of a staged
    /// Build is answered on such a frontier: its author may be a source of a
    /// package that stage calls, and must be found there. A build dependency
    /// the query already mounts under the same alias is mounted once.
    pub fn with_prepared_build_dependencies(
        mut self,
        mut build_dependencies: Vec<ProductQueryDependency>,
    ) -> Result<Self, String> {
        build_dependencies.retain(|prepared| {
            !self
                .dependencies
                .iter()
                .any(|mounted| mounted.alias == prepared.alias)
        });
        let original = &self.original;
        for (position, dependency) in build_dependencies.iter().enumerate() {
            if dependency.alias.is_empty()
                || build_dependencies[..position]
                    .iter()
                    .any(|previous| previous.alias == dependency.alias)
            {
                return Err("prepared build dependency aliases must be nonempty and unique".into());
            }
            if dependency.inputs.root_role() != build_declarations::BuildDeclarationKind::Package {
                return Err("prepared build dependency must declare a package".into());
            }
            if let Some(original) = original
                && original.dependency_target_for_purpose(
                    original.root(),
                    DependencyPurpose::Build,
                    &dependency.alias,
                ) != Some(dependency.inputs.root())
            {
                return Err(
                    "prepared build dependency differs from its originally admitted alias".into(),
                );
            }
        }
        self.build_dependencies = build_dependencies;
        self.with_consistent_sources()
    }

    fn with_consistent_sources(self) -> Result<Self, String> {
        let graphs = self
            .graphs()
            .chain(self.named_source.iter())
            .collect::<Vec<_>>();
        for (position, graph) in graphs.iter().enumerate() {
            for previous in &graphs[..position] {
                graph.require_same_shared_product_sources(previous)?;
            }
        }
        Ok(self)
    }

    pub(crate) fn build_dependencies(&self) -> &[ProductQueryDependency] {
        &self.build_dependencies
    }

    /// The mounted dependency a source of the unbound root selects by
    /// `alias`. A query's dependencies are selectable from every source of
    /// the root, as they were before stages: a Build names the provider it
    /// waits for through one. What a stage mounts is selectable from Build
    /// sources only, so a Product source loaded with the stage gains nothing
    /// from the Build's preparations: "Product dependencies supply no
    /// callable Build authority", and the reverse
    /// (`wiki/spec/build/scoped_execution.md`, "Two checked contexts").
    fn mounted_for(
        &self,
        scope: source::DependencyScope,
        alias: &str,
    ) -> Option<&ProductQueryDependency> {
        let staged: &[ProductQueryDependency] = match scope {
            source::DependencyScope::Product => &[],
            source::DependencyScope::Build => &self.build_dependencies,
        };
        self.dependencies
            .iter()
            .chain(staged)
            .find(|dependency| dependency.alias == alias)
    }

    pub(crate) fn original(&self) -> Option<&PackageCompilationInputs> {
        self.original.as_ref()
    }

    fn graphs(&self) -> impl Iterator<Item = &PackageCompilationInputs> {
        self.original.iter().chain(
            self.dependencies
                .iter()
                .chain(&self.build_dependencies)
                .map(|dependency| &dependency.inputs),
        )
    }

    pub(crate) fn admit_source_owner(
        &self,
        storage: &mut SourceStorage,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(named) = &self.named_source {
            // Source ownership is joined before the new consumer is selected.
            // Visibility is installed afterward, once, on that fresh cohort.
            storage.join_package_inputs(named)?;
        }
        Ok(())
    }

    pub(crate) fn mount(&self, storage: &mut SourceStorage) -> Result<(), Vec<Diagnostic>> {
        if let Some(named) = &self.named_source {
            storage
                .sources
                .retain_product_dependency_scope(self.dependencies.iter().map(|dependency| {
                    (
                        named.root(),
                        dependency.alias.as_str(),
                        dependency.inputs.root(),
                    )
                }));
        }
        for graph in self.graphs() {
            storage.require_disjoint_contract_packages(graph)?;
        }
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
                let (root, paths, prefix) = if let Some(inputs) = owner {
                    let requester = inputs.package_for_source(&authored.path);
                    // A mounted dependency's sources resolve through its own
                    // Product edges and by the Product acquisition rule,
                    // whichever context loaded them: "A build dependency's
                    // ordinary dependencies are its library implementation
                    // dependencies in the host/build graph", and "Its own
                    // build dependencies belong to its separate build
                    // activation" (scoped_execution.md, "Two checked
                    // contexts").
                    let mounted = !self
                        .original
                        .as_ref()
                        .is_some_and(|original| std::ptr::eq(original, inputs));
                    let (purpose, importer) = if mounted {
                        (DependencyPurpose::Product, source::DependencyScope::Product)
                    } else {
                        (
                            super::source_import_scope(inputs, authored.scope, &authored.path),
                            authored.scope,
                        )
                    };
                    let selected = reconciled_package_import(
                        syntax,
                        &authored.path,
                        members,
                        requester,
                        purpose,
                        storage.toolchain_contract_custody.root_for(&authored.path),
                        inputs,
                    )?;
                    match selected {
                        ReconciledPackageImport::Toolchain { root, logical_path }
                        | ReconciledPackageImport::ToolchainContract { root, logical_path } => {
                            let prefix = members.len() - logical_path.len();
                            let paths = storage.package_roots.select(
                                &root,
                                &logical_path,
                                &storage.filesystem,
                            )?;
                            (root, paths, prefix)
                        }
                        ReconciledPackageImport::Package(request) => {
                            let prefix = members.len() - request.relative_path.components().count();
                            let paths = request.logical_sources(
                                inputs,
                                &mut storage.package_roots,
                                importer,
                            )?;
                            if paths.is_empty() && request.defers_in(importer) {
                                continue;
                            }
                            (request.expected_root, paths, prefix)
                        }
                    }
                } else {
                    let first = members
                        .first()
                        .ok_or_else(|| vec![Diagnostic::error("empty Product query import")])?;
                    let (root, prefix) = if is_bundled_core_path(syntax, members) {
                        (super::bundled_core_root()?, 3)
                    } else if is_bundled_omega_path(syntax, members) {
                        return Err(vec![Diagnostic::error(
                            "Product query library imports require an issued Product dependency",
                        )]);
                    } else if let Some(root) =
                        storage.toolchain_contract_custody.root_for(&authored.path)
                    {
                        (root.to_path_buf(), 0)
                    } else if !authored.path.starts_with(storage.default_package_root()) {
                        let core = super::bundled_core_root()?;
                        if !authored.path.starts_with(&core) {
                            return Err(vec![Diagnostic::error(
                                "Product query source has no admitted package scope",
                            )]);
                        }
                        (core, 0)
                    } else if let Some(dependency) =
                        self.mounted_for(authored.scope, first.as_str(syntax))
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
                    let logical = members[prefix..]
                        .iter()
                        .map(|name| name.as_str(syntax).to_owned())
                        .collect::<Vec<_>>();
                    // The Build sources of the unbound root keep their
                    // deferred imports while a query is answered.
                    let own_build_import = authored.scope == source::DependencyScope::Build
                        && prefix == 0
                        && root == storage.default_package_root();
                    let paths = if own_build_import {
                        storage.package_roots.select_declaring(
                            &root,
                            &logical,
                            &storage.filesystem,
                            Vec::new(),
                        )?
                    } else {
                        storage
                            .package_roots
                            .select(&root, &logical, &storage.filesystem)?
                    };
                    if paths.is_empty() && own_build_import {
                        continue;
                    }
                    (root, paths, prefix)
                };
                if paths.is_empty() {
                    return Err(vec![Diagnostic::error(
                        "Product query selects no available logical source namespace",
                    )]);
                }
                let _ = root;
                for path in paths {
                    storage.resolved_imports.push(ResolvedSourceImport {
                        occurrence: ImportOccurrence::new(
                            syntax,
                            authored.source_id,
                            authored.import_root_range.start + offset,
                            members,
                            prefix,
                        ),
                        path: path.clone(),
                    });
                    imports.push((path, authored.scope));
                }
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
