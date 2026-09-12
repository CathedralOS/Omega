use super::*;

const MISSING_COMMIT: &str = "1111111111111111111111111111111111111111";

fn add_submodule(fixture: &mut Fixture, manifest: &str, link: &str) {
    std::fs::write(
        fixture.repository.join(manifest),
        b"[submodule \"unavailable\"]\n",
    )
    .unwrap();
    run_test_git(&fixture.repository, ["add", manifest]);
    run_test_git(
        &fixture.repository,
        [
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            MISSING_COMMIT,
            link,
        ],
    );
    run_test_git(
        &fixture.repository,
        ["commit", "--quiet", "-m", "inert submodule edge"],
    );
    fixture.commit = GitCommitId::parse_hex(&run_test_git_with_input(
        &fixture.repository,
        ["rev-parse", "HEAD"],
        b"",
    ))
    .unwrap();
    fixture.root_tree = GitTreeId::parse_hex(&run_test_git_with_input(
        &fixture.repository,
        ["rev-parse", "HEAD^{tree}"],
        b"",
    ))
    .unwrap();
    fixture.member_tree = GitTreeId::parse_hex(&run_test_git_with_input(
        &fixture.repository,
        ["rev-parse", "HEAD:packages/member"],
        b"",
    ))
    .unwrap();
}

#[test]
fn unrelated_submodules_remain_inert_in_exact_member_and_offline_recovery() {
    let mut fixture = Fixture::new("workspace-unrelated-submodule");
    add_submodule(&mut fixture, ".gitmodules", "unavailable");
    let first = fixture
        .resolve(
            GitExactRevisionAcquisition::AllowFetch,
            &mut Planner::default(),
        )
        .unwrap();
    fixture.assert_original(&first);
    assert!(!first.source().snapshot_root().join(".gitmodules").exists());
    assert!(!first.source().snapshot_root().join("unavailable").exists());
    fixture.disconnect();
    let recovered = fixture
        .resolve(
            GitExactRevisionAcquisition::Offline,
            &mut Planner::default(),
        )
        .unwrap();
    fixture.assert_original(&recovered);
    assert_eq!(
        recovered.source().content_identity(),
        first.source().content_identity()
    );
}

#[test]
fn selected_member_submodule_metadata_or_gitlink_rejects() {
    for (manifest, link) in [
        ("packages/member/.gitmodules", "unavailable"),
        (".gitmodules", "packages/member/unavailable"),
    ] {
        let mut fixture = Fixture::new("workspace-selected-submodule");
        add_submodule(&mut fixture, manifest, link);
        let result = fixture.resolve(
            GitExactRevisionAcquisition::AllowFetch,
            &mut Planner::default(),
        );
        assert!(matches!(
            result,
            Err(GitWorkspaceProjectionError::Source(
                SourceResolveError::GitSubmodulesUnsupported { .. }
            ))
        ));
    }
}
