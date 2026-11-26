// Dependency graph queries, cycle detection, and `PackageDependencyClosure`
// projection over `PackageCompilationInputs`. Durable closures project only
// product-scope edges reachable from a package root, while build-scope edges
// remain compilation-local nameability for the root's build entry.

use super::construction::is_snake_case;
use super::{PackageCompilationInputs, PackageCompilationSourceRecord, PackageDependencyBinding};
use crate::BuildDeclarationKind;
use build_declarations::DependencyPurpose;
use declaration_scope::PackageDeclarationScope;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyClosure {
    root: PackageKeyIdentity,
    root_role: BuildDeclarationKind,
    packages: Vec<PackageKeyIdentity>,
    dependencies: Vec<PackageDependencyBinding>,
}

impl PackageDependencyClosure {
    pub fn declaration_permissions(&self) -> PackageDeclarationScope {
        PackageDeclarationScope::from_direct_dependencies(
            &self.packages,
            self.dependencies
                .iter()
                .map(|edge| (edge.requester, edge.target)),
        )
    }

    pub const fn root(&self) -> PackageKeyIdentity {
        self.root
    }

    pub const fn root_role(&self) -> BuildDeclarationKind {
        self.root_role
    }

    pub fn packages(&self) -> &[PackageKeyIdentity] {
        &self.packages
    }

    pub fn dependencies(&self) -> &[PackageDependencyBinding] {
        &self.dependencies
    }

    #[doc(hidden)]
    pub fn from_canonical_parts(
        root: PackageKeyIdentity,
        root_role: BuildDeclarationKind,
        packages: Vec<PackageKeyIdentity>,
        dependencies: Vec<PackageDependencyBinding>,
    ) -> Result<Self, &'static str> {
        if root_role == BuildDeclarationKind::Workspace {
            return Err("package dependency closure root has workspace role");
        }
        if packages.is_empty() {
            return Err("package dependency closure has no packages");
        }
        if packages.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("package dependency closure packages are not in strict canonical order");
        }
        let package_set = packages.iter().copied().collect::<BTreeSet<_>>();
        if !package_set.contains(&root) {
            return Err("package dependency closure does not contain its root package");
        }

        let mut adjacency =
            BTreeMap::<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>::new();
        let mut prior_coordinate: Option<(PackageKeyIdentity, &str)> = None;
        for dependency in &dependencies {
            if dependency.purpose != DependencyPurpose::Product {
                return Err("package dependency closure retains only product-scope edges");
            }
            let coordinate = (dependency.requester, dependency.alias.as_str());
            if prior_coordinate.is_some_and(|prior| prior >= coordinate) {
                return Err("package dependency closure edges are not in strict canonical order");
            }
            prior_coordinate = Some(coordinate);
            if !is_snake_case(&dependency.alias) {
                return Err("package dependency closure contains a noncanonical alias");
            }
            if !package_set.contains(&dependency.requester)
                || !package_set.contains(&dependency.target)
            {
                return Err("package dependency closure contains an open edge");
            }
            adjacency
                .entry(dependency.requester)
                .or_default()
                .insert(dependency.alias.clone(), dependency.target);
        }

        if reachable_packages(root, |package| adjacency.get(&package)) != package_set {
            return Err("package dependency closure contains an unreachable package");
        }
        if dependency_cycle_in_set(&package_set, &adjacency) {
            return Err("package dependency closure contains a cycle");
        }

        Ok(Self {
            root,
            root_role,
            packages,
            dependencies,
        })
    }
}

impl PackageCompilationInputs {
    pub fn package_uses(
        &self,
        purpose: DependencyPurpose,
    ) -> impl Iterator<Item = PackageDependencyBinding> {
        self.source
            .packages
            .iter()
            .flat_map(move |(&requester, record)| {
                let edges = match purpose {
                    DependencyPurpose::Build => &record.build_dependencies,
                    DependencyPurpose::Product => &record.dependencies,
                };
                edges.iter().map(move |(alias, edge)| {
                    PackageDependencyBinding::for_purpose(requester, alias, edge.target, purpose)
                        .with_max_reach(edge.max_reach.clone())
                })
            })
    }

