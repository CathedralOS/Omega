//! Revalidated package-aware inputs handed from source custody to the compiler.

use crate::resolution::graph::ResolvedPackageSourceClosure;
use package_compilation::{
    PackageCompilationInputError, PackageCompilationInputs, PackageDependencyBinding,
    PackageSourceBinding,
};
use std::collections::BTreeSet;

/// Translate resolver-owned closure custody into the compiler's independently
/// validated package-aware input graph.
///
/// Package identities come from `PackageKey`; snapshot paths remain routing
/// custody only. `PackageCompilationInputs::new` canonicalizes and rechecks the
/// complete graph rather than trusting the package-side structural verdict.
pub fn package_compilation_inputs(
    closure: &ResolvedPackageSourceClosure,
) -> Result<PackageCompilationInputs, Vec<PackageCompilationInputError>> {
    package_compilation_inputs_for(closure, closure.graph().root())
}

/// Build the independently validated compiler graph for one package and only
/// its transitive dependencies inside an already closed source custody graph.
///
/// Re-rooting is required when every dependency is compiled for its own review:
/// passing unrelated sibling packages would correctly fail the compiler's
/// unreachable-package check.
pub fn package_compilation_inputs_for(
    closure: &ResolvedPackageSourceClosure,
    root: &crate::declarations::PackageKey,
) -> Result<PackageCompilationInputs, Vec<PackageCompilationInputError>> {
    PackageCompilationScope::new(closure, root).compilation_inputs()
}

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

    pub(crate) fn compilation_inputs(
        &self,
    ) -> Result<PackageCompilationInputs, Vec<PackageCompilationInputError>> {
        let closure = self.closure;
        let root = self.root;
        let reachable = &self.reachable;
        let packages = closure
            .custodies()
            .iter()
            .filter(|custody| reachable.contains(custody.key()))
            .map(|custody| {
                revalidate_package_source_selection(custody)?;
                let binding = PackageSourceBinding::new(
                    custody.key().identity(),
                    custody.key().name().as_str(),
                    custody.snapshot_root().to_path_buf(),
                );
                if custody.key() == root {
                    binding_with_canonical_source_metadata(custody, binding)
                } else {
                    Ok(binding)
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
        PackageCompilationInputs::new(root.identity(), root_role, packages, dependencies)
    }
}

fn binding_with_canonical_source_metadata(
    custody: &crate::resolution::source::PackageSourceCustody,
    binding: PackageSourceBinding,
) -> Result<PackageSourceBinding, PackageCompilationInputError> {
    package_source::local::operations::capture_verified_package_source_snapshot(
        custody.snapshot_root(),
        custody.materialization().content(),
        custody.source_limits(),
    )
    .map_err(|error| PackageCompilationInputError::InvalidSourceRoot {
        identity: custody.key().identity(),
        path: custody.snapshot_root().to_path_buf(),
        reason: format!("could not derive canonical build-source metadata: {error}"),
    })?;
    let invalid = |reason: String| PackageCompilationInputError::InvalidSourceRoot {
        identity: custody.key().identity(),
        path: custody.snapshot_root().to_path_buf(),
        reason: format!("invalid canonical build-source metadata: {reason}"),
    };
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

fn revalidate_package_source_selection(
    custody: &crate::resolution::source::PackageSourceCustody,
) -> Result<(), PackageCompilationInputError> {
    custody.selection_evidence().revalidate().map_err(|error| {
        PackageCompilationInputError::InvalidSourceRoot {
            identity: custody.key().identity(),
            path: custody.snapshot_root().to_path_buf(),
            reason: format!("could not revalidate package selection evidence: {error}"),
        }
    })
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
