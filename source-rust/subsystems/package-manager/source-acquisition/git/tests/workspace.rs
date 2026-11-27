// Git workspace member projection tests verifying that selected member
// resolution authenticates root and member declarations without materializing
// unrelated repository payloads.

use super::{
    GitWorkspaceDeclarationLimits, GitWorkspaceSession, LocalSourceLimits, SourceRelativePath,
    create_git_source, create_private_dir_all, fixture_storage, local_git_request, remove_fixture,
    run_test_git, temp_root,
};

#[test]
fn selected_workspace_member_never_materializes_unrelated_repository_payloads() {
    let (repository, _) = create_git_source("workspace-member");
    std::fs::create_dir_all(repository.join("packages/member/source"))
        .expect("create member source");
    std::fs::write(repository.join("build.omg"), b"workspace declaration\n")
        .expect("write root declaration");
    std::fs::write(
        repository.join("packages/member/build.omg"),
        b"member declaration\n",
    )
    .expect("write member declaration");
    std::fs::write(
        repository.join("packages/member/source/lib.omg"),
        b"machine Member::value() {}\n",
    )
    .expect("write member source");
    std::fs::write(repository.join("unrelated.bin"), vec![b'x'; 4096])
        .expect("write unrelated oversized payload");
    run_test_git(&repository, ["add", "."]);
    run_test_git(
        &repository,
        ["commit", "--quiet", "-m", "add projected workspace"],
    );

    let storage_base = temp_root("workspace-member-storage");
    create_private_dir_all(&storage_base).expect("create storage base");
    let request = local_git_request(&repository, "HEAD");
    let storage = fixture_storage(&storage_base, &request);
    let member = SourceRelativePath::parse("packages/member").expect("member path");
    let session = GitWorkspaceSession::open(
        &storage,
        &request,
        None,
        LocalSourceLimits {
            max_entries: 32,
            max_bytes: 1024,
            max_depth: 8,
        },
        GitWorkspaceDeclarationLimits::new(8, 256, 512),
    )
    .expect("open workspace session");
    assert_eq!(
        session.root_declaration().bytes(),
        b"workspace declaration\n"
    );
    let catalog = session
        .open_members(&storage, std::slice::from_ref(&member))
        .expect("read member declaration");
    let [declaration] = catalog.member_declarations() else {
        panic!("one requested member declaration");
    };
    assert_eq!(declaration.member_path(), Some(&member));
    assert_eq!(declaration.bytes(), b"member declaration\n");
    let source = catalog
        .materialize(&storage, &member)
        .expect("resolve selected workspace member");

    assert_eq!(
        std::fs::read(source.snapshot_root().join("build.omg")).expect("read selected declaration"),
        b"member declaration\n"
    );
    assert!(source.snapshot_root().join("source/lib.omg").is_file());
    assert!(!source.snapshot_root().join("unrelated.bin").exists());
    assert!(!source.snapshot_root().join("packages").exists());
    assert_ne!(source.tree(), source.materialized_tree());
    let projection = source
        .workspace_projection()
        .expect("selected member projection custody");
    assert_eq!(projection.selected_member_path(), &member);
    assert_eq!(
        projection.selected_member_tree(),
        source.materialized_tree()
    );
    assert_eq!(
        projection.root_declaration().bytes(),
        b"workspace declaration\n"
    );
    assert_eq!(projection.member_declarations().len(), 1);
    assert!(
        source
            .snapshot_root()
            .starts_with(storage.workspace_members().path())
    );
    assert_eq!(source.lineage(), request.lineage());
    assert_eq!(source.selected_member(), Some(&member));

    drop(catalog);
    drop(storage);
    remove_fixture(&repository, &storage_base);
}
