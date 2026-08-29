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
    assert_eq!(resolved.resolution_observation().schema_version(), 4);
    assert_eq!(resolved.resolution_observation().identity().len(), 64);
    assert_eq!(
        resolved.resolution_observation().command_count(),
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
        resolved.resolution_observation().captured_output_ceiling(),
        resolved.captured_output_observation().ceiling()
    );
    assert_eq!(
        resolved.resolution_observation().captured_output_observed(),
        resolved.captured_output_observation().observed()
    );
    assert_eq!(
        resolved.network_transfer_observation().ceiling(),
        git_resolution_network_transfer_ceiling(LocalSourceLimits::default())
    );
    assert_eq!(resolved.network_transfer_observation().observed(), 0);
    assert_eq!(
        resolved.resolution_observation().network_transfer_ceiling(),
        resolved.network_transfer_observation().ceiling()
    );
    assert_eq!(
        resolved
            .resolution_observation()
            .network_transfer_uploaded(),
        resolved.network_transfer_observation().uploaded()
    );
    assert_eq!(
        resolved
            .resolution_observation()
            .network_transfer_downloaded(),
        resolved.network_transfer_observation().downloaded()
    );
    let alternate_policy_result = PendingResolvedGitSource::from_issued(&resolved);
    let alternate_policy_observation = issue_git_source_resolution_observation(
        &alternate_policy_result,
        LocalSourceLimits {
            max_files: LocalSourceLimits::default().max_files - 1,
            ..LocalSourceLimits::default()
        },
    )
    .expect("issue observation for an alternate source policy");
    assert_ne!(
        alternate_policy_observation.identity(),
        resolved.resolution_observation().identity(),
        "the final observation must bind compiler source ceilings"
    );
    let mut unjoined_result = PendingResolvedGitSource::from_issued(&resolved);
    unjoined_result.command_execution_observations.pop();
    assert!(
        issue_git_source_resolution_observation(&unjoined_result, LocalSourceLimits::default())
            .is_err(),
        "final issuance must reject missing command outcome rows"
    );
    let mut unjoined_endpoint = PendingResolvedGitSource::from_issued(&resolved);
    unjoined_endpoint
        .command_execution_observations
        .iter_mut()
        .find(|command| command.endpoint_observation.is_some())
        .expect("resolved fixture retains a network command")
        .endpoint_observation = None;
    assert!(
        issue_git_source_resolution_observation(&unjoined_endpoint, LocalSourceLimits::default())
            .is_err(),
        "final issuance must reject endpoint activity detached from its route policy"
    );
    let mut mismatched_output_accounting = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_output_accounting
        .captured_output_observation
        .observed += 1;
    assert!(
        issue_git_source_resolution_observation(
            &mismatched_output_accounting,
            LocalSourceLimits::default()
        )
        .is_err(),
        "final issuance must reject changed cumulative output accounting"
    );
    let mut mismatched_network_accounting = PendingResolvedGitSource::from_issued(&resolved);
    mismatched_network_accounting
        .network_transfer_observation
        .uploaded += 1;
    assert!(
        issue_git_source_resolution_observation(
            &mismatched_network_accounting,
            LocalSourceLimits::default()
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
                && observation.status_code() == Some(0)
                && observation.termination_signal().is_none()
                && observation.stdout_identity().len() == 64
                && observation.stderr_identity().len() == 64)
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
            .all(|observation| observation.require_strict().is_err()),
        "the current native backend must not overstate strict resolution"
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

#[test]
fn git_source_fetches_only_the_selected_revision_depth() {
    let (repo, _) = create_git_source("git-shallow");
    std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n").expect("change source");
    run_test_git(&repo, ["add", "main.omg"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "second"]);
    let cache = temp_root("git-shallow-cache");
    let request = local_git_request(&repo, "HEAD");

    resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve a shallow exact revision");

    let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
    let output = Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["rev-list", "--count", "FETCH_HEAD"])
        .output()
        .expect("count fetched history");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
    assert!(repository.join("shallow").is_file());

    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_fetch_request_is_depth_one_and_omits_individually_inadmissible_blobs() {
    let arguments = bounded_git_fetch_arguments(
        "https://example.invalid/package.git",
        "0123456789012345678901234567890123456789",
        LocalSourceLimits {
            max_bytes: 4096,
            ..LocalSourceLimits::default()
        },
    );
    let arguments = arguments
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        arguments,
        [
            "fetch",
            "--quiet",
            "--depth=1",
            "--no-tags",
            "--no-recurse-submodules",
            "--filter=blob:limit=4097",
            "--",
            "https://example.invalid/package.git",
            "0123456789012345678901234567890123456789",
        ]
    );
}

