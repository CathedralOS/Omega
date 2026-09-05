use super::*;

pub(super) struct Fixture {
    pub(super) repository: PathBuf,
    storage_base: PathBuf,
    pub(super) storage: SourceResolverStorage,
    pub(super) request: GitSourceRequest,
    pub(super) commit: GitCommitId,
    pub(super) tree: GitTreeId,
}

impl Fixture {
    pub(super) fn new(name: &str) -> Self {
        Self::with_format(name, None)
    }

    pub(super) fn with_format(name: &str, format: Option<&str>) -> Self {
        let (repository, commit) = create_git_source_with_format(name, format);
        let tree = run_test_git_with_input(&repository, ["rev-parse", "HEAD^{tree}"], b"");
        let request = local_git_request(&repository, "HEAD");
        let storage_base = temp_root(name);
        let executable = test_system_git_executor(request.execution_transport())
            .unwrap()
            .execution_backend
            .executable()
            .to_path_buf();
        let storage =
            SourceResolverStorage::for_hardened_base_with_primary_git(&storage_base, executable)
                .unwrap();
        Self {
            repository,
            storage_base,
            storage,
            request,
            // These are persisted hex inputs, not privately issued reuse pins.
            commit: GitCommitId::parse_hex(&commit).unwrap(),
            tree: GitTreeId::parse_hex(&tree).unwrap(),
        }
    }

    pub(super) fn resolve(
        &self,
        mode: GitExactRevisionAcquisition,
    ) -> Result<ResolvedGitSource, SourceResolveError> {
        self.resolve_objects(&self.commit, &self.tree, mode)
    }

    pub(super) fn resolve_objects(
        &self,
        commit: &GitCommitId,
        tree: &GitTreeId,
        mode: GitExactRevisionAcquisition,
    ) -> Result<ResolvedGitSource, SourceResolveError> {
        resolve_git_source_at_revision_in_lane(
            &self.request,
            commit,
            tree,
            mode,
            self.storage.git_sources(),
            LocalSourceLimits::default(),
        )
    }

    pub(super) fn advance(&self) -> (GitCommitId, GitTreeId) {
        std::fs::write(
            self.repository.join("main.omg"),
            b"machine Main::changed() {}\n",
        )
        .unwrap();
        run_test_git(&self.repository, ["add", "main.omg"]);
        run_test_git(
            &self.repository,
            ["commit", "--quiet", "-m", "advance source"],
        );
        let commit = run_test_git_with_input(&self.repository, ["rev-parse", "HEAD"], b"");
        let tree = run_test_git_with_input(&self.repository, ["rev-parse", "HEAD^{tree}"], b"");
        (
            GitCommitId::parse_hex(&commit).unwrap(),
            GitTreeId::parse_hex(&tree).unwrap(),
        )
    }

    pub(super) fn disconnect(&mut self) {
        let offline = self.repository.with_extension("offline");
        std::fs::rename(&self.repository, &offline).unwrap();
        self.repository = offline;
    }

    pub(super) fn entry(&self) -> PathBuf {
        self.request_entry(&self.request)
    }

    pub(super) fn request_entry(&self, request: &GitSourceRequest) -> PathBuf {
        let identity = git_cache_identity(
            request.locator_identity(),
            request.requested_revision(),
            request.execution_transport(),
        );
        self.storage
            .git_sources()
            .path()
            .join(format!("git-{identity}"))
    }

    pub(super) fn assert_original(&self, resolved: &ResolvedGitSource) {
        assert_eq!(resolved.commit(), self.commit.to_hex());
        assert_eq!(resolved.tree(), self.tree.to_hex());
        assert_eq!(resolved.materialized_tree(), self.tree.to_hex());
        assert_eq!(resolved.requested_revision(), "HEAD");
        assert!(resolved.selected_member().is_none());
        assert_eq!(
            std::fs::read(resolved.snapshot_root().join("main.omg")).unwrap(),
            b"machine Main::main() {}\n"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.repository);
        make_tree_owner_writable(&self.storage_base);
        let _ = std::fs::remove_dir_all(&self.storage_base);
    }
}
