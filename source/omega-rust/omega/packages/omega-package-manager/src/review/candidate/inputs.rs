//! Revalidated package-aware inputs handed from source custody to the compiler.

use crate::graph::ResolvedPackageSourceClosure;
use omega_package_compilation::{
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
    root: &crate::identity::PackageKey,
) -> Result<PackageCompilationInputs, Vec<PackageCompilationInputError>> {
    let reachable = reachable_package_keys(closure, root);
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
    let dependencies = closure
        .graph()
        .packages()
        .iter()
        .filter(|package| reachable.contains(package.source().key()))
        .flat_map(|package| {
            package.dependencies().iter().map(|dependency| {
                PackageDependencyBinding::new(
                    package.source().key().identity(),
                    dependency.alias().as_str(),
                    dependency.target().identity(),
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

fn binding_with_canonical_source_metadata(
    custody: &crate::discovery::PackageSourceCustody,
    binding: PackageSourceBinding,
) -> Result<PackageSourceBinding, PackageCompilationInputError> {
    omega_package_source::local::operations::capture_verified_package_source_snapshot(
        custody.snapshot_root(),
        custody.materialization().content(),
        custody.source_limits(),
    )
    .map_err(|error| PackageCompilationInputError::InvalidSourceRoot {
        identity: custody.key().identity(),
        path: custody.snapshot_root().to_path_buf(),
        reason: format!("could not derive canonical build-source metadata: {error}"),
    })?;
    binding.with_canonical_source_metadata().map_err(|reason| {
        PackageCompilationInputError::InvalidSourceRoot {
            identity: custody.key().identity(),
            path: custody.snapshot_root().to_path_buf(),
            reason: format!("invalid canonical build-source metadata: {reason}"),
        }
    })
}

fn revalidate_package_source_selection(
    custody: &crate::discovery::PackageSourceCustody,
) -> Result<(), PackageCompilationInputError> {
    custody.selection_evidence().revalidate().map_err(|error| {
        PackageCompilationInputError::InvalidSourceRoot {
            identity: custody.key().identity(),
            path: custody.snapshot_root().to_path_buf(),
            reason: format!("could not revalidate package selection evidence: {error}"),
        }
    })
}

pub(crate) fn reachable_package_keys(
    closure: &ResolvedPackageSourceClosure,
    root: &crate::identity::PackageKey,
) -> BTreeSet<crate::identity::PackageKey> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root.clone()];
    while let Some(package) = pending.pop() {
        if !reachable.insert(package.clone()) {
            continue;
        }
        let Some(node) = closure.graph().package(&package) else {
            continue;
        };
        pending.extend(
            node.dependencies()
                .iter()
                .rev()
                .map(|dependency| dependency.target().clone()),
        );
    }
    reachable
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declarations::dependencies::read::DependencySourceRequest;
    use crate::discovery::PackageSourceCustody;
    use crate::graph::PackageRootSourceRequest;
    use crate::graph::reconcile::resolve_package_source_closure;
    use crate::identity::{PackageKey, PackageName};
    use omega_package_source::{GitCommitId, GitTreeId, ImmutableSourceResolution, SourceLineage};
    #[cfg(unix)]
    use psi_checked_interpreter::CanonicalFilesystemMetadataRowKind;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root_request(root: &PackageSourceCustody) -> PackageRootSourceRequest {
        PackageRootSourceRequest::Git(crate::discovery::GitPackageSourceRequest::root(
            omega_package_source::GitSourceRequest::new(
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
        std::fs::create_dir_all(&source_root).expect("create source root");
        let source_root = source_root
            .canonicalize()
            .expect("retain the canonical synthetic source root");
        let source = omega_package_source::resolve_local_source(
            &source_root,
            omega_package_source::LocalSourceLimits::default(),
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
        let materialization = crate::discovery::PackageSourceMaterialization::from_local(&source);
        PackageSourceCustody::from_resolved_parts(
            key,
            crate::declarations::BuildDeclarationKind::Package,
            resolution,
            materialization,
            source_root,
            crate::discovery::PackageSourceNavigation::Root,
            crate::discovery::PackageSourceSelectionEvidence::Root,
            omega_package_source::LocalSourceLimits::default(),
            dependency_requests,
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
        std::fs::remove_dir(&root_path).expect("remove source root after reconciliation");

        let errors = package_compilation_inputs(&closure)
            .expect_err("compiler handoff must reject drifted source custody");
        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::InvalidSourceRoot { .. }
        )));

        let _ = std::fs::remove_dir_all(roots);
    }

    #[test]
    fn rerooted_handoff_excludes_unreachable_siblings() {
        let roots = temp_root("rerooted");
        let first = custody("arithmetic-kernels", 2, roots.join("first"), vec![]);
        let second = custody("capability-vault", 3, roots.join("second"), vec![]);
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
                _ => Err("unexpected request"),
            })
            .expect("resolve diamond-free sibling closure");

        let inputs = package_compilation_inputs_for(&closure, &first_key)
            .expect("leaf package can be compiled as a temporary root");

        assert_eq!(inputs.root(), first_key.identity());
        assert_eq!(inputs.packages().count(), 1);
        assert!(inputs.package_root(first_key.identity()).is_some());
        let dependency_closure = inputs.dependency_closure();
        assert_eq!(dependency_closure.root(), first_key.identity());
        assert_eq!(dependency_closure.packages(), &[first_key.identity()]);
        assert!(dependency_closure.dependencies().is_empty());

        let _ = std::fs::remove_dir_all(roots);
    }
}
