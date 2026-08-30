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
    assert!(!resolved.execution_policy_observations().is_empty());
    assert_eq!(
        resolved.command_execution_observations().len(),
        resolved.execution_policy_observations().len()
    );
    assert_eq!(resolved.receipt().schema_version(), 8);
    assert_eq!(resolved.receipt().identity().len(), 64);
    assert_eq!(
        resolved.receipt().command_count(),
        resolved.command_execution_observations().len()
    );
    assert_eq!(
        resolved.captured_output_observation().ceiling(),
        git_resolution_captured_output_ceiling(LocalSourceLimits::default())
    );
    assert_eq!(
        resolved.captured_output_observation().observed(),
        resolved
            .command_execution_observations()
            .iter()
            .map(|command| command.stdout_length() + command.stderr_length())
            .sum::<u64>()
    );
    assert_eq!(
        resolved.receipt().captured_output_ceiling(),
        resolved.captured_output_observation().ceiling()
    );
    assert_eq!(
        resolved.receipt().captured_output_observed(),
        resolved.captured_output_observation().observed()
    );
    assert_eq!(
        resolved.network_transfer_observation().ceiling(),
        git_resolution_network_transfer_ceiling(LocalSourceLimits::default())
    );
    assert_eq!(resolved.network_transfer_observation().observed(), 0);
    assert_eq!(
        resolved.receipt().network_transfer_ceiling(),
        resolved.network_transfer_observation().ceiling()
    );
    assert_eq!(
        resolved.receipt().network_transfer_uploaded(),
        resolved.network_transfer_observation().uploaded()
    );
    assert_eq!(
        resolved.receipt().network_transfer_downloaded(),
        resolved.network_transfer_observation().downloaded()
    );
    assert_eq!(resolved.retained_storage_observation().schema_version(), 1);
    assert_eq!(resolved.retained_storage_observation().identity().len(), 64);
    assert!(resolved.retained_storage_observation().entry_count() > 0);
    assert!(
        resolved.retained_storage_observation().entry_count()
            <= resolved.retained_storage_observation().entry_ceiling()
    );
    assert!(
        resolved.retained_storage_observation().logical_bytes()
            <= resolved.retained_storage_observation().byte_ceiling()
    );
    assert!(
        resolved.retained_storage_observation().maximum_depth()
            <= resolved.retained_storage_observation().depth_ceiling()
    );
    let alternate_policy_result = PendingResolvedGitSource::from_issued(&resolved);
    let alternate_policy_observation = issue_git_source_receipt(
        &alternate_policy_result,
        LocalSourceLimits {
            max_entries: LocalSourceLimits::default().max_entries - 1,
            ..LocalSourceLimits::default()
        },
        resolved.retained_storage_observation(),
    )
    .expect("issue observation for an alternate source policy");
    assert_ne!(
        alternate_policy_observation.identity(),
        resolved.receipt().identity(),
        "the final observation must bind compiler source ceilings"
    );
    let mut unjoined_result = PendingResolvedGitSource::from_issued(&resolved);
    unjoined_result.command_execution_observations.pop();
    assert!(
        issue_git_source_receipt(
            &unjoined_result,
            LocalSourceLimits::default(),
            resolved.retained_storage_observation(),
        )
        .is_err(),
        "final issuance must reject missing command outcome rows"
    );
    let mut mismatched_completion = PendingResolvedGitSource::from_issued(&resolved);
    let alternate_completion = mismatched_completion.command_execution_observations[1]
        .completion
        .clone();
    mismatched_completion.command_execution_observations[0].completion = alternate_completion;
    assert!(
        issue_git_source_receipt(
            &mismatched_completion,
            LocalSourceLimits::default(),
            resolved.retained_storage_observation(),
        )
        .is_err(),
        "final issuance must reject a completion detached from its command policy"
    );
    let mut unjoined_endpoint = PendingResolvedGitSource::from_issued(&resolved);
    unjoined_endpoint
        .command_execution_observations
        .iter_mut()
        .find(|command| command.endpoint_observation.is_some())
        .expect("resolved fixture retains a network command")
        .endpoint_observation = None;
    assert!(
        issue_git_source_receipt(
            &unjoined_endpoint,
            LocalSourceLimits::default(),
            resolved.retained_storage_observation(),
        )
        .is_err(),
        "final issuance must reject endpoint activity detached from its route policy"
    );
    let mut mismatched_output_accounting = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_output_accounting
        .captured_output_observation
        .observed += 1;
    assert!(
        issue_git_source_receipt(
            &mismatched_output_accounting,
            LocalSourceLimits::default(),
            resolved.retained_storage_observation(),
        )
        .is_err(),
        "final issuance must reject changed cumulative output accounting"
    );
    let mut mismatched_network_accounting = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_network_accounting
        .network_transfer_observation
        .uploaded += 1;
    assert!(
        issue_git_source_receipt(
            &mismatched_network_accounting,
            LocalSourceLimits::default(),
            resolved.retained_storage_observation(),
        )
        .is_err(),
        "final issuance must reject changed network-transfer accounting"
    );
    let mut mismatched_transport = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_transport.transport_profile = GitTransportProfile::Https;
    assert!(validate_pending_git_request(&mismatched_transport, &request).is_err());
    assert!(
        resolved
            .command_execution_observations()
            .iter()
            .all(|observation| observation.policy_identity().len() == 64
                && observation.command_identity().len() == 64
                && observation.completion().policy().phase() == observation.phase()
                && observation.completion().status().success()
                && !observation.completion().canonical_bytes().is_empty()
                && observation.status_code() == Some(0)
                && observation.termination_signal().is_none()
                && observation.stdout_identity().len() == 64
                && observation.stderr_identity().len() == 64)
    );
    assert!(
        resolved
            .command_execution_observations()
            .iter()
            .any(|observation| matches!(
                observation.input(),
                GitCommandInputCommitment::ExactBytes { .. }
            )),
        "object-batch commands must retain exact input custody"
    );
    assert!(
        resolved
            .execution_policy_observations()
            .iter()
            .all(|observation| observation.executable() == resolved.git_executable.path())
    );
    assert!(
        resolved
            .execution_policy_observations()
            .iter()
            .any(
                |observation| observation.phase() == ResolverExecutionPhase::Fetch
                    && observation.mutable_root().is_some()
            )
    );
    assert!(
        resolved
            .execution_policy_observations()
            .iter()
            .any(
                |observation| observation.phase() == ResolverExecutionPhase::RepositoryInspection
                    && observation.mutable_root().is_none()
            )
    );
    #[cfg(target_os = "macos")]
    {
        assert_eq!(resolved.execution_helper_executables().len(), 5);
        assert!(
            resolved
                .execution_helper_executables()
                .iter()
                .all(|executable| executable.content_identity().len() == 64)
        );
    }

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
    let mut permissions = std::fs::metadata(&payload)
        .expect("stat published payload")
        .permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(&payload, permissions).expect("make payload mutable for probe");
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
