//! Staged catalog reads must pin source without materializing a whole repository.
use super::{
    create_git_source, local_git_request, make_tree_owner_writable, run_test_git, temp_root,
    test_system_git_executor,
};
use crate::git::resolution::GitWorkspaceSession;
use crate::{
    GitWorkspaceDeclarationLimits, LocalSourceLimits, PrimaryGitChoices, SourceRelativePath,
    SourceResolveError, SourceResolver,
};
use std::path::PathBuf;

struct WorkspaceFixture {
    repository: PathBuf,
    storage_root: PathBuf,
    storage: SourceResolver,
}

impl WorkspaceFixture {
    fn new() -> Self {
        let (repository, _) = create_git_source("staged-workspace");
        std::fs::write(repository.join("build.omg"), b"root catalog\n").unwrap();
        for member in ["first", "second", "oversized"] {
            std::fs::create_dir_all(repository.join(member)).unwrap();
            std::fs::write(repository.join(member).join("build.omg"), member).unwrap();
            std::fs::write(
                repository.join(member).join("lib.omg"),
                b"original source\n",
            )
            .unwrap();
        }
        std::fs::write(repository.join("oversized/payload"), vec![b'x'; 2048]).unwrap();
        std::fs::create_dir_all(repository.join("unrelated")).unwrap();
        for ordinal in 0..40 {
            std::fs::write(
                repository.join(format!("unrelated/{ordinal}")),
                vec![b'z'; 2048],
            )
            .unwrap();
        }
        run_test_git(&repository, ["add", "."]);
        run_test_git(&repository, ["commit", "--quiet", "-m", "workspace"]);
        let storage_root = temp_root("staged-workspace-storage");
        std::fs::create_dir_all(&storage_root).unwrap();
        let request = local_git_request(&repository, "HEAD");
        let git = test_system_git_executor(request.execution_transport())
            .unwrap()
            .execution_backend
            .executable()
            .to_path_buf();
        let storage = SourceResolver::for_hardened_base(
            &storage_root,
            PrimaryGitChoices {
                explicit_git: Some(&git),
                ..PrimaryGitChoices::default()
            },
        )
        .unwrap();
        Self {
            repository,
            storage_root,
            storage,
        }
    }

    fn open(&self) -> GitWorkspaceSession {
        GitWorkspaceSession::open(
            &self.storage,
            &local_git_request(&self.repository, "HEAD"),
            None,
            LocalSourceLimits {
                max_entries: 16,
                max_bytes: 1024,
                max_depth: 8,
            },
            GitWorkspaceDeclarationLimits::new(4, 128, 512),
        )
        .unwrap()
    }
}

impl Drop for WorkspaceFixture {
    fn drop(&mut self) {
        make_tree_owner_writable(&self.storage_root);
        let _ = std::fs::remove_dir_all(&self.storage_root);
        let _ = std::fs::remove_dir_all(&self.repository);
    }
}

fn member(name: &str) -> SourceRelativePath {
    SourceRelativePath::parse(name).unwrap()
}

#[test]
fn staged_workspace_pins_catalog_and_materializes_only_selected_payloads() {
    let fixture = WorkspaceFixture::new();
    let session = fixture.open();
    let commit = session.commit().to_owned();
    assert_eq!(session.root_declaration().bytes(), b"root catalog\n");
    // Move the authored selector while Build is inspecting the catalog.
    std::fs::write(
        fixture.repository.join("second/lib.omg"),
        b"changed source\n",
    )
    .unwrap();
    run_test_git(&fixture.repository, ["add", "."]);
    run_test_git(
        &fixture.repository,
        ["commit", "--quiet", "-m", "move HEAD"],
    );
    // A separate operation advances the shared selector cache as well. The
    // suspended session and an explicit reuse pin must still select old bytes.
    assert_ne!(fixture.open().commit(), commit);
    let reopened = GitWorkspaceSession::open(
        &fixture.storage,
        session.request(),
        Some(&session.acquisition_pin()),
        session.source_limits(),
        GitWorkspaceDeclarationLimits::new(4, 128, 512),
    )
    .unwrap();
    assert_eq!(reopened.commit(), commit);
    let recorded = GitWorkspaceSession::open_at_revision(
        &fixture.storage,
        session.request(),
        &crate::GitCommitId::parse_hex(session.commit()).unwrap(),
        &crate::GitTreeId::parse_hex(session.tree()).unwrap(),
        crate::git::resolution::GitExactRevisionAcquisition::Offline,
        session.source_limits(),
        GitWorkspaceDeclarationLimits::new(4, 128, 512),
    )
    .unwrap();
    assert_eq!(recorded.commit(), commit);
    let catalog = session
        .open_members(
            &fixture.storage,
            &[member("first"), member("second"), member("oversized")],
        )
        .unwrap();
    assert_eq!(catalog.member_declarations().len(), 3);
    for name in ["first", "second"] {
        let source = catalog
            .materialize(&fixture.storage, &member(name))
            .unwrap();
        assert_eq!(source.commit(), commit);
        assert_eq!(
            std::fs::read(source.snapshot_root().join("lib.omg")).unwrap(),
            b"original source\n"
        );
        assert!(!source.snapshot_root().join("unrelated").exists());
        assert_eq!(source.selected_member(), Some(&member(name)));
        assert_eq!(
            source
                .workspace_projection()
                .unwrap()
                .member_declarations()
                .len(),
            3
        );
    }
    assert!(matches!(
        catalog.materialize(&fixture.storage, &member("oversized")),
        Err(SourceResolveError::TooManyBytes { limit: 1024 })
    ));
    assert!(matches!(
        catalog.materialize(&fixture.storage, &member("unrelated")),
        Err(SourceResolveError::GitTreeInvalid { .. })
    ));
}

#[test]
fn staged_workspace_refuses_missing_duplicate_and_excess_declaration_requests() {
    let fixture = WorkspaceFixture::new();
    assert!(
        fixture
            .open()
            .open_members(&fixture.storage, &[member("missing")])
            .is_err()
    );
    assert!(matches!(
        fixture
            .open()
            .open_members(&fixture.storage, &[member("first"), member("first")]),
        Err(SourceResolveError::GitTreeInvalid { .. })
    ));
    assert!(matches!(
        fixture.open().open_members(
            &fixture.storage,
            &[
                member("first"),
                member("second"),
                member("oversized"),
                member("fourth"),
                member("fifth")
            ]
        ),
        Err(SourceResolveError::TooManyFiles { limit: 4 })
    ));
}
