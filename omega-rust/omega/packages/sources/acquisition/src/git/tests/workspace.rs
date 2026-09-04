use super::*;

struct FixedWorkspacePlanner {
    member: SourceRelativePath,
}

impl GitWorkspaceProjectionPlanner for FixedWorkspacePlanner {
    type Error = &'static str;
    type Evidence = String;

    fn discover_members(
        &mut self,
        root_declaration: &GitWorkspaceDeclaration,
    ) -> Result<Vec<SourceRelativePath>, Self::Error> {
        if root_declaration.bytes() != b"workspace declaration\n" {
            return Err("unexpected root declaration");
        }
        Ok(vec![self.member.clone()])
    }

    fn select_member(
        &mut self,
        _root_declaration: &GitWorkspaceDeclaration,
        member_declarations: &[GitWorkspaceDeclaration],
    ) -> Result<GitWorkspaceSelection<Self::Evidence>, Self::Error> {
        let [declaration] = member_declarations else {
            return Err("unexpected member declarations");
        };
        if declaration.member_path() != Some(&self.member)
            || declaration.bytes() != b"member declaration\n"
        {
            return Err("unexpected member declaration");
        }
        Ok(GitWorkspaceSelection::new(
            self.member.clone(),
            "manager evidence".to_owned(),
        ))
    }
}

#[test]
fn selected_workspace_member_never_materializes_unrelated_repository_payloads() {
    let (repository, _) = create_git_source("workspace-projection-resolution");
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

    let storage_base = temp_root("workspace-projection-resolution-storage");
    std::fs::create_dir_all(&storage_base).expect("create storage base");
    let request = local_git_request(&repository, "HEAD");
    let primary_git = test_system_git_executor(request.execution_transport())
        .expect("select fixture Git")
        .execution_backend
        .executable()
        .to_path_buf();
    let storage =
        SourceResolverStorage::for_hardened_base_with_primary_git(&storage_base, primary_git)
            .expect("retain resolver storage");
    let member = SourceRelativePath::parse("packages/member").expect("member path");
    let mut planner = FixedWorkspacePlanner {
        member: member.clone(),
    };
    let result = resolve_git_workspace_member_with_storage(
        &request,
        &storage,
        LocalSourceLimits {
            max_entries: 32,
            max_bytes: 1024,
            max_depth: 8,
        },
        GitWorkspaceDeclarationLimits::new(8, 256, 512),
        &mut planner,
    )
    .expect("resolve selected workspace member");
    let (source, evidence) = result.into_parts();

    assert_eq!(evidence, "manager evidence");
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

    drop(storage);
    let _ = std::fs::remove_dir_all(&repository);
    make_tree_owner_writable(&storage_base);
    let _ = std::fs::remove_dir_all(&storage_base);
}
