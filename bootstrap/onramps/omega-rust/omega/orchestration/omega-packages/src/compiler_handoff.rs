use crate::closure_resolution::ResolvedPackageSourceClosure;
use omega_compiler::{
    PackageCompilationInputError, PackageCompilationInputs, PackageDependencyBinding,
    PackageSourceBinding,
};

/// Translate resolver-owned closure custody into the compiler's independently
/// validated package-aware input graph.
///
/// Package identities come from `PackageKey`; snapshot paths remain routing
/// custody only. `PackageCompilationInputs::new` canonicalizes and rechecks the
/// complete graph rather than trusting the package-side structural verdict.
pub fn package_compilation_inputs(
    closure: &ResolvedPackageSourceClosure,
) -> Result<PackageCompilationInputs, Vec<PackageCompilationInputError>> {
    let packages = closure
        .custodies()
        .iter()
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

    PackageCompilationInputs::new(closure.graph().root().identity(), packages, dependencies)
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
        PackageSourceCustody::from_resolved_parts(key, resolution, source_root, dependency_requests)
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
}
