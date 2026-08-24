use crate::closure_resolution::ResolvedPackageSourceClosure;
use omega_compiler::{
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
            PackageSourceBinding::new(
                custody.key().identity(),
                custody.snapshot_root().to_path_buf(),
            )
        })
        .collect();
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

    PackageCompilationInputs::new(root.identity(), packages, dependencies)
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
    use crate::closure_resolution::{PackageSourceCustody, resolve_package_source_closure};
    use crate::dependency_projection::DependencySourceRequest;
    use crate::identity::{
        GitCommitId, GitTreeId, ImmutableSourceResolution, PackageKey, PackageName,
        SourceContentDigest, SourceLineage,
    };
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        let key = PackageKey::new(
            PackageName::parse(name).expect("package name"),
            SourceLineage::git(&format!("https://github.com/CathedralOS/{name}.git"))
                .expect("source lineage"),
        );
        let digit = char::from_digit(u32::from(marker), 16).expect("hex marker");
        let resolution = ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&digit.to_string().repeat(40)).expect("commit"),
            GitTreeId::parse_hex(&digit.to_string().repeat(40)).expect("tree"),
            SourceContentDigest::derive(&[marker]),
        )
        .expect("resolution");
        PackageSourceCustody::from_resolved_parts(
            key,
            resolution,
            source_root,
            crate::LocalSourceLimits::default(),
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
        let closure =
            resolve_package_source_closure(root, |_, _| Ok::<_, &'static str>(dependency.clone()))
                .expect("resolve source closure");

        let inputs = package_compilation_inputs(&closure).expect("compiler handoff validates");

        assert_eq!(inputs.root(), root_key.identity());
        assert_eq!(inputs.packages().count(), 2);
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

    #[test]
    fn compiler_handoff_rechecks_snapshot_root_custody() {
        let roots = temp_root("drift");
        let root = custody("application", 1, roots.join("root"), vec![]);
        let root_path = root.snapshot_root().to_path_buf();
        let closure = resolve_package_source_closure(
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
        let closure = resolve_package_source_closure(root, |_, request| match request {
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

        let _ = std::fs::remove_dir_all(roots);
    }
}
