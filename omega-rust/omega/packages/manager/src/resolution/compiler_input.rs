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

#[cfg(test)]
mod tests {
    use super::{
        BTreeSet, PackageCompilationInputError, PackageCompilationScope, package_compilation_inputs,
    };
    use crate::declarations::dependencies::read::DependencySourceRequest;
    use crate::declarations::{PackageKey, PackageName};
    use crate::resolution::graph::PackageRootSourceRequest;
    use crate::resolution::graph::reconcile::resolve_package_source_closure;
    use crate::resolution::source::PackageSourceCustody;
    #[cfg(unix)]
    use checked_interpreter::CanonicalFilesystemMetadataRowKind;
    use package_source::{GitCommitId, GitTreeId, ImmutableSourceResolution, SourceLineage};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root_request(root: &PackageSourceCustody) -> PackageRootSourceRequest {
        PackageRootSourceRequest::Git(crate::resolution::source::GitPackageSourceRequest::root(
            package_source::GitSourceRequest::new(
                format!(
                    "https://github.com/CathedralOS/{}.git",
                    root.key().name().as_str()
                ),
                Some("HEAD".to_owned()),
            )
            .expect("synthetic root request"),
        ))
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-compiler-handoff-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn custody(
        name: &str,
        marker: u8,
        source_root: PathBuf,
        dependency_requests: Vec<DependencySourceRequest>,
    ) -> PackageSourceCustody {
        custody_with_scopes(
            name,
            marker,
            source_root,
            crate::declarations::dependencies::DependencyProjections::from(dependency_requests),
        )
    }

    fn custody_with_scopes(
        name: &str,
        marker: u8,
        source_root: PathBuf,
        projections: crate::declarations::dependencies::DependencyProjections,
    ) -> PackageSourceCustody {
        std::fs::create_dir_all(&source_root).expect("create source root");
        let source_root = source_root
            .canonicalize()
            .expect("retain the canonical synthetic source root");
        let source = package_source::resolve_local_source(
            &source_root,
            package_source::LocalSourceLimits::default(),
        )
        .expect("derive synthetic source identity");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&source_root, std::fs::Permissions::from_mode(0o555))
                .expect("seal synthetic source root");
        }
        let key = PackageKey::new(
            PackageName::parse(name).expect("package name"),
            SourceLineage::git(&format!("https://github.com/CathedralOS/{name}.git"))
                .expect("source lineage"),
        );
        let digit = char::from_digit(u32::from(marker), 16).expect("hex marker");
        let resolution = ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&digit.to_string().repeat(40)).expect("commit"),
            GitTreeId::parse_hex(&digit.to_string().repeat(40)).expect("tree"),
        )
        .expect("resolution");
        let materialization =
            crate::resolution::source::PackageSourceMaterialization::from_local(&source);
        PackageSourceCustody::from_resolved_parts(
            key,
            crate::declarations::BuildDeclarationKind::Package,
            resolution,
            materialization,
            source_root,
            crate::resolution::source::PackageSourceNavigation::Root,
            crate::resolution::source::PackageSourceSelectionEvidence::Root,
            package_source::LocalSourceLimits::default(),
            projections,
        )
    }

    #[test]
    fn translates_exact_keys_roots_and_requester_local_aliases() {
        let roots = temp_root("valid");
        let dependency = custody("arithmetic-kernels", 2, roots.join("dependency"), vec![]);
        let dependency_key = dependency.key().clone();
        let root = custody(
            "application",
            1,
            roots.join("root"),
            vec![DependencySourceRequest::Path {
                explicit_alias: None,
                location: "dependency".to_owned(),
            }],
        );
        let root_key = root.key().clone();
        let closure = resolve_package_source_closure(root_request(&root), root, |_, _| {
            Ok::<_, &'static str>(dependency.clone())
        })
        .expect("resolve source closure");

        let inputs = package_compilation_inputs(&closure).expect("compiler handoff validates");

        assert_eq!(inputs.root(), root_key.identity());
        assert_eq!(inputs.packages().count(), 2);
        let root_metadata = inputs
            .canonical_source_metadata(root_key.identity())
            .expect("resolved package handoff carries canonical source metadata");
        assert_ne!(root_metadata.source_content_commitment(), &[0; 32]);
        assert_eq!(root_metadata.rows().count(), 1);
        assert!(
            inputs
                .canonical_source_metadata(dependency_key.identity())
                .is_none(),
            "only the package whose build machine can run retains a metadata index"
        );
        assert_eq!(
            inputs.package_name(root_key.identity()),
            Some("application")
        );
        assert_eq!(
            inputs.package_name(dependency_key.identity()),
            Some("arithmetic-kernels")
        );
        assert_eq!(
            inputs.package_root(dependency_key.identity()),
            Some(
                dependency
                    .snapshot_root()
                    .canonicalize()
                    .expect("canonical dependency root")
                    .as_path()
            )
        );

        let _ = std::fs::remove_dir_all(roots);
    }

    #[cfg(unix)]
    #[test]
    fn checked_source_cache_replays_the_retained_index_for_an_unchanged_root() {
        use std::os::unix::fs::PermissionsExt;

        let roots = temp_root("checked-source-cache");
        let source_root = roots.join("root");
        std::fs::create_dir_all(&source_root).expect("create source root");
        std::fs::write(source_root.join("module.omg"), b"source bytes").expect("write root source");
        std::fs::set_permissions(
            source_root.join("module.omg"),
            std::fs::Permissions::from_mode(0o444),
        )
        .expect("seal root source");
        let cache_dir = roots.join("cache");

        let root = custody("application", 1, source_root.clone(), vec![])
            .with_checked_source_cache_dir(cache_dir.clone());
        let root_identity = root.key().identity();
        let canonical_root = root.snapshot_root().to_path_buf();
        let closure = resolve_package_source_closure(
            root_request(&root),
            root,
            |_, _| -> Result<PackageSourceCustody, &'static str> { unreachable!() },
        )
        .expect("resolve root-only closure");

        let first = package_compilation_inputs(&closure).expect("cold handoff validates");
        let second = package_compilation_inputs(&closure).expect("warm handoff validates");
        assert_eq!(
            first
                .canonical_source_metadata(root_identity)
                .map(|index| index.source_content_commitment()),
            second
                .canonical_source_metadata(root_identity)
                .map(|index| index.source_content_commitment()),
        );

        // The wired cold capture stored a self-verifying record into the
        // custody's retained lane; a direct capture through the same lane
        // replays it without rehashing, and a stat-visible drift misses.
        let replay = package_compilation::CheckedSourceCache::open(&cache_dir)
            .expect("open the lane the wiring populated")
            .capture(&canonical_root)
            .expect("capture the unchanged root");
        assert_eq!(
            replay.outcome(),
            package_compilation::CheckedSourceCacheOutcome::Warm
        );

        std::fs::set_permissions(&source_root, std::fs::Permissions::from_mode(0o755))
            .expect("unseal source root for drift");
        std::fs::write(source_root.join("added.omg"), b"drift").expect("drift the source");
        std::fs::set_permissions(
            source_root.join("added.omg"),
            std::fs::Permissions::from_mode(0o444),
        )
        .expect("seal drifted source");
        std::fs::set_permissions(&source_root, std::fs::Permissions::from_mode(0o555))
            .expect("re-seal source root");
        let drifted = package_compilation::CheckedSourceCache::open(&cache_dir)
            .expect("reopen the cache lane")
            .capture(&canonical_root)
            .expect("capture the drifted root");
        assert_eq!(
            drifted.outcome(),
            package_compilation::CheckedSourceCacheOutcome::Cold
        );

        let _ = std::fs::remove_dir_all(roots);
    }

    #[cfg(unix)]
    #[test]
    fn canonical_handoff_derives_directory_file_executable_and_symlink_rows() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let roots = temp_root("canonical-metadata");
        let source_root = roots.join("root");
        let nested = source_root.join("nested");
        std::fs::create_dir_all(&nested).expect("create nested source directory");
        std::fs::write(source_root.join("ordinary.omg"), b"ordinary")
            .expect("write ordinary source");
        std::fs::write(nested.join("generator"), b"#!/bin/omega\n")
            .expect("write executable source");
        symlink("ordinary.omg", source_root.join("ordinary-link"))
            .expect("create relative source symlink");
        std::fs::set_permissions(&nested, std::fs::Permissions::from_mode(0o555))
            .expect("seal nested source directory");
        std::fs::set_permissions(
            source_root.join("ordinary.omg"),
            std::fs::Permissions::from_mode(0o444),
        )
        .expect("seal ordinary source");
        std::fs::set_permissions(
            nested.join("generator"),
            std::fs::Permissions::from_mode(0o555),
        )
        .expect("seal executable source");

        let root = custody("application", 1, source_root, vec![]);
        let root_identity = root.key().identity();
        let closure = resolve_package_source_closure(
            root_request(&root),
            root,
            |_, _| -> Result<PackageSourceCustody, &'static str> { unreachable!() },
        )
        .expect("resolve root-only closure");
        let inputs = package_compilation_inputs(&closure).expect("compiler handoff validates");
        let rows = inputs
            .canonical_source_metadata(root_identity)
            .expect("canonical metadata")
            .rows()
            .map(|row| (row.relative_path().to_vec(), row.kind()))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(
            rows.get(b"".as_slice()),
            Some(&CanonicalFilesystemMetadataRowKind::Directory)
        );
        assert_eq!(
            rows.get(b"nested".as_slice()),
            Some(&CanonicalFilesystemMetadataRowKind::Directory)
        );
        assert_eq!(
            rows.get(b"ordinary.omg".as_slice()),
            Some(&CanonicalFilesystemMetadataRowKind::File {
                executable: false,
                logical_byte_length: 8,
            })
        );
        assert_eq!(
            rows.get(b"nested/generator".as_slice()),
            Some(&CanonicalFilesystemMetadataRowKind::File {
                executable: true,
                logical_byte_length: 13,
            })
        );
        assert_eq!(
            rows.get(b"ordinary-link".as_slice()),
            Some(&CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length: 12,
            })
        );

        std::fs::set_permissions(
            roots.join("root/nested"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("unseal nested source for cleanup");
        std::fs::set_permissions(roots.join("root"), std::fs::Permissions::from_mode(0o755))
            .expect("unseal source root for cleanup");
        let _ = std::fs::remove_dir_all(roots);
    }

    #[test]
    fn compiler_handoff_rechecks_snapshot_root_custody() {
        let roots = temp_root("drift");
        let root = custody("application", 1, roots.join("root"), vec![]);
        let root_path = root.snapshot_root().to_path_buf();
        let closure = resolve_package_source_closure(
            root_request(&root),
            root,
            |_, _| -> Result<PackageSourceCustody, &'static str> { unreachable!() },
        )
        .expect("resolve root-only closure");
        let scope = PackageCompilationScope::new(&closure, closure.graph().root());
        scope.compilation_inputs().expect("initial source custody");
        std::fs::remove_dir(&root_path).expect("remove source root after reconciliation");

        let errors = scope
            .compilation_inputs()
            .expect_err("retaining topology must not retain a source verification verdict");
        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::InvalidSourceRoot { .. }
        )));

        let _ = std::fs::remove_dir_all(roots);
    }

    #[test]
    fn retained_scope_deduplicates_diamonds_and_excludes_unreachable_siblings() {
        let roots = temp_root("rerooted");
        let shared = custody("shared", 4, roots.join("shared"), vec![]);
        let shared_key = shared.key().clone();
        let shared_request = DependencySourceRequest::Path {
            explicit_alias: None,
            location: "shared".to_owned(),
        };
        let first = custody(
            "arithmetic-kernels",
            2,
            roots.join("first"),
            vec![shared_request.clone()],
        );
        let second = custody(
            "capability-vault",
            3,
            roots.join("second"),
            vec![shared_request],
        );
        let first_key = first.key().clone();
        let root = custody(
            "application",
            1,
            roots.join("root"),
            vec![
                DependencySourceRequest::Path {
                    explicit_alias: None,
                    location: "first".to_owned(),
                },
                DependencySourceRequest::Path {
                    explicit_alias: None,
                    location: "second".to_owned(),
                },
            ],
        );
        let closure =
            resolve_package_source_closure(root_request(&root), root, |_, request| match request {
                DependencySourceRequest::Path { location, .. } if location == "first" => {
                    Ok::<_, &'static str>(first.clone())
                }
                DependencySourceRequest::Path { location, .. } if location == "second" => {
                    Ok(second.clone())
                }
                DependencySourceRequest::Path { location, .. } if location == "shared" => {
                    Ok(shared.clone())
                }
                _ => Err("unexpected request"),
            })
            .expect("resolve diamond dependency closure");

        let scope = PackageCompilationScope::new(&closure, &first_key);
        assert_eq!(
            scope.packages(),
            &BTreeSet::from([first_key.clone(), shared_key.clone()])
        );
        assert!(std::ptr::eq(scope.closure(), &closure));
        let inputs = scope
            .compilation_inputs()
            .expect("one branch can be compiled as a temporary root");

        assert_eq!(inputs.root(), first_key.identity());
        assert_eq!(inputs.packages().count(), 2);
        assert!(inputs.package_root(first_key.identity()).is_some());
        let dependency_closure = inputs.dependency_closure();
        assert_eq!(dependency_closure.root(), first_key.identity());
        assert_eq!(dependency_closure.packages().len(), 2);
        assert!(
            dependency_closure
                .packages()
                .contains(&shared_key.identity())
        );
        assert_eq!(dependency_closure.dependencies().len(), 1);

        let root_scope = PackageCompilationScope::new(&closure, closure.graph().root());
        assert_eq!(root_scope.packages().len(), 4);
        let root_inputs = root_scope
            .compilation_inputs()
            .expect("shared leaf appears once");
        assert_eq!(root_inputs.packages().count(), 4);
        assert_eq!(root_inputs.dependency_closure().dependencies().len(), 4);
        for (position, node) in closure.graph().packages().iter().enumerate() {
            assert_eq!(
                closure.graph().package_position(node.source().key()),
                Some(position)
            );
        }

        let _ = std::fs::remove_dir_all(roots);
    }

    #[test]
    fn build_purpose_edges_do_not_become_product_import_bindings() {
        let roots = temp_root("build-scope");
        let product = custody("product-lib", 2, roots.join("product"), Vec::new());
        let host = custody("host-tool", 3, roots.join("host"), Vec::new());
        let host_key = host.key().clone();
        let root = custody_with_scopes(
            "application",
            1,
            roots.join("root"),
            crate::declarations::dependencies::DependencyProjections::new(
                vec![DependencySourceRequest::Path {
                    explicit_alias: None,
                    location: "product".to_owned(),
                }]
                .into(),
                vec![DependencySourceRequest::Path {
                    explicit_alias: None,
                    location: "host".to_owned(),
                }]
                .into(),
            ),
        );
        let root_key = root.key().clone();
        let closure =
            resolve_package_source_closure(root_request(&root), root, |_, request| match request {
                DependencySourceRequest::Path { location, .. } if location == "product" => {
                    Ok::<_, &'static str>(product.clone())
                }
                DependencySourceRequest::Path { location, .. } if location == "host" => {
                    Ok(host.clone())
                }
                _ => Err("unexpected request"),
            })
            .expect("dual-scope closure resolves");

        let inputs = package_compilation_inputs(&closure).expect("compiler handoff validates");

        // Source custody acquired the host package and the root's build entry
        // may import it, so the host snapshot is a compilation input. It
        // still holds no product binding: a product-scope alias lookup misses
        // and the durable product closure never contains it.
        assert_eq!(inputs.packages().count(), 3);
        assert!(inputs.package_root(host_key.identity()).is_some());
        assert_eq!(
            inputs.dependency_target(root_key.identity(), "product_lib"),
            Some(product.key().identity())
        );
        assert_eq!(
            inputs.dependency_target(root_key.identity(), "host_tool"),
            None
        );
        assert_eq!(
            inputs.dependency_target_for_purpose(
                root_key.identity(),
                crate::declarations::DependencyPurpose::Build,
                "host_tool",
            ),
            Some(host_key.identity())
        );
        assert_eq!(
            inputs.dependency_target_for_purpose(
                root_key.identity(),
                crate::declarations::DependencyPurpose::Build,
                "product_lib",
            ),
            None,
            "a product edge never answers a build-scope lookup"
        );
        assert_eq!(
            inputs.build_dependencies().collect::<Vec<_>>(),
            vec![(root_key.identity(), "host_tool", host_key.identity())]
        );
        let dependency_closure = inputs.dependency_closure();
        assert_eq!(dependency_closure.packages().len(), 2);
        assert_eq!(dependency_closure.dependencies().len(), 1);

        let _ = std::fs::remove_dir_all(roots);
    }
}
