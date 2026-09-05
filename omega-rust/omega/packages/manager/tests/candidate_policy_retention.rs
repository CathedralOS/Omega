//! Live normalized findings are retained independently of review replay data.

use omega_package_evidence::{
    encoding::PackagePolicyRecoveryLimits, record::PackagePolicyBaseline,
};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_package_closure_with_storage,
};
use omega_package_manager::review::compile_resolved_package_candidate_reviews;
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use omega_target::TargetProfile;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-candidate-policy-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        writable(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Thaw only this test's immutable cache; never follow symlinks.
#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
fn writable(path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    #[cfg(unix)]
    if metadata.is_dir() {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
        )
        .unwrap();
    }
    #[cfg(windows)]
    {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).unwrap();
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            writable(&entry.unwrap().path());
        }
    }
}

#[test]
fn exact_package_and_target_policies_recover_without_the_source_or_review_set() {
    let tree = Tree::new();
    for (member, name, value, dependencies) in [
        (
            "root",
            "policy-root",
            7,
            "builder.depend_as(\"dependency\", Source::Path { location: \"../dependency\" });",
        ),
        ("dependency", "policy-dependency", 11, ""),
    ] {
        let root = tree.path(&format!("source/{member}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("build.omg"), format!("machine build(builder: &mut Build) {{ builder.package(\"{name}\"); {dependencies} }}\n")).unwrap();
        fs::write(
            root.join("main.omg"),
            format!("pub const VALUE: u64 = {value};\n"),
        )
        .unwrap();
    }
    let storage = SourceResolverStorage::for_hardened_base(tree.path("cache")).unwrap();
    let sources = resolve_external_local_package_closure_with_storage(
        tree.path("source/root"),
        ExternalSourceContext::derive(b"candidate-policy-retention"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve two exact package sources");
    let mut retained = Vec::new();
    for target in [TargetProfile::WindowsX64, TargetProfile::LinuxX64] {
        let reviews = compile_resolved_package_candidate_reviews(
            &sources.for_exact_target(target),
            &tree.path("build"),
        )
        .expect("compile final package candidate policies");
        assert_eq!(reviews.reviews().len(), 2);
        for review in reviews.reviews() {
            let policy = review.policy();
            assert_eq!(policy.package(), review.key().identity());
            assert_eq!(policy.target(), target);
            assert_eq!(policy.target(), review.projection().target());
            assert_eq!(policy.public_consts(), review.projection().public_consts());
            assert_eq!(policy.public_consts().len(), 1);
            assert_eq!(
                review.projection().canonical_review_bytes().unwrap(),
                review.canonical_review_bytes()
            );
            assert_eq!(
                review.projection().canonical_rows().unwrap(),
                review.canonical_rows()
            );
            retained.push((policy.clone(), policy.canonical_bytes().unwrap()));
        }
        // Only normalized values/bytes survive this iteration, not evidence.
        drop(reviews);
    }
    drop(sources);
    drop(storage);
    fs::remove_dir_all(tree.path("source")).unwrap();
    writable(&tree.path("cache"));
    fs::remove_dir_all(tree.path("cache")).unwrap();
    assert!(!tree.path("source").exists() && !tree.path("cache").exists());
    for (policy, bytes) in retained {
        let recovered = PackagePolicyBaseline::recover_canonical(
            &bytes,
            PackagePolicyRecoveryLimits::default(),
        )
        .unwrap();
        assert_eq!(recovered, policy);
        assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    }
}
