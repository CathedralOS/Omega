use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) repository: PathBuf,
    pub(super) locator: String,
}

impl Fixture {
    pub(super) fn package(label: &str, name: &str, application: bool) -> Self {
        let fixture = Self::empty(label);
        if application {
            write_application(&fixture.repository, name, None);
        } else {
            write_package(&fixture.repository, name, None);
        }
        fixture.initialize();
        fixture
    }

    pub(super) fn workspace(label: &str) -> Self {
        let fixture = Self::empty(label);
        std::fs::write(
            fixture.repository.join("build.omg"),
            "machine build(builder: &mut Build) {\n    builder.member(\"packages/left\");\n    builder.member(\"packages/right\");\n}\n",
        )
        .unwrap();
        write_package(
            &fixture.repository.join("packages/left"),
            "left",
            Some("../right"),
        );
        write_package(&fixture.repository.join("packages/right"), "right", None);
        fixture.initialize();
        fixture
    }

    fn empty(label: &str) -> Self {
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = temp_root(&format!("git-pins-{label}-{serial}"));
        let repository = root.join("repository");
        std::fs::create_dir_all(&repository).unwrap();
        Self {
            root,
            repository,
            locator: format!("https://github.com/CathedralOS/{label}.git"),
        }
    }

    fn initialize(&self) {
        run_test_git(&self.repository, ["init", "--quiet"]);
        run_test_git(
            &self.repository,
            ["config", "user.email", "omega@example.invalid"],
        );
        run_test_git(&self.repository, ["config", "user.name", "Omega Tests"]);
        self.commit();
        run_test_git(&self.repository, ["branch", "-M", "main"]);
    }

    pub(super) fn commit(&self) {
        run_test_git(&self.repository, ["add", "."]);
        run_test_git(
            &self.repository,
            ["commit", "--quiet", "-m", "fixture source"],
        );
    }

    pub(super) fn advance(&self, path: &str) {
        std::fs::write(self.repository.join(path), "machine advanced() {}\n").unwrap();
        self.commit();
    }

    pub(super) fn disconnect(&self) {
        std::fs::rename(&self.repository, self.root.join("offline-repository")).unwrap();
    }

    pub(super) fn storage(&self, label: &str) -> SourceResolverStorage {
        SourceResolverStorage::for_hardened_base(self.root.join(label)).unwrap()
    }

    pub(super) fn request(&self) -> GitPackageSourceRequest {
        self.request_at(&self.locator, "main", PackageSelection::Root)
    }

    pub(super) fn named(&self, name: &str) -> GitPackageSourceRequest {
        self.request_at(
            &self.locator,
            "main",
            PackageSelection::Named(PackageName::parse(name).unwrap()),
        )
    }

    pub(super) fn request_at(
        &self,
        locator: &str,
        revision: &str,
        selection: PackageSelection,
    ) -> GitPackageSourceRequest {
        GitPackageSourceRequest::new(
            GitSourceRequest::for_local_test_repository_with_lineage(
                &self.repository,
                Some(revision.to_owned()),
                locator,
            )
            .unwrap(),
            selection,
        )
    }

    pub(super) fn subject(
        &self,
        request: &GitPackageSourceRequest,
        storage: &SourceResolverStorage,
        application: bool,
    ) -> CanonicalSourceClosureSubject {
        let resolver = if application {
            crate::resolution::graph::resolve_selected_git_project_closure_with_storage
        } else {
            crate::resolution::graph::resolve_selected_git_package_closure_with_storage
        };
        let closure = resolver(
            request,
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap();
        CanonicalSourceClosureSubject::from_resolved(
            &closure.for_exact_target(target::TargetProfile::CrossPlatformCli),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fn prepare(path: &Path) {
            let Ok(metadata) = std::fs::symlink_metadata(path) else {
                return;
            };
            if metadata.file_type().is_symlink() {
                return;
            }
            make_owner_writable(path);
            if metadata.is_dir() {
                for entry in std::fs::read_dir(path).unwrap().flatten() {
                    prepare(&entry.path());
                }
            }
        }
        prepare(&self.root);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub(super) fn make_owner_writable(path: &Path) {
    let metadata = std::fs::metadata(path).unwrap();
    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | if metadata.is_dir() { 0o700 } else { 0o600 });
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions).unwrap();
}

pub(super) fn resolve(
    cache: &mut GitAcquisitionCache<'_>,
    request: &GitPackageSourceRequest,
    storage: &SourceResolverStorage,
    application: bool,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let git = SourceCacheLane::Retained(storage.git_sources());
    let members = SourceCacheLane::Retained(storage.workspace_members());
    if application {
        cache.resolve_selected_project(request, git, members, LocalSourceLimits::default())
    } else {
        cache.resolve_selected(request, git, members, LocalSourceLimits::default())
    }
}

pub(super) fn commit(resolution: &package_source::ImmutableSourceResolution) -> String {
    let package_source::ImmutableSourceResolution::Git { commit, .. } = resolution else {
        panic!("expected real Git resolution")
    };
    commit.to_hex()
}

pub(super) fn package_key(subject: &CanonicalSourceClosureSubject, name: &str) -> PackageKey {
    subject
        .packages()
        .iter()
        .find(|source| source.key().name().as_str() == name)
        .expect("accepted package")
        .key()
        .clone()
}
