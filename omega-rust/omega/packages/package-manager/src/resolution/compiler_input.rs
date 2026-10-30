//! Revalidated package-aware inputs handed from source custody to the compiler.

use crate::resolution::graph::ResolvedPackageSourceClosure;
use package_compilation::{
    PackageCompilationInputError, PackageCompilationInputs, PackageDependencyAcquisition,
    PackageDependencyBinding, PackageSourceBinding,
};
use std::collections::BTreeSet;

/// One request-local re-rooting of immutable package topology. Reusing the
/// selection does not retain a filesystem-verification verdict: each consumer
/// still performs its own required source and compiler-input checks.
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

    /// Build the inputs, capturing the root's canonical build-source index
    /// only when `root_binding` holds none yet. The snapshot root is immutable
    /// for the invocation, so every pass and purpose of the same package root
    /// reads one capture.
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
                    // A same-sized but differently resolved closure keeps
                    // its stale slot out by identity and snapshot root.
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
        // Product edges authorize product imports for every package in scope.
        // Build edges authorize imports only from the compilation root's build
        // entry: a dependency's own build context resolves in its own
        // compilation, where that package is the root, so its build edges are
        // never projected into a consumer's inputs.
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
    // Invocation custody supplies its already-captured bytes and namespace.
    // A persisted source instead supplies sealed physical backing; its index
    // may use the checked-source cache. A live checkout stat fingerprint is
    // never a substitute for the content retained by this invocation.
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
    // A resolved custody may name a retained lane directory holding the
    // persistent checked-source index. Reuse is self-verifying — a retained
    // record is only replayed when its stat fingerprint still matches — so a
    // directory that cannot be opened degrades to the cold capture rather
    // than failing compilation.
    if let Some(directory) = custody.checked_source_cache_dir()
        && let Ok(cache) = package_compilation::CheckedSourceCache::open_or_create(directory)
    {
        return binding
            .with_cached_canonical_source_metadata(&cache)
            .map_err(invalid);
    }
    binding.with_canonical_source_metadata().map_err(invalid)
}

/// The compilation closure of one package root.
///
/// Product edges reach from every package. The root's build-purpose edges also
/// reach: its build entry may import build-scope snapshots, so those packages —
/// and their own product closures — are compilation inputs. A non-root
/// package's build edges never reach into a consumer's closure: dependency
/// build files do not join the compiled program, and the dependency's own
/// build context resolves when it is the root of its own compilation.
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
