//! Validated package-closure model over immutable source identities.

use omega_package_source::{AliasName, IdentityError, ImmutableSourceResolution, PackageKey};
use std::collections::{BTreeMap, BTreeSet};

/// A package's stable identity paired with one immutable source selection.
///
/// This is source-resolution output only. It does not represent compiler
/// evidence, capability admission, or an accepted package instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSourceIdentity {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
}

impl ResolvedSourceIdentity {
    pub fn new(
        key: PackageKey,
        resolution: ImmutableSourceResolution,
    ) -> Result<Self, IdentityError> {
        if !resolution.matches_lineage(key.source_lineage()) {
            return Err(IdentityError::ResolutionLineageMismatch);
        }

        Ok(Self { key, resolution })
    }

    pub(crate) fn from_validated_parts(
        key: PackageKey,
        resolution: ImmutableSourceResolution,
    ) -> Self {
        debug_assert!(resolution.matches_lineage(key.source_lineage()));
        Self { key, resolution }
    }

    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }
}

/// One requester-local import alias and its exact package target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDependency {
    alias: AliasName,
    target: PackageKey,
}

impl ResolvedDependency {
    pub fn new(alias: AliasName, target: PackageKey) -> Self {
        Self { alias, target }
    }

    pub fn alias(&self) -> &AliasName {
        &self.alias
    }

    pub fn target(&self) -> &PackageKey {
        &self.target
    }
}

/// One resolved package and the dependency edges authored by that package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackageNode {
    source: ResolvedSourceIdentity,
    dependencies: Vec<ResolvedDependency>,
}

impl ResolvedPackageNode {
    pub fn new(source: ResolvedSourceIdentity, dependencies: Vec<ResolvedDependency>) -> Self {
        Self {
            source,
            dependencies,
        }
    }

    pub fn source(&self) -> &ResolvedSourceIdentity {
        &self.source
    }

    pub fn dependencies(&self) -> &[ResolvedDependency] {
        &self.dependencies
    }
}

/// A closed, finite source graph before compilation or admission.
///
/// Construction validates the complete graph. In particular, every edge is
/// resolved by exact `PackageKey`; a package with the same declared name but a
/// different source lineage cannot satisfy the edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackageClosure {
    root: PackageKey,
    packages: Vec<ResolvedPackageNode>,
    package_indices: BTreeMap<PackageKey, usize>,
}

impl ResolvedPackageClosure {
    pub fn new(
        root: PackageKey,
        packages: Vec<ResolvedPackageNode>,
    ) -> Result<Self, Vec<PackageClosureValidationError>> {
        let mut errors = Vec::new();
        let mut package_indices: BTreeMap<PackageKey, usize> = BTreeMap::new();

        for (index, package) in packages.iter().enumerate() {
            let key = package.source.key();
            if let Some(previous_index) = package_indices.get(key).copied() {
                let previous = &packages[previous_index];
                if previous.source.resolution() == package.source.resolution() {
                    errors
                        .push(PackageClosureValidationError::DuplicatePackage { key: key.clone() });
                } else {
                    errors.push(PackageClosureValidationError::ConflictingResolution {
                        key: key.clone(),
                        first: previous.source.resolution().clone(),
                        conflicting: package.source.resolution().clone(),
                    });
                }
            } else {
                package_indices.insert(key.clone(), index);
            }

            let mut aliases = BTreeSet::new();
            for dependency in package.dependencies() {
                if !aliases.insert(dependency.alias().clone()) {
                    errors.push(PackageClosureValidationError::DuplicateAlias {
                        requester: key.clone(),
                        alias: dependency.alias().clone(),
                    });
                }
            }
        }

        if !package_indices.contains_key(&root) {
            errors.push(PackageClosureValidationError::MissingRoot { root: root.clone() });
        }

        for package in &packages {
            for dependency in package.dependencies() {
                if !package_indices.contains_key(dependency.target()) {
                    errors.push(PackageClosureValidationError::MissingDependencyTarget {
                        requester: package.source.key().clone(),
                        alias: dependency.alias().clone(),
                        target: dependency.target().clone(),
                    });
                }
            }
        }

        if package_indices.contains_key(&root) {
            let reachable = reachable_packages(&root, &packages, &package_indices);
            for key in package_indices.keys() {
                if !reachable.contains(key) {
                    errors.push(PackageClosureValidationError::UnreachablePackage {
                        key: key.clone(),
                    });
                }
            }
        }

        if let Some(cycle) = find_dependency_cycle(&packages, &package_indices) {
            errors.push(PackageClosureValidationError::DependencyCycle { cycle });
        }

        if errors.is_empty() {
            Ok(Self {
                root,
                packages,
                package_indices,
            })
        } else {
            Err(errors)
        }
    }

