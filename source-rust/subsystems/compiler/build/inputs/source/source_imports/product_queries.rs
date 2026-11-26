// Source routing (`ProductQueryInputs`, `ProductQueryDependency`) for a paused `Build`'s product queries.
//
// While a `Build` machine is paused at a product query, it may inspect its frozen
// authored `Product` namespace alongside any already-settled `ProductQueryDependency`
// graphs. Mounting those dependency graphs into `SourceStorage` verifies that all
// shared packages agree on exact source custody (`require_same_shared_product_sources`)
// and that no query dependency overrides an already-admitted product alias, without
// settling the root `Build`. Anonymous roots remain unbound; an already-issued
// named declaration supplies the acquired owner for its exact captured source.

mod declaration;
use declaration::DeclarationLookup;

use super::SourceStorage;
use crate::PackageCompilationInputs;
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use source_compiler::ParsedSourceBatch;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductQueryDependency {
    pub max_reach: semantic_vocabulary::dependency_reach::DependencyReach,
    pub alias: String,
    pub inputs: PackageCompilationInputs,
    /// The package graph and sources admitted for the child's original Build, when named.
    pub child_build_source: Option<crate::PackageSourceConsumptionCommitment>,
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
    use source_capture::{CapturedSourceNode, CapturedSourceTree};
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
            max_reach: Default::default(),
            alias: alias.into(),
            inputs,
            child_build_source: None,
            generated: None,
        }
    }

    #[test]
    fn joining_shared_package_sources_rejects_different_commits_of_the_same_tree() {
        use source_acquisition::{GitCommitId, GitTreeId, ImmutableSourceResolution};
        let tree = GitTreeId::parse_hex(&"2".repeat(40)).unwrap();
        let package = package(1, "meter", "public machine read() {}");
        let at = |digit: char| {
            let revision = ImmutableSourceResolution::git(
                GitCommitId::parse_hex(&digit.to_string().repeat(40)).unwrap(),
                tree.clone(),
            )
            .unwrap();
            graph(1, package.clone().with_resolution(&revision))
        };
        let same = vec![dependency("first", at('1')), dependency("second", at('1'))];
        ProductQueryInputs::new(None, same).unwrap();
        let different = vec![dependency("first", at('1')), dependency("second", at('3'))];
        let reason = ProductQueryInputs::new(None, different).unwrap_err();
        assert!(
            reason.contains("disagree about a shared package source"),
            "{reason}"
        );
    }

    #[test]
    fn a_product_query_selects_the_alias_in_the_requesting_sources_scope() {
        use source_provenance::DependencyScope::{Build, Product};
        let product = graph(
            1,
            package(1, "product_library", "public machine value() {}"),
        );
        let build = graph(2, package(2, "build_library", "public machine value() {}"));
        let product_only =
            ProductQueryInputs::new(None, vec![dependency("kit", product.clone())]).unwrap();
        assert!(product_only.mounted_for(Build, "kit").is_none());
        let both = product_only
            .with_prepared_build_dependencies(vec![dependency("kit", build.clone())])
            .unwrap();
        assert_eq!(
            both.mounted_for(Build, "kit").unwrap().inputs.root(),
            build.root()
        );
        assert_eq!(
            both.mounted_for(Product, "kit").unwrap().inputs.root(),
            product.root()
        );
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
    declarations: Vec<DeclarationLookup>,
    successor: Option<build_declarations::BuildEntryPath>,
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
            declarations: Vec::new(),
            successor: None,
            dependencies,
            build_dependencies: Vec::new(),
        }
        .with_consistent_sources()
    }

    /// Select the Build sources with the libraries supplied for this invocation. During
    /// migration a staged caller can also select a library its earlier interval prepared.
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

    /// Build sources in a query keep the dependencies prepared by their own
    /// activation. A Product registration may use the same alias for another package.
    pub fn with_prepared_build_dependencies(
        mut self,
        build_dependencies: Vec<ProductQueryDependency>,
    ) -> Result<Self, String> {
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

    fn mounted_for(
        &self,
        scope: source_provenance::DependencyScope,
        alias: &str,
    ) -> Option<&ProductQueryDependency> {
        let dependencies = match scope {
            source_provenance::DependencyScope::Build => &self.build_dependencies,
            source_provenance::DependencyScope::Product => &self.dependencies,
        };
        dependencies
            .iter()
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

    pub(crate) fn resolved_edges(
        &self,
    ) -> Result<Vec<crate::source::anonymous_owner::ResolvedEdge<'_>>, Vec<Diagnostic>> {
        use crate::source::anonymous_owner::ResolvedEdge;
        [
            (DependencyPurpose::Build, &self.build_dependencies),
            (DependencyPurpose::Product, &self.dependencies),
        ]
        .into_iter()
        .flat_map(|(purpose, dependencies)| dependencies.iter().map(move |edge| (purpose, edge)))
        .map(|(purpose, edge)| {
            let owner = edge.inputs.root();
            let resolution = edge.inputs.resolution(owner).ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "dependency `{}` has no immutable resolution for the anonymous Build product",
                    edge.alias,
                ))]
            })?;
            Ok(ResolvedEdge {
                purpose,
                alias: &edge.alias,
                owner,
                resolution,
            })
        })
        .collect()
    }

    pub(crate) fn mount(&self, storage: &mut SourceStorage) -> Result<(), Vec<Diagnostic>> {
        if let Some(owner) = storage.default_package_identity() {
            storage.sources.retain_product_dependency_scope(
                self.dependencies
                    .iter()
                    .map(|dependency| (owner, dependency.alias.as_str(), dependency.inputs.root())),
            );
        }
        for graph in self.graphs() {
            storage.require_disjoint_installed_roots(graph)?;
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
            storage.register_package_graph(graph);
            storage
                .sources
                .retain_product_dependency_scope(graph.dependencies());
        }
        if storage.default_package_identity().is_none() {
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
        parsed: &ParsedSourceBatch,
        storage: &mut SourceStorage,
    ) -> Result<Vec<(PathBuf, source_provenance::DependencyScope)>, Vec<Diagnostic>> {
        storage.capture_imported_build_library(parsed)?;
        let mut imports = Vec::new();
        for authored in parsed.sources.span_or_empty(parsed.batch) {
            let owner = storage
                .sources
                .get(authored.source_id)
                .and_then(|source| source.package_identity)
                .and_then(|owner| {
                    self.graphs()
                        .find(|inputs| inputs.package_for_source(&authored.path) == Some(owner))
                });
            for import in authored
                .imports
                .iter()
                .filter(|import| import.used_in_declaration_query(authored.scope))
            {
                let contract = storage.installed_roots.root_for(&authored.path);
                let mounted_owner = owner.is_some_and(|inputs| {
                    !self
                        .original
                        .as_ref()
                        .is_some_and(|original| std::ptr::eq(original, inputs))
                });
                let purpose = |inputs: &PackageCompilationInputs| {
                    if mounted_owner {
                        DependencyPurpose::Product
                    } else {
                        super::source_import_scope(inputs, authored.scope, &authored.path)
                    }
                };
                let file = super::files::FileImport::from(authored, import);
                // Installed sources do not inherit the caller’s dependency aliases.
                let mount = file
                    .root
                    .filter(|_| contract.is_none())
                    .and_then(|alias| self.mounted_for(authored.scope, alias));
                let mounted = mount.map(|dependency| &dependency.inputs);
                let selected = if let Some(inputs) = owner {
                    file.reconciled(
                        inputs,
                        inputs.package_for_source(&authored.path),
                        purpose(inputs),
                        contract,
                        &storage.filesystem,
                    )?
                } else {
                    let core = super::bundled_core_root()?;
                    let local = contract
                        .or_else(|| authored.path.starts_with(&core).then_some(core.as_path()))
                        .or_else(|| {
                            authored
                                .path
                                .starts_with(storage.default_package_root())
                                .then_some(storage.default_package_root())
                        });
                    file.standalone(local, mounted, &storage.filesystem)?
                };
                let selected = match selected {
                    super::files::FileSelection::Selected(file) => vec![file],
                    super::files::FileSelection::Package(pending) => pending
                        .resolve_with_prepared_build(
                            owner
                                .or(mounted)
                                .expect("a package import has an admitted graph"),
                            &storage.sources,
                            mount.and_then(|dependency| dependency.generated.as_ref()),
                        )?,
                    super::files::FileSelection::Unbound => continue,
                };
                for resolved in selected {
                    imports.push((resolved.path.clone(), authored.scope));
                    storage.resolved_imports.push(resolved);
                }
            }
        }
        Ok(imports)
    }

    pub(crate) fn validate_frontier(
        &self,
        frontier: Vec<(PathBuf, source_provenance::DependencyScope)>,
        storage: &SourceStorage,
    ) -> Result<Vec<(PathBuf, source_provenance::DependencyScope)>, Vec<Diagnostic>> {
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
                && !storage.is_installed_source(&canonical)
            {
                return Err(vec![Diagnostic::error(
                    "Product query source escapes its admitted source frontier",
                )]);
            }
            if self.graphs().any(|inputs| inputs.package_for_source(&canonical).is_some_and(|owner| {
                inputs.package_root(owner) != Some(storage.default_package_root())
                    && inputs.dependency_build_file(owner, &canonical)
            }))
            {
                return Err(vec![Diagnostic::error(format!(
                    "Product query cannot import dependency Build entry {}", canonical.display()
                ))]);
            }
            validated.push((canonical, scope));
        }
        Ok(validated)
    }
}