#[test]
fn git_fetch_omits_a_blob_above_the_source_byte_ceiling_and_rejects() {
    let (repo, _) = create_git_source("git-filtered-oversized-blob");
    std::fs::write(repo.join("oversized.bin"), vec![0x5a; 4096]).expect("write oversized blob");
    run_test_git(&repo, ["add", "oversized.bin"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "add oversized blob"]);
    run_test_git(&repo, ["config", "uploadpack.allowFilter", "true"]);
    let commit = run_test_git_with_input(&repo, ["rev-parse", "HEAD"], b"");
    let oversized_blob = run_test_git_with_input(&repo, ["rev-parse", "HEAD:oversized.bin"], b"");
    let cache = temp_root("git-filtered-oversized-blob-cache");
    let mut request = local_git_request(&repo, &commit);
    request.fetch_locator = format!("file://{}", repo.display());

    let limits = LocalSourceLimits {
        max_bytes: 1024,
        ..LocalSourceLimits::default()
    }
    .compiler_bounded();
    std::fs::create_dir_all(&cache).expect("create resolver cache");
    let canonical_cache = cache.canonicalize().expect("canonical resolver cache");
    verify_git_cache_root_custody(&canonical_cache).expect("verify resolver cache custody");
    let execution_transport = request.execution_transport();
    let executor = test_system_git_executor(execution_transport).expect("select test Git executor");
    let cache_identity = git_cache_identity(
        request.locator_identity(),
        request.requested_revision(),
        execution_transport,
    );
    let entry_root = canonical_cache.join(format!("git-{cache_identity}"));
    let cache_directory =
        open_absolute_directory_nofollow(&canonical_cache).expect("retain resolver cache");
    let entry_name = entry_root.file_name().expect("cache entry has a name");
    create_git_cache_entry(
        &executor,
        &canonical_cache,
        &cache_directory,
        &entry_root,
        entry_name,
        &cache_identity,
        request.locator_identity(),
        request.fetch_locator(),
        request.requested_revision(),
        execution_transport,
        limits,
    )
    .expect("create quarantined Git cache entry");
    let error = resolve_verified_git_cache_entry(
        &executor,
        &cache_directory,
        entry_name,
        &entry_root,
        request.requested_locator(),
        request.locator_identity(),
        request.fetch_locator(),
        request.requested_revision(),
        execution_transport,
        limits,
        true,
    )
    .expect_err("a required blob above the source ceiling must not be acquired");

    assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
    let repository = entry_root.join(GIT_CACHE_REPOSITORY);
    let output = Command::new("git")
        .env("GIT_NO_LAZY_FETCH", "1")
        .arg("-C")
        .arg(&repository)
        .args(["cat-file", "-e", &oversized_blob])
        .output()
        .expect("inspect quarantined object store");
    assert!(
        !output.status.success(),
        "the inadmissible blob must remain absent from resolver custody"
    );
    assert!(entry_root.join(GIT_CACHE_METADATA).exists());
    assert!(!entry_root.join(GIT_CACHE_SNAPSHOTS).exists());

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn exact_git_revision_reuses_authenticated_objects_without_transport() {
    let (repo, commit) = create_git_source("git-exact-offline-reuse");
    let cache = temp_root("git-exact-offline-reuse-cache");
    let request = local_git_request(&repo, &commit);
    let first = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve exact revision");
    let offline_repo = repo.with_extension("offline");
    std::fs::rename(&repo, &offline_repo).expect("make source transport unavailable");

    let second = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("reuse exact resolver custody without transport");

    assert_eq!(second.commit, first.commit);
    assert_eq!(second.tree, first.tree);
    assert_eq!(second.snapshot_root, first.snapshot_root);
    assert_eq!(second.local, first.local);

    let _ = std::fs::remove_dir_all(&offline_repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn exact_git_revision_offline_reuse_still_enforces_source_limits() {
    let (repo, commit) = create_git_source("git-exact-offline-limits");
    let cache = temp_root("git-exact-offline-limits-cache");
    let request = local_git_request(&repo, &commit);
    resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve exact revision");
    let offline_repo = repo.with_extension("offline");
    std::fs::rename(&repo, &offline_repo).expect("make source transport unavailable");

    let error = resolve_git_source(
        &request,
        &cache,
        LocalSourceLimits {
            max_bytes: 0,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("cached exact source must remain subject to current limits");

    assert_eq!(error, SourceResolveError::TooManyBytes { limit: 0 });

    let _ = std::fs::remove_dir_all(&offline_repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn symbolic_git_revision_still_refetches_and_observes_movement() {
    let (repo, first_commit) = create_git_source("git-symbolic-refresh");
    let cache = temp_root("git-symbolic-refresh-cache");
    let request = local_git_request(&repo, "HEAD");
    let first = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve initial symbolic revision");
    assert_eq!(first.commit, first_commit);

    std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n").expect("change source");
    run_test_git(&repo, ["add", "main.omg"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "move symbolic revision"]);
    let second_commit = run_test_git_with_input(&repo, ["rev-parse", "HEAD"], b"");

    let second = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("refresh symbolic revision");

    assert_eq!(second.commit, second_commit);
    assert_ne!(second.commit, first.commit);
    assert_eq!(
        std::fs::read(second.snapshot_root.join("main.omg")).expect("read refreshed source"),
        b"machine Main::changed() {}\n"
    );

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_tree_rejects_traversal_metadata_and_nonportable_paths_before_materialization() {
    let repository = temp_root("git-tree-path-validation");
    let oid = "0123456789012345678901234567890123456789";
    for path in [
        b"../escape.omg".as_slice(),
        b"nested/../../escape.omg".as_slice(),
        b"/absolute.omg".as_slice(),
        b"nested\\ambiguous.omg".as_slice(),
        b"nested/.git/config".as_slice(),
        b"C:/drive-prefixed.omg".as_slice(),
        b"nested/NUL.txt".as_slice(),
        b"aux.omg".as_slice(),
        b"name:stream.omg".as_slice(),
        b"trailing.".as_slice(),
        b"trailing ".as_slice(),
        b"question?.omg".as_slice(),
    ] {
        let mut listing = format!("100644 blob {oid} 1\t").into_bytes();
        listing.extend_from_slice(path);
        listing.push(0);
        let error = parse_git_tree_entries(&listing, &repository, LocalSourceLimits::default())
            .expect_err("unsafe Git path must reject");
        assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
    }
    assert!(
        !repository.exists(),
        "validation must not create a staging path"
    );
}

#[test]
fn git_symlink_targets_reject_windows_ambiguous_spellings() {
    for target in [
        b"C:/escape.omg".as_slice(),
        b"NUL".as_slice(),
        b"nested/COM1.log".as_slice(),
        b"name:stream".as_slice(),
        b"trailing.".as_slice(),
    ] {
        assert!(matches!(
            validate_git_symlink_target(b"link", target),
            Err(SourceResolveError::GitTreeInvalid { .. })
        ));
    }
}

#[test]
fn git_tree_enforces_declared_limits_before_reading_blobs() {
    let repository = temp_root("git-tree-limit-validation");
    let oid = "0123456789012345678901234567890123456789";
    let listing = format!("100644 blob {oid} 4\tmain.omg\0");

    let error = parse_git_tree_entries(
        listing.as_bytes(),
        &repository,
        LocalSourceLimits {
            max_bytes: 3,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("oversized tree must reject from metadata");

    assert_eq!(error, SourceResolveError::TooManyBytes { limit: 3 });
    assert!(
        !repository.exists(),
        "limit rejection must not inspect an object"
    );
}

#[test]
fn git_tree_entry_limit_counts_declared_directories() {
    let repository = temp_root("git-tree-directory-limit");
    let oid = "0123456789012345678901234567890123456789";
    let listing = format!("040000 tree {oid} -\tnested\0100644 blob {oid} 0\tnested/main.omg\0");

    let error = parse_git_tree_entries(
        listing.as_bytes(),
        &repository,
        LocalSourceLimits {
            max_files: 1,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("directory and blob must consume separate identity entries");

    assert_eq!(error, SourceResolveError::TooManyFiles { limit: 1 });
    assert!(!repository.exists());
}

#[test]
fn git_tree_rejects_gitlinks_before_materialization() {
    let repository = temp_root("gitlink-validation");
    let oid = "0123456789012345678901234567890123456789";
    let listing = format!("160000 commit {oid} -\tdependency\0");

    let error = parse_git_tree_entries(
        listing.as_bytes(),
        &repository,
        LocalSourceLimits::default(),
    )
    .expect_err("gitlink must reject");

    assert!(matches!(
        error,
        SourceResolveError::GitSubmodulesUnsupported { .. }
    ));
    assert!(!repository.exists());
}

#[cfg(unix)]
#[test]
fn git_snapshot_preserves_paths_executable_modes_and_symlink_spelling() {
    use std::os::unix::fs::PermissionsExt;

    let (repo, _) = create_git_source("git-snapshot-kinds");
    let script = repo.join("tools/generate");
    std::fs::create_dir_all(script.parent().expect("script parent")).expect("create tools");
    std::fs::write(&script, "#!/bin/sh\n").expect("write script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("mark script executable");
    std::os::unix::fs::symlink("generate", repo.join("tools/current"))
        .expect("create source symlink");
    run_test_git(&repo, ["add", "tools/generate", "tools/current"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "add exact entry kinds"]);
    let cache = temp_root("git-snapshot-kinds-cache");
    let request = local_git_request(&repo, "HEAD");

    let resolved =
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("resolve kinds");
    let published_script = resolved.snapshot_root.join("tools/generate");
    let published_link = resolved.snapshot_root.join("tools/current");

    assert_eq!(
        std::fs::read(&published_script).expect("read script"),
        b"#!/bin/sh\n"
    );
    assert_ne!(
        std::fs::metadata(&published_script)
            .expect("script metadata")
            .permissions()
            .mode()
            & 0o111,
        0
    );
    assert_eq!(
        raw_os_bytes(
            std::fs::read_link(&published_link)
                .expect("read published symlink")
                .as_os_str()
        ),
        b"generate"
    );
    assert_eq!(resolved.local.file_count, 3);
    assert_eq!(
        std::fs::metadata(resolved.snapshot_root.join("tools"))
            .expect("nested directory metadata")
            .permissions()
            .mode()
            & 0o7777,
        u32::from(CANONICAL_DIRECTORY_MODE)
    );
    let verified = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("verify nested snapshot reuse");
    assert_eq!(resolved.local, verified.local);

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_snapshot_uses_blob_bytes_not_checkout_attribute_conversions() {
    let (repo, _) = create_git_source("git-snapshot-attributes");
    std::fs::write(repo.join(".gitattributes"), "*.omg eol=crlf\n")
        .expect("write checkout conversion attribute");
    run_test_git(&repo, ["add", ".gitattributes"]);
    run_test_git(
        &repo,
        ["commit", "--quiet", "-m", "add checkout conversion"],
    );
    let cache = temp_root("git-snapshot-attributes-cache");
    let request = local_git_request(&repo, "HEAD");

    let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("materialize object bytes");

    assert_eq!(
        std::fs::read(resolved.snapshot_root.join("main.omg")).expect("read snapshot blob"),
        b"machine Main::main() {}\n"
    );
    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_snapshot_reuse_rejects_content_with_forged_matching_metadata() {
    use std::os::unix::fs::PermissionsExt;

    let (repo, _) = create_git_source("git-snapshot-reuse");
    let cache = temp_root("git-snapshot-reuse-cache");
    let request = local_git_request(&repo, "HEAD");
    let first =
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("first resolve");
    let second =
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("reuse snapshot");
    assert_eq!(first.snapshot_root, second.snapshot_root);
    assert_eq!(first.local, second.local);

    std::fs::set_permissions(&first.snapshot_root, std::fs::Permissions::from_mode(0o755))
        .expect("make source root writable for tamper simulation");
    let source = first.snapshot_root.join("main.omg");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
        .expect("make source writable for tamper simulation");
    std::fs::write(&source, "machine Tampered::main() {}\n").expect("tamper snapshot");
    let publication = first
        .snapshot_root
        .parent()
        .expect("snapshot source has a publication parent");
    let metadata_path = publication.join(GIT_SNAPSHOT_METADATA);
    std::fs::set_permissions(&metadata_path, std::fs::Permissions::from_mode(0o644))
        .expect("make snapshot metadata writable for tamper simulation");
    let forged = resolve_materialized_source(&first.snapshot_root, LocalSourceLimits::default())
        .expect("derive the public identity an attacker could recompute");
    std::fs::write(&metadata_path, git_snapshot_metadata(&first.tree, &forged))
        .expect("forge matching snapshot metadata");
    make_snapshot_read_only(publication).expect("restore canonical snapshot modes");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("tampered snapshot and matching forged metadata must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    let entry = git_cache_entry_root(&cache, &request);
    assert!(!entry.join(GIT_CACHE_METADATA).exists());

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_batch_failure_precedes_snapshot_staging() {
    let (repo, _) = create_git_source("git-snapshot-cleanup");
    let cache = temp_root("git-snapshot-cleanup-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry_root = git_cache_entry_root(&cache, &request);
    let repository = entry_root.join(GIT_CACHE_REPOSITORY);
    let missing_oid = "0000000000000000000000000000000000000000";
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");
    let mut entries = vec![GitTreeEntry {
        relative_bytes: b"missing.omg".to_vec(),
        relative_path: PathBuf::from("missing.omg"),
        oid: missing_oid.to_owned(),
        size: 1,
        kind: GitTreeEntryKind::File {
            executable: false,
            bytes: GitBlobBytes::empty(),
        },
    }];
    let error = read_git_blobs_batch_from_path(
        &executor,
        &repository,
        &mut entries,
        LocalSourceLimits::default(),
    )
    .expect_err("missing object must fail before staged materialization");
    assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
    let snapshots = entry_root.join(GIT_CACHE_SNAPSHOTS);
    assert!(
        std::fs::read_dir(&snapshots)
            .expect("read snapshots")
            .all(|entry| !entry
                .expect("snapshot entry")
                .file_name()
                .to_string_lossy()
                .contains(".stage-")),
        "failed materialization must leave no staging directory"
    );
    assert!(
        !snapshots
            .join("tree-1111111111111111111111111111111111111111")
            .exists()
    );

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_snapshot_excludes_untracked_source_worktree_state() {
    let (repo, _) = create_git_source("git-untracked-source");
    let cache = temp_root("git-untracked-cache");
    std::fs::write(repo.join("injected.omg"), "machine Injected::main() {}\n")
        .expect("write untracked source state");
    let request = local_git_request(&repo, "HEAD");
    let resolved =
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");

    assert!(!resolved.snapshot_root.join("injected.omg").exists());
    assert_eq!(resolved.local.file_count, 1);
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_source_rejects_submodule_manifest() {
    let (repo, commit) = create_git_source("git-submodule");
    let cache = temp_root("git-submodule-cache");
    let request = local_git_request(&repo, "HEAD");
    let initial = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve initial source");
    let snapshot_source = initial.snapshot_root.join("main.omg");
    let initial_snapshot = std::fs::read(&snapshot_source).expect("read initial snapshot");

    std::fs::write(repo.join(".gitmodules"), "[submodule \"dep\"]\n").expect("write gitmodules");
    std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n").expect("change source");
    run_test_git(&repo, ["add", ".gitmodules"]);
    run_test_git(&repo, ["add", "main.omg"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "submodule manifest"]);

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("submodule manifest should reject");

    assert!(matches!(
        error,
        SourceResolveError::GitSubmodulesUnsupported { .. }
    ));
    assert_eq!(
        std::fs::read(&snapshot_source).expect("read snapshot after rejection"),
        initial_snapshot,
        "the fetched submodule tree must be rejected before materialization"
    );
    assert!(!initial.snapshot_root.join("../source.identity").exists());
    assert!(!commit.is_empty());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}
