use super::*;

pub(super) struct Project(pub(super) PathBuf);

impl Project {
    pub(super) fn new(role: &str) -> Self {
        let project = Self(temporary_root("locked"));
        for (directory, name, role, dependencies) in [
            (
                "project",
                "root",
                role,
                " builder.depend_as(\"dependency\", Source::Path { location: \"../dependency\" });\n",
            ),
            ("dependency", "dependency", "package", ""),
        ] {
            let root = project.0.join(directory);
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("build.omg"), format!("machine build(builder: &mut Build) {{\n builder.{role}(\"{name}\");\n{dependencies}\n}}\n")).unwrap();
            fs::write(root.join("main.omg"), "pub machine value() -> u64 { 7 }\n").unwrap();
        }
        project
    }

    pub(super) fn root(&self) -> PathBuf {
        self.0.join("project")
    }

    pub(super) fn storage(&self) -> SourceResolverStorage {
        SourceResolverStorage::for_hardened_base(self.0.join("cache")).unwrap()
    }

    pub(super) fn resolve(&self, root: &Path) -> ResolvedPackageSourceClosure {
        resolve_external_local_project_closure_with_storage(
            root,
            ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
            &self.storage(),
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap()
    }

    pub(super) fn prepare(&self) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
        prepare_with_storage(&self.root().join("main.omg"), TargetProfile::host(), |_| {
            Ok(self.storage())
        })
    }

    pub(super) fn lock(&self) -> PackageLock {
        let closure = self.resolve(&self.root().canonicalize().unwrap());
        self.lock_closure(&closure)
    }

    pub(super) fn lock_closure(&self, closure: &ResolvedPackageSourceClosure) -> PackageLock {
        let target = closure.for_exact_target(TargetProfile::host());
        let reviews = compile_resolved_package_reviews(&target, &self.0.join("review")).unwrap();
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
        let lock = PackageLock::from_targets(vec![
            PackageLockTarget::from_parts(subject, baselines, decisions).unwrap(),
        ])
        .unwrap();
        fs::write(
            self.root().join("omega.lock"),
            lock.canonical_text().unwrap(),
        )
        .unwrap();
        lock
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        writable(&self.0);
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn writable(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
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
        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "Windows read-only attributes do not change Unix permission bits"
        )]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).unwrap();
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            writable(&entry.unwrap().path());
        }
    }
}