    /// Follow the edges this execution can use. A Build starts with its frozen
    /// Build imports, while every supplied library imports its own Product dependencies.
    pub fn execution_service_edges(
        &self,
        purpose: DependencyPurpose,
    ) -> Vec<semantic_vocabulary::dependency_reach::DependencyServiceEdge> {
        let mut edges = Vec::new();
        let mut pending = vec![self.root()];
        let mut visited = BTreeSet::new();
        while let Some(requester) = pending.pop() {
            if !visited.insert(requester) {
                continue;
            }
            let record = &self.source.packages[&requester];
            let dependencies = match self.declaration_selection_purpose(requester, purpose) {
                DependencyPurpose::Build => &record.build_dependencies,
                DependencyPurpose::Product => &record.dependencies,
            };
            for (alias, dependency) in dependencies {
                edges.push(
                    semantic_vocabulary::dependency_reach::DependencyServiceEdge {
                        requester: Some(requester),
                        package: dependency.target,
                        alias: alias.clone(),
                        max_reach: dependency.max_reach.clone(),
                    },
                );
                pending.push(dependency.target);
            }
        }
        edges
    }

    pub fn dependency_max_reach(
        &self,
        requester: PackageKeyIdentity,
        purpose: DependencyPurpose,
        alias: &str,
    ) -> Option<&semantic_vocabulary::dependency_reach::DependencyReach> {
        let record = self.source.packages.get(&requester)?;
        let edges = match purpose {
            DependencyPurpose::Build => &record.build_dependencies,
            DependencyPurpose::Product => &record.dependencies,
        };
        edges.get(alias).map(|edge| &edge.max_reach)
    }

    pub(crate) fn declaration_selection_purpose(
        &self,
        requester: PackageKeyIdentity,
        requested: DependencyPurpose,
    ) -> DependencyPurpose {
        use semantic_vocabulary::source_selections::SelectionPurpose;
        let purpose = match requested {
            DependencyPurpose::Build => SelectionPurpose::Build,
            DependencyPurpose::Product => SelectionPurpose::Product,
        }
        .for_declaring_package(Some(self.root()), requester);
        match purpose {
            SelectionPurpose::Build => DependencyPurpose::Build,
            SelectionPurpose::Product => DependencyPurpose::Product,
        }
    }

    /// A Build uses its own Build edges. Imported source always uses its supplier's Product edges.
    pub fn declaration_scope(&self, purpose: DependencyPurpose) -> PackageDeclarationScope {
        let packages = &self.source.packages;
        let edges = packages.iter().flat_map(|(&requester, record)| {
            let dependencies = match self.declaration_selection_purpose(requester, purpose) {
                DependencyPurpose::Build => &record.build_dependencies,
                DependencyPurpose::Product => &record.dependencies,
            };
            dependencies
                .values()
                .map(move |edge| (requester, edge.target))
        });
        PackageDeclarationScope::from_direct_dependencies(packages.keys(), edges)
            .with_package_names(
                packages
                    .iter()
                    .map(|(&id, record)| (id, record.canonical_name.clone()))
                    .collect(),
            )
    }
    pub fn direct_dependencies(
        &self,
        purpose: DependencyPurpose,
    ) -> impl Iterator<Item = (&str, PackageKeyIdentity)> {
        self.source
            .packages
            .get(&self.source.root)
            .into_iter()
            .flat_map(move |root| {
                let dependencies = match purpose {
                    DependencyPurpose::Product => &root.dependencies,
                    DependencyPurpose::Build => &root.build_dependencies,
                };
                dependencies
                    .iter()
                    .map(|(alias, target)| (alias.as_str(), target.target))
            })
    }

    pub fn dependencies(
        &self,
    ) -> impl Iterator<Item = (PackageKeyIdentity, &str, PackageKeyIdentity)> {
        self.source.packages.iter().flat_map(|(requester, record)| {
            record
                .dependencies
                .iter()
                .map(|(alias, target)| (*requester, alias.as_str(), target.target))
        })
    }

    pub(crate) fn build_dependencies(
        &self,
    ) -> impl Iterator<Item = (PackageKeyIdentity, &str, PackageKeyIdentity)> {
        self.source.packages.iter().flat_map(|(requester, record)| {
            record
                .build_dependencies
                .iter()
                .map(|(alias, target)| (*requester, alias.as_str(), target.target))
        })
    }

    pub fn dependency_closure(&self) -> PackageDependencyClosure {
        self.dependency_closure_for(self.source.root)
    }

    pub(super) fn product_dependencies_of(
        &self,
        root: PackageKeyIdentity,
    ) -> BTreeSet<PackageKeyIdentity> {
        reachable_packages(root, |package| {
            self.source
                .packages
                .get(&package)
                .map(|record| &record.dependencies)
        })
    }

    #[doc(hidden)]
    pub(crate) fn dependency_closure_for(
        &self,
        root: PackageKeyIdentity,
    ) -> PackageDependencyClosure {
        let reachable = self.product_dependencies_of(root);
        PackageDependencyClosure {
            root,
            root_role: if root == self.source.root {
                self.source.root_role
            } else {
                BuildDeclarationKind::Package
            },
            packages: self
                .source
                .packages
                .keys()
                .copied()
                .filter(|package| reachable.contains(package))
                .collect(),
            dependencies: self
                .package_uses(DependencyPurpose::Product)
                .filter(|edge| {
                    reachable.contains(&edge.requester) && reachable.contains(&edge.target)
                })
                .collect(),
        }
    }

