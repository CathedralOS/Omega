use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Tree(pub(super) PathBuf);
impl Tree {
    pub(super) fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-locked-recovery-{}-{stamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
    pub(super) fn storage(&self, name: &str) -> SourceResolverStorage {
        SourceResolverStorage::for_hardened_base(self.path(name)).unwrap()
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        writable(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

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
        let _ = fs::set_permissions(
            path,
            fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
        );
    }
    #[cfg(windows)]
    {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        let _ = fs::set_permissions(path, permissions);
    }
    if metadata.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten() {
            writable(&entry.path());
        }
    }
}

pub(super) fn package(path: &Path, name: &str, dependencies: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(path.join("build.omg"), format!("machine build(builder: &mut Build) {{\n builder.package(\"{name}\");\n{dependencies}}}\n")).unwrap();
    fs::write(path.join("main.omg"), "pub machine value() -> u64 { 7 }\n").unwrap();
}

pub(super) fn capture_lock(
    closure: &ResolvedPackageSourceClosure,
    build: &Path,
) -> (PackageLock, PackageRootSourceRequest) {
    let target = closure.for_exact_target(TARGET);
    let reviews = compile_resolved_package_reviews(&target, build).unwrap();
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &target,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    let baselines = subject
        .packages()
        .iter()
        .map(|package| reviews.review(package.key()).unwrap().policy().clone())
        .collect();
    let decisions = HistoricalPackagePolicyDecisions::recover_text(
        &format!(
            "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
            subject.fingerprint().to_hex()
        ),
        &subject,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    let target = PackageLockTarget::from_parts(subject, baselines, decisions).unwrap();
    let original = PackageLock::from_targets(vec![target]).unwrap();
    let text = original.canonical_text().unwrap();
    let lock = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert_eq!(lock, original);
    (lock, closure.source_requests().root().request().clone())
}

pub(super) fn assert_fresh_matches(lock: &PackageLock, fresh: &ResolvedPackageSourceClosure) {
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &fresh.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    assert_eq!(&subject, lock.target(TARGET).unwrap().source());
    let inputs = package_compilation_inputs(fresh)
        .expect("fresh resolver custody forms real compiler inputs");
    assert_eq!(inputs.root(), fresh.graph().root().identity());
    assert_eq!(inputs.packages().count(), subject.packages().len());
}