    pub fn root(&self) -> &PackageKey {
        &self.root
    }

    pub fn packages(&self) -> &[ResolvedPackageNode] {
        &self.packages
    }

    pub fn package(&self, key: &PackageKey) -> Option<&ResolvedPackageNode> {
        self.package_indices
            .get(key)
            .map(|index| &self.packages[*index])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageClosureValidationError {
    MissingRoot {
        root: PackageKey,
    },
    DuplicatePackage {
        key: PackageKey,
    },
    ConflictingResolution {
        key: PackageKey,
        first: ImmutableSourceResolution,
        conflicting: ImmutableSourceResolution,
    },
    DuplicateAlias {
        requester: PackageKey,
        alias: AliasName,
    },
    MissingDependencyTarget {
        requester: PackageKey,
        alias: AliasName,
        target: PackageKey,
    },
    UnreachablePackage {
        key: PackageKey,
    },
    DependencyCycle {
        /// The first and last entries are the same package.
        cycle: Vec<PackageKey>,
    },
}

fn reachable_packages(
    root: &PackageKey,
    packages: &[ResolvedPackageNode],
    package_indices: &BTreeMap<PackageKey, usize>,
) -> BTreeSet<PackageKey> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root.clone()];

    while let Some(key) = pending.pop() {
        if !reachable.insert(key.clone()) {
            continue;
        }
        let Some(index) = package_indices.get(&key) else {
            continue;
        };
        for dependency in packages[*index].dependencies().iter().rev() {
            if package_indices.contains_key(dependency.target()) {
                pending.push(dependency.target().clone());
            }
        }
    }

    reachable
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

fn find_dependency_cycle(
    packages: &[ResolvedPackageNode],
    package_indices: &BTreeMap<PackageKey, usize>,
) -> Option<Vec<PackageKey>> {
    let mut states = BTreeMap::new();
    let mut stack = Vec::new();

    for key in package_indices.keys() {
        if !states.contains_key(key)
            && let Some(cycle) =
                visit_for_cycle(key, packages, package_indices, &mut states, &mut stack)
        {
            return Some(cycle);
        }
    }

    None
}

fn visit_for_cycle(
    key: &PackageKey,
    packages: &[ResolvedPackageNode],
    package_indices: &BTreeMap<PackageKey, usize>,
    states: &mut BTreeMap<PackageKey, VisitState>,
    stack: &mut Vec<PackageKey>,
) -> Option<Vec<PackageKey>> {
    states.insert(key.clone(), VisitState::Visiting);
    stack.push(key.clone());

    let index = package_indices[key];
    for dependency in packages[index].dependencies() {
        let target = dependency.target();
        if !package_indices.contains_key(target) {
            continue;
        }

        match states.get(target) {
            Some(VisitState::Visiting) => {
                let cycle_start = stack
                    .iter()
                    .position(|candidate| candidate == target)
                    .expect("a visiting package is present in the DFS stack");
                let mut cycle = stack[cycle_start..].to_vec();
                cycle.push(target.clone());
                return Some(cycle);
            }
            Some(VisitState::Visited) => {}
            None => {
                if let Some(cycle) =
                    visit_for_cycle(target, packages, package_indices, states, stack)
                {
                    return Some(cycle);
                }
            }
        }
    }

    let removed = stack.pop();
    debug_assert_eq!(removed.as_ref(), Some(key));
    states.insert(key.clone(), VisitState::Visited);
    None
}