    #[doc(hidden)]
    pub fn dependency_target_for_purpose(
        &self,
        requester: PackageKeyIdentity,
        purpose: DependencyPurpose,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        let record = self.source.packages.get(&requester)?;
        match purpose {
            DependencyPurpose::Product => record.dependencies.get(alias),
            DependencyPurpose::Build => record.build_dependencies.get(alias),
        }
        .map(|edge| edge.target)
    }
}

fn reachable_packages<'inputs, T: std::borrow::Borrow<PackageKeyIdentity> + 'inputs>(
    root: PackageKeyIdentity,
    dependencies: impl Fn(PackageKeyIdentity) -> Option<&'inputs BTreeMap<String, T>>,
) -> BTreeSet<PackageKeyIdentity> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        if let Some(targets) = dependencies(identity) {
            pending.extend(targets.values().map(|edge| *edge.borrow()));
        }
    }
    reachable
}

pub(super) fn dependency_cycle(
    packages: &BTreeMap<PackageKeyIdentity, PackageCompilationSourceRecord>,
) -> Option<Vec<PackageKeyIdentity>> {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Active,
        Complete,
    }

    fn visit(
        identity: PackageKeyIdentity,
        packages: &BTreeMap<PackageKeyIdentity, PackageCompilationSourceRecord>,
        states: &mut BTreeMap<PackageKeyIdentity, Visit>,
        stack: &mut Vec<PackageKeyIdentity>,
    ) -> Option<Vec<PackageKeyIdentity>> {
        if states.get(&identity) == Some(&Visit::Complete) {
            return None;
        }
        if states.get(&identity) == Some(&Visit::Active) {
            let start = stack.iter().position(|entry| *entry == identity)?;
            let mut cycle = stack[start..].to_vec();
            cycle.push(identity);
            return Some(cycle);
        }

        states.insert(identity, Visit::Active);
        stack.push(identity);
        if let Some(record) = packages.get(&identity) {
            for target in record
                .dependencies
                .values()
                .chain(record.build_dependencies.values())
                .map(|edge| edge.target)
            {
                if let Some(cycle) = visit(target, packages, states, stack) {
                    return Some(cycle);
                }
            }
        }
        stack.pop();
        states.insert(identity, Visit::Complete);
        None
    }

    let mut states = BTreeMap::new();
    let mut stack = Vec::new();
    for identity in packages.keys().copied() {
        if let Some(cycle) = visit(identity, packages, &mut states, &mut stack) {
            return Some(cycle);
        }
    }
    None
}

fn dependency_cycle_in_set(
    packages: &BTreeSet<PackageKeyIdentity>,
    dependencies: &BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
) -> bool {
    let mut inbound = packages
        .iter()
        .copied()
        .map(|package| (package, 0usize))
        .collect::<BTreeMap<_, _>>();
    for targets in dependencies.values() {
        for target in targets.values() {
            let Some(count) = inbound.get_mut(target) else {
                return true;
            };
            let Some(next) = count.checked_add(1) else {
                return true;
            };
            *count = next;
        }
    }

    let mut ready = inbound
        .iter()
        .filter_map(|(package, count)| (*count == 0).then_some(*package))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some(package) = ready.pop() {
        visited += 1;
        if let Some(targets) = dependencies.get(&package) {
            for target in targets.values() {
                let count = inbound
                    .get_mut(target)
                    .expect("closed package dependency edge retains its target");
                *count -= 1;
                if *count == 0 {
                    ready.push(*target);
                }
            }
        }
    }
    visited != packages.len()
}

#[cfg(test)]
mod selection_authority_tests {
    use super::*;
    use crate::PackageSourceBinding;

    #[test]
    fn sealed_product_authority_retains_direct_edges_only() {
        let identity = |byte| PackageKeyIdentity::from_digest([byte; 32]).unwrap();
        let [root, dependency, transitive, absent] = [1, 2, 3, 4].map(identity);
        let graph = PackageDependencyClosure::from_canonical_parts(
            root,
            BuildDeclarationKind::Application,
            vec![root, dependency, transitive],
            vec![
                PackageDependencyBinding::for_purpose(
                    root,
                    "dependency",
                    dependency,
                    DependencyPurpose::Product,
                ),
                PackageDependencyBinding::for_purpose(
                    dependency,
                    "transitive",
                    transitive,
                    DependencyPurpose::Product,
                ),
            ],
        )
        .expect("canonical Product closure");
        let captured = graph.declaration_permissions();
        assert!(captured.allows_declaration_selection(root, root));
        assert!(captured.allows_declaration_selection(root, dependency));
        assert!(captured.allows_declaration_selection(dependency, transitive));
        assert!(!captured.allows_declaration_selection(root, transitive));
        assert!(!captured.allows_declaration_selection(absent, absent));
        assert!(!captured.allows_declaration_selection(root, absent));
    }

