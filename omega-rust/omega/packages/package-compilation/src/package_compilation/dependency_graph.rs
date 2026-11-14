// Dependency graph queries, cycle detection, and `PackageDependencyClosure`
// projection over `PackageCompilationInputs`. Durable closures project only
// product-scope edges reachable from a package root, while build-scope edges
// remain compilation-local nameability for the root's build entry.

use super::construction::is_snake_case;
use super::{PackageCompilationInputs, PackageCompilationSourceRecord, PackageDependencyBinding};
use crate::BuildDeclarationKind;
use build_declarations::DependencyPurpose;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};

/// A package's own Build reads its Build dependencies. Source supplied by a
/// dependency keeps that dependency's Product dependencies, even when the
/// consumer is running Build. An inert Product selection requests Product
/// nameability explicitly; it does not make its target executable in Build.
struct PurposeDeclarationAuthority<'a> {
    inputs: &'a PackageCompilationInputs,
    purpose: DependencyPurpose,
}

impl PurposeDeclarationAuthority<'_> {
    fn dependencies(
        &self,
        requester: PackageKeyIdentity,
    ) -> Option<&BTreeMap<String, PackageKeyIdentity>> {
        let record = self.inputs.source.packages.get(&requester)?;
        Some(
            match self
                .inputs
                .declaration_selection_purpose(requester, self.purpose)
            {
                DependencyPurpose::Build => &record.build_dependencies,
                DependencyPurpose::Product => &record.dependencies,
            },
        )
    }
}

impl semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority
    for PurposeDeclarationAuthority<'_>
{
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        self.inputs.source.packages.contains_key(&owner)
            && self.dependencies(requester).is_some_and(|dependencies| {
                requester == owner || dependencies.values().any(|target| *target == owner)
            })
    }

    fn package_label(&self, identity: PackageKeyIdentity) -> String {
        self.inputs.package_label(identity)
    }

    fn declaration_selection_scope(
        &self,
    ) -> Option<BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>> {
        Some(
            self.inputs
                .source
                .packages
                .iter()
                .map(|(requester, record)| {
                    let mut owners = vec![*requester];
                    let dependencies = match self
                        .inputs
                        .declaration_selection_purpose(*requester, self.purpose)
                    {
                        DependencyPurpose::Build => &record.build_dependencies,
                        DependencyPurpose::Product => &record.dependencies,
                    };
                    owners.extend(dependencies.values().copied());
                    owners.sort_unstable();
                    owners.dedup();
                    (*requester, owners)
                })
                .collect(),
        )
    }
}

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
    pub(crate) fn declaration_selection_purpose(
        &self,
        requester: PackageKeyIdentity,
        requested: DependencyPurpose,
    ) -> DependencyPurpose {
        if requester == self.root() {
            requested
        } else {
            DependencyPurpose::Product
        }
    }

    /// The exact direct package selections available for one source purpose.
    /// This view never combines Build and Product dependency edges.
    pub fn declaration_selection_authority(
        &self,
        purpose: DependencyPurpose,
    ) -> impl semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority + '_ {
        PurposeDeclarationAuthority {
            inputs: self,
            purpose,
        }
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
                    .map(|(alias, target)| (alias.as_str(), *target))
            })
    }

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

    pub fn dependency_closure(&self) -> PackageDependencyClosure {
        self.dependency_closure_for(self.source.root)
    }

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

#[cfg(test)]
mod selection_authority_tests {
    use super::*;
    use crate::PackageSourceBinding;
    use semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority;

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
                    "helper",
                    helper,
                    DependencyPurpose::Build,
                ),
                PackageDependencyBinding::for_purpose(
                    root,
                    "product",
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
        let build = inputs.declaration_selection_authority(DependencyPurpose::Build);
        let selected_product = inputs.declaration_selection_authority(DependencyPurpose::Product);
        assert!(build.allows_declaration_selection(root, helper));
        assert!(!build.allows_declaration_selection(root, product));
        assert!(selected_product.allows_declaration_selection(root, product));
        assert!(!selected_product.allows_declaration_selection(root, helper));
        assert!(build.allows_declaration_selection(helper, helper_dependency));
        assert!(!build.allows_declaration_selection(helper, product));
        assert!(!build.allows_declaration_selection(identity(5), identity(5)));
        let recorded = build.declaration_selection_scope().unwrap();
        assert_eq!(recorded[&root], vec![root, helper]);
        assert_eq!(recorded[&helper], vec![helper, helper_dependency]);
    }
}
