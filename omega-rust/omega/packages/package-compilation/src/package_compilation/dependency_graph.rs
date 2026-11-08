//! Questions answered over the reconciled dependency graph: exact edge
//! occurrences, scoped alias targets, declaration-selection authority, and
//! the durable product-scope closure, plus the reachability and cycle
//! walks shared by graph construction and closure recovery.

use super::construction::is_snake_case;
use super::{PackageCompilationInputs, PackageCompilationSourceRecord, PackageDependencyBinding};
use crate::BuildDeclarationKind;
use build_declarations::DependencyPurpose;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};

/// Exact, source-path-free dependency closure consumed by one package-aware
/// compilation. This is a semantic subject coordinate, not source custody or
/// an admission verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyClosure {
    root: PackageKeyIdentity,
    root_role: BuildDeclarationKind,
    packages: Vec<PackageKeyIdentity>,
    dependencies: Vec<PackageDependencyBinding>,
}

impl PackageDependencyClosure {
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

    /// Recover one canonical source-path-free closure from persisted semantic
    /// coordinates. This does not recover source custody or package admission.
    ///
    /// The wire decoder must not be able to manufacture a weaker graph shape
    /// than `PackageCompilationInputs`: packages and requester-local aliases
    /// are strictly ordered, every edge is closed, the root reaches every
    /// package, and cycles reject.
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
    /// Direct aliases in one root execution scope. Build aliases never fall
    /// back to Product aliases, including during the final request join.
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
                    .map(|(alias, target)| (alias.as_str(), *target))
            })
    }

    /// Product-scope edges only. Build-purpose edges are compilation-local
    /// nameability for the root's build entry and never join the durable
    /// closure projections produced from this iterator.
    pub fn dependencies(
        &self,
    ) -> impl Iterator<Item = (PackageKeyIdentity, &str, PackageKeyIdentity)> {
        self.source.packages.iter().flat_map(|(requester, record)| {
            record
                .dependencies
                .iter()
                .map(|(alias, target)| (*requester, alias.as_str(), *target))
        })
    }

    /// The root's build-scope edges. Only the root package may hold them.
    pub(crate) fn build_dependencies(
        &self,
    ) -> impl Iterator<Item = (PackageKeyIdentity, &str, PackageKeyIdentity)> {
        self.source.packages.iter().flat_map(|(requester, record)| {
            record
                .build_dependencies
                .iter()
                .map(|(alias, target)| (*requester, alias.as_str(), *target))
        })
    }

    /// Project the exact validated graph without source paths, package display
    /// names, immutable source resolutions, or source bytes.
    ///
    /// The durable closure is the product scope: packages reachable from the
    /// root through product edges plus the product edges between them.
    /// Build-scope packages retain compilation-local nameability through
    /// [`Self::dependency_target_for_purpose`] but never join this projection —
    /// host build inputs are not product review subjects.
    pub fn dependency_closure(&self) -> PackageDependencyClosure {
        self.dependency_closure_for(self.source.root)
    }

    /// The product-scope closure of one package inside this validated graph:
    /// its product-reachable packages and the product edges between them.
    /// Generated-source bundle custody compares against exactly this shape.
    #[doc(hidden)]
    pub(crate) fn dependency_closure_for(
        &self,
        root: PackageKeyIdentity,
    ) -> PackageDependencyClosure {
        let reachable = reachable_packages(root, |package| {
            self.source
                .packages
                .get(&package)
                .map(|record| &record.dependencies)
        });
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
                .dependencies()
                .filter(|(requester, _, target)| {
                    reachable.contains(requester) && reachable.contains(target)
                })
                .map(|(requester, alias, target)| {
                    PackageDependencyBinding::new(requester, alias, target)
                })
                .collect(),
        }
    }
    /// The target of `requester`'s `alias` in exactly one dependency scope.
    /// Scopes never fall back to each other: a miss here does not license
    /// probing the other scope.
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
        .copied()
    }

    /// Whether `requester` directly declared `owner` as a dependency in either
    /// scope. Which source files may *name* which scope is decided earlier at
    /// import binding; this package-level check confirms only that a declared
    /// direct edge exists for the selected declaration's owner.
    #[doc(hidden)]
    pub fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        requester == owner
            || self.source.packages.get(&requester).is_some_and(|record| {
                record.dependencies.values().any(|target| *target == owner)
                    || record
                        .build_dependencies
                        .values()
                        .any(|target| *target == owner)
            })
    }
}

fn reachable_packages<'inputs>(
    root: PackageKeyIdentity,
    dependencies: impl Fn(PackageKeyIdentity) -> Option<&'inputs BTreeMap<String, PackageKeyIdentity>>,
) -> BTreeSet<PackageKeyIdentity> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        if let Some(targets) = dependencies(identity) {
            pending.extend(targets.values().copied());
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
                .copied()
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