    #[test]
    fn source_purposes_keep_root_build_and_dependency_product_edges_separate() {
        let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
            "purpose-declaration-authority",
            false,
        )
        .expect("temporary package roots");
        let identity = |byte| PackageKeyIdentity::from_digest([byte; 32]).unwrap();
        let root = identity(1);
        let helper = identity(2);
        let product = identity(3);
        let helper_dependency = identity(4);
        let packages = [
            (root, "root"),
            (helper, "helper"),
            (product, "product"),
            (helper_dependency, "helper_dependency"),
        ]
        .into_iter()
        .map(|(owner, name)| {
            let path = directory.path().join(name);
            std::fs::create_dir(&path).expect("package source root");
            PackageSourceBinding::new(owner, name, path)
        })
        .collect();
        let inputs = PackageCompilationInputs::new(
            root,
            BuildDeclarationKind::Application,
            packages,
            vec![
                PackageDependencyBinding::for_purpose(
                    root,
                    "service",
                    helper,
                    DependencyPurpose::Build,
                ),
                PackageDependencyBinding::for_purpose(
                    root,
                    "service",
                    product,
                    DependencyPurpose::Product,
                ),
                PackageDependencyBinding::for_purpose(
                    helper,
                    "helper_dependency",
                    helper_dependency,
                    DependencyPurpose::Product,
                ),
            ],
        )
        .expect("closed purpose-specific package graph");
        let build = inputs.declaration_scope(DependencyPurpose::Build);
        let selected_product = inputs.declaration_scope(DependencyPurpose::Product);
        assert_eq!(
            inputs.dependency_target_for_purpose(root, DependencyPurpose::Build, "service"),
            Some(helper)
        );
        assert_eq!(
            inputs.dependency_target_for_purpose(root, DependencyPurpose::Product, "service"),
            Some(product)
        );
        assert!(build.allows_declaration_selection(root, helper));
        assert!(!build.allows_declaration_selection(root, product));
        assert!(selected_product.allows_declaration_selection(root, product));
        assert!(!selected_product.allows_declaration_selection(root, helper));
        assert!(build.allows_declaration_selection(helper, helper_dependency));
        assert!(!build.allows_declaration_selection(helper, product));
        assert!(!build.allows_declaration_selection(identity(5), identity(5)));
    }

    #[test]
    fn component_selections_follow_the_supplying_products_own_dependencies() {
        let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
            "component-supplier-graph",
            false,
        )
        .unwrap();
        let [root, component, leaf, sibling, helper] =
            [1, 2, 3, 4, 5].map(|byte| PackageKeyIdentity::from_digest([byte; 32]).unwrap());
        let packages = [
            (root, "root"),
            (component, "component"),
            (leaf, "leaf"),
            (sibling, "sibling"),
            (helper, "helper"),
        ]
        .map(|(identity, name)| {
            let path = directory.path().join(name);
            std::fs::create_dir(&path).unwrap();
            PackageSourceBinding::new(identity, name, path)
        });
        let edges = [
            (root, "component", component, DependencyPurpose::Product),
            (root, "sibling", sibling, DependencyPurpose::Product),
            (component, "leaf", leaf, DependencyPurpose::Product),
            (root, "helper", helper, DependencyPurpose::Build),
        ]
        .map(|(from, name, to, purpose)| {
            PackageDependencyBinding::for_purpose(from, name, to, purpose)
        });
        let inputs = PackageCompilationInputs::new(
            root,
            BuildDeclarationKind::Application,
            packages.into(),
            edges.into(),
        )
        .unwrap();
        let description = |supplier| {
            crate::IndependentComponentDescription::new(
                component,
                terminal_psi::TerminalPsiIdentity {
                    vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                    program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([7; 32]),
                },
                [supplier].into(),
                Vec::new(),
            )
        };
        assert!(
            inputs
                .clone()
                .with_independent_component_descriptions(vec![description(leaf)])
                .is_ok()
        );
        for supplier in [component, sibling, helper, root] {
            assert_eq!(
                inputs
                    .clone()
                    .with_independent_component_descriptions(vec![description(supplier)]),
                Err(vec![
                    crate::PackageCompilationInputError::ForeignComponentSupplier {
                        package: component,
                        supplier,
                    }
                ])
            );
        }
    }
}
