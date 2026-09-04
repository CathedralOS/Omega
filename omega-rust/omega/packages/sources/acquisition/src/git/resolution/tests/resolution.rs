use super::*;

#[test]
fn git_source_resolves_exact_commit_and_local_identity() {
    let (repo, commit) = create_git_source("git");
    let cache = temp_root("git-cache");

    let request = local_git_request(&repo, &commit);
    let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve git source");

    assert_eq!(resolved.commit, commit);
    assert_eq!(resolved.local.file_count, 1);
    assert!(!resolved.tree.is_empty());
    assert_eq!(resolved.lineage(), request.lineage());
    assert_eq!(resolved.requested_revision(), request.requested_revision());
    assert_eq!(resolved.object_format(), GitObjectIdAlgorithm::Sha1);
    assert_eq!(resolved.content_identity(), resolved.local.content_identity);
    assert_eq!(resolved.snapshot_root(), resolved.local.root);
    assert_eq!(resolved.source_limits(), LocalSourceLimits::default());
    assert!(resolved.selected_member().is_none());
    assert!(resolved.retained_storage().entry_count() > 0);
    assert!(resolved.retained_storage().entry_count() <= resolved.retained_storage().entry_limit());
    assert!(
        resolved.retained_storage().logical_bytes() <= resolved.retained_storage().byte_limit()
    );
    assert!(
        resolved.retained_storage().maximum_depth() <= resolved.retained_storage().depth_limit()
    );
    let immutable_source = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&resolved.commit).expect("authenticated commit identity"),
        GitTreeId::parse_hex(&resolved.tree).expect("authenticated tree identity"),
    )
    .expect("coherent authenticated Git object format");
    let same_immutable_source = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&resolved.commit).expect("same authenticated commit identity"),
        GitTreeId::parse_hex(&resolved.tree).expect("same authenticated tree identity"),
    )
    .expect("same coherent authenticated Git object format");
    assert_eq!(same_immutable_source, immutable_source);

    let mut mismatched_transport = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_transport.transport_profile = GitTransportProfile::Https;
    assert!(validate_pending_git_request(&mismatched_transport, &request).is_err());

    let mut mismatched_lineage = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_lineage.lineage = GitSourceRequest::new(
        "https://example.invalid/different.git",
        Some(commit.clone()),
    )
    .expect("alternate valid lineage")
    .lineage
    .clone();
    assert!(validate_pending_git_request(&mismatched_lineage, &request).is_err());

    let mut mismatched_limits = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_limits.source_limits.max_entries -= 1;
    assert!(
        validate_pending_git_source_custody(&mismatched_limits, LocalSourceLimits::default())
            .is_err()
    );

    let mut mismatched_format = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_format.object_format = GitObjectIdAlgorithm::Sha256;
    assert!(
        validate_pending_git_source_custody(&mismatched_format, LocalSourceLimits::default())
            .is_err()
    );

    let mut mismatched_snapshot = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_snapshot.snapshot_root = cache.clone();
    assert!(
        validate_pending_git_source_custody(&mismatched_snapshot, LocalSourceLimits::default())
            .is_err()
    );

    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_source_authenticates_and_materializes_empty_subtrees() {
    let (repo, _) = create_git_source("git-empty-subtree");
    let commit = add_empty_tree_commit(&repo);
    let cache = temp_root("git-empty-subtree-cache");

    let resolved = resolve_git_source(
        &local_git_request(&repo, &commit),
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("resolve Git source containing an explicit empty subtree");

    assert_eq!(resolved.commit, commit);
    assert_eq!(resolved.local.file_count, 1);
    assert!(resolved.snapshot_root.join("empty").is_dir());

    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_resolution_issuance_rejects_final_snapshot_drift() {
    let (repo, commit) = create_git_source("git-final-issuance-drift");
    let cache = temp_root("git-final-issuance-drift-cache");
    let resolved = resolve_git_source(
        &local_git_request(&repo, &commit),
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("resolve source before issuance-drift probe");
    let pending = PendingResolvedGitSource::from_issued(&resolved);
    let payload = pending.snapshot_root.join("main.omg");
    make_tree_owner_writable(&pending.snapshot_root);
    std::fs::write(&payload, "machine Drift::main() {}\n")
        .expect("mutate published payload for probe");

    assert!(verify_pending_git_snapshot(&pending, LocalSourceLimits::default()).is_err());

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_source_authenticates_sha256_object_graph() {
    let (repo, commit) = create_git_source_with_format("git-sha256", Some("sha256"));
    let cache = temp_root("git-sha256-cache");

    let resolved = resolve_git_source(
        &local_git_request(&repo, &commit),
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("resolve SHA-256 git source");

    assert_eq!(resolved.commit, commit);
    assert_eq!(resolved.commit.len(), 64);
    assert_eq!(resolved.tree.len(), 64);
    assert_eq!(resolved.local.file_count, 1);

    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_source_discovers_sha256_for_symbolic_revision() {
    let (repo, commit) = create_git_source_with_format("git-sha256-symbolic", Some("sha256"));
    let cache = temp_root("git-sha256-symbolic-cache");

    let resolved = resolve_git_source(
        &local_git_request(&repo, "HEAD"),
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("discover and resolve symbolic SHA-256 git source");

    assert_eq!(resolved.commit, commit);
    assert_eq!(resolved.commit.len(), 64);
    assert_eq!(resolved.tree.len(), 64);

    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_remote_object_format_parser_rejects_absent_malformed_and_mixed_ids() {
    let root = temp_root("git-object-format-parser");
    let sha1 = "11".repeat(20);
    let sha256 = "22".repeat(32);
    assert_eq!(
        parse_git_remote_object_format(
            format!("ref: refs/heads/main\tHEAD\n{sha1}\tHEAD\n").as_bytes(),
            &root,
        )
        .expect("parse SHA-1 remote advertisement"),
        GitObjectIdAlgorithm::Sha1
    );
    assert_eq!(
        parse_git_remote_object_format(
            format!("ref: refs/heads/main\tHEAD\n{sha256}\tHEAD\n").as_bytes(),
            &root,
        )
        .expect("parse SHA-256 remote advertisement"),
        GitObjectIdAlgorithm::Sha256
    );
    for invalid in [
        b"ref: refs/heads/main\tHEAD\n".to_vec(),
        b"not-a-row\n".to_vec(),
        format!("{sha1}\tHEAD\n{sha256}\trefs/heads/main\n").into_bytes(),
    ] {
        assert!(matches!(
            parse_git_remote_object_format(&invalid, &root),
            Err(SourceResolveError::GitCacheInvalid { .. })
                | Err(SourceResolveError::GitObjectInvalid { .. })
        ));
    }
    assert!(!root.exists());
}

#[test]
fn git_tree_authentication_matches_git_prefix_ordering() {
    let (repo, _) = create_git_source("git-prefix-ordering");
    std::fs::create_dir(repo.join("name")).expect("create prefix directory");
    std::fs::write(repo.join("name/child.omg"), "// child\n").expect("write child");
    std::fs::write(repo.join("name.ext"), "// sibling\n").expect("write sibling");
    run_test_git(&repo, ["add", "."]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "exercise tree ordering"]);
    let cache = temp_root("git-prefix-ordering-cache");

    let resolved = resolve_git_source(
        &local_git_request(&repo, "HEAD"),
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("Git tree reconstruction must use canonical directory ordering");

    assert!(resolved.snapshot_root.join("name/child.omg").is_file());
    assert!(resolved.snapshot_root.join("name.ext").is_file());
    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}
