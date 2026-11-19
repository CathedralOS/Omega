/*
Request-local package compilation scope (`PackageCompilationScope`) and source-binding assembly.

`PackageCompilationScope` re-roots an immutable `ResolvedPackageSourceClosure` at one
package key. `Product` dependency edges reach from every package in the scope, whereas
`Build` dependency edges reach only from the compilation root (`root`), because a
dependency's own build file resolves in its own compilation where that dependency is
the root.
*/

/*
`compilation_inputs_reusing` captures the root's canonical build-source metadata once
per `(identity, source_root)` pair and reuses it across passes and purposes for that
root, consulting the self-verifying `CheckedSourceCache` when a persisted source
supplies a checked-source cache directory.
*/

use crate::resolution::graph::ResolvedPackageSourceClosure;
use package_compilation::{
    PackageCompilationInputError, PackageCompilationInputs, PackageDependencyAcquisition,
    PackageDependencyBinding, PackageSourceBinding,
};
use std::collections::BTreeSet;

pub(crate) struct PackageCompilationScope<'closure> {
    closure: &'closure ResolvedPackageSourceClosure,
    root: &'closure crate::declarations::PackageKey,
    reachable: BTreeSet<crate::declarations::PackageKey>,
}

impl<'closure> PackageCompilationScope<'closure> {
    pub(crate) fn new(
        closure: &'closure ResolvedPackageSourceClosure,
        root: &'closure crate::declarations::PackageKey,
    ) -> Self {
        Self {
            closure,
            root,
            reachable: reachable_package_keys(closure, root),
        }
    }

    pub(crate) fn closure(&self) -> &ResolvedPackageSourceClosure {
        self.closure
    }

    pub(crate) fn root(&self) -> &crate::declarations::PackageKey {
        self.root
    }

    pub(crate) fn packages(&self) -> &BTreeSet<crate::declarations::PackageKey> {
        &self.reachable
    }

    pub(crate) fn compilation_inputs_reusing(
        &self,
        root_binding: &mut Option<PackageSourceBinding>,
    ) -> Result<PackageCompilationInputs, Vec<PackageCompilationInputError>> {
        let closure = self.closure;
        let root = self.root;
        let reachable = &self.reachable;
        let packages = closure
            .custodies()
            .iter()
            .filter(|custody| reachable.contains(custody.key()))
            .map(|custody| {
                let binding = PackageSourceBinding::new(
                    custody.key().identity(),
                    custody.key().name().as_str(),
                    custody.snapshot_root().to_path_buf(),
                );
                if custody.key() == root {
                    if let Some(captured) = root_binding.as_ref().filter(|captured| {
                        captured.identity() == binding.identity()
                            && captured.source_root() == binding.source_root()
                    }) {
                        return Ok(captured.clone());
                    }
                    let captured = binding_with_canonical_source_metadata(custody, binding)?;
                    *root_binding = Some(captured.clone());
                    Ok(captured)
                } else {
                    if let Some(tree) = custody.captured_source() {
                        binding
                            .with_captured_source(std::sync::Arc::clone(tree))
                            .map_err(|reason| PackageCompilationInputError::InvalidSourceRoot {
                                identity: custody.key().identity(),
                                path: custody.snapshot_root().to_path_buf(),
                                reason,
                            })
                    } else {
                        Ok(binding)
                    }
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| vec![error])?;
        let dependencies = closure
            .graph()
            .packages()
            .iter()
            .filter(|package| reachable.contains(package.source().key()))
            .flat_map(|package| {
                let requester = package.source().key().identity();
                package
                    .dependencies()
                    .iter()
                    .filter(move |dependency| {
                        dependency.purpose().is_product() || requester == root.identity()
                    })
                    .map(move |dependency| {
                        PackageDependencyBinding::for_purpose(
                            requester,
                            dependency.alias().as_str(),
                            dependency.target().identity(),
                            dependency.purpose(),
                        )
                    })
            })
            .collect();

        let root_role = if root == closure.graph().root() {
            closure.root_role()
        } else {
            crate::declarations::BuildDeclarationKind::Package
        };
        let invalid = |reason: String| {
            vec![PackageCompilationInputError::InvalidDependencyAcquisition {
                root: root.identity(),
                reason,
            }]
        };
        let custody = closure
            .custody(root)
            .ok_or_else(|| invalid("root source custody is missing".into()))?;
        let node = closure
            .graph()
            .package(root)
            .ok_or_else(|| invalid("root source node is missing".into()))?;
        let mut acquisitions = Vec::with_capacity(node.dependencies().len());
        for dependency in node.dependencies() {
            let request = custody
                .dependency_projections()
                .requests(dependency.purpose())
                .get(dependency.dependency_index())
                .ok_or_else(|| {
                    invalid("selected source has no originating dependency request".into())
                })?;
            acquisitions.push(PackageDependencyAcquisition {
                request_ordinal: dependency.dependency_index(),
                request: request
                    .build_request(dependency.purpose())
                    .map_err(invalid)?,
                package: dependency.target().identity(),
            });
        }
        PackageCompilationInputs::new(root.identity(), root_role, packages, dependencies)?
            .with_dependency_acquisitions(acquisitions)
    }
}

fn binding_with_canonical_source_metadata(
    custody: &crate::resolution::source::PackageSourceCustody,
    binding: PackageSourceBinding,
) -> Result<PackageSourceBinding, PackageCompilationInputError> {
    let invalid = |reason: String| PackageCompilationInputError::InvalidSourceRoot {
        identity: custody.key().identity(),
        path: custody.snapshot_root().to_path_buf(),
        reason: format!("invalid canonical build-source metadata: {reason}"),
    };
    if let Some(tree) = custody.captured_source() {
        return binding
            .with_captured_source(std::sync::Arc::clone(tree))
            .map_err(invalid);
    }
    if let Some(directory) = custody.checked_source_cache_dir()
        && let Ok(cache) = package_compilation::CheckedSourceCache::open_or_create(directory)
    {
        return binding
            .with_cached_canonical_source_metadata(&cache)
            .map_err(invalid);
    }
    binding.with_canonical_source_metadata().map_err(invalid)
}

fn reachable_package_keys(
    closure: &ResolvedPackageSourceClosure,
    root: &crate::declarations::PackageKey,
) -> BTreeSet<crate::declarations::PackageKey> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root.clone()];
    while let Some(package) = pending.pop() {
        if !reachable.insert(package.clone()) {
            continue;
        }
        let Some(node) = closure.graph().package(&package) else {
            continue;
        };
        let is_root = &package == root;
        pending.extend(
            node.dependencies()
                .iter()
                .rev()
                .filter(|dependency| dependency.purpose().is_product() || is_root)
                .map(|dependency| dependency.target().clone()),
        );
    }
    reachable
}
