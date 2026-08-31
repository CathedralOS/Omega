use super::{
    authentication::*, batch::*, identity::*, inspection::*, model::*, projection::*, tree::*,
};
use crate::error::SourceResolveError;
use crate::git::commands::invocation::*;
use crate::git::executable::executor::test_system_git_executor;
use crate::git::request::GitExecutionTransport;
use crate::git::snapshot::preflight_git_snapshot;
use crate::identity::GitObjectIdAlgorithm;
use crate::limits::*;
use crate::snapshot::permissions::make_tree_owner_writable;
use crate::test_support::*;
use omega_resolver_execution::ResolverExecutionPhase;
use sha1_checked::Sha1 as CheckedSha1;
use sha2::Digest;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[test]
fn git_blob_batch_uses_one_bounded_launch_for_many_files() {
    let (repo, _) = create_git_source("batched-blobs");
    for index in 0..32 {
        std::fs::write(
            repo.join(format!("source-{index}.omg")),
            format!("// {index}\n"),
        )
        .expect("write batched source");
    }
    run_test_git(&repo, ["add", "."]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "add batched sources"]);
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");
    let tree = run_git_stdout(
        &executor,
        &repo,
        ResolverExecutionPhase::RepositoryInspection,
        [OsStr::new("rev-parse"), OsStr::new("HEAD^{tree}")],
    )
    .expect("resolve tree");
    let listing = run_git_bytes_stdout(
        &executor,
        &repo,
        ResolverExecutionPhase::RepositoryInspection,
        [
            OsStr::new("ls-tree"),
            OsStr::new("--full-tree"),
            OsStr::new("-r"),
            OsStr::new("-t"),
            OsStr::new("-l"),
            OsStr::new("-z"),
            OsStr::new(tree.trim()),
        ],
    )
    .expect("list tree");
    let mut entries = parse_git_tree_entries(
        &listing,
        &repo,
        LocalSourceLimits {
            max_entries: 10_000,
            ..LocalSourceLimits::default()
        },
    )
    .expect("parse tree");
    let launches_before = executor.launches.get();
    let captured_before = executor.captured_output_budget.observed();
    read_git_blobs_batch_from_path(&executor, &repo, &mut entries, LocalSourceLimits::default())
        .expect("read all blobs in one batch");

    assert_eq!(entries.len(), 33);
    assert_eq!(executor.launches.get() - launches_before, 1);
    assert!(executor.captured_output_budget.observed() > captured_before);
    assert!(executor.captured_output_budget.observed() <= executor.captured_output_budget.ceiling);
    assert_eq!(executor.maximum_launches, GIT_FIXED_COMMAND_ALLOWANCE);
    assert!(entries.iter().any(|entry| {
        entry.relative_bytes == b"main.omg"
            && matches!(
                &entry.kind,
                GitTreeEntryKind::File { bytes, .. }
                    if bytes.as_slice() == b"machine Main::main() {}\n"
            )
    }));

    let oversized = GitTreeEntry {
        relative_bytes: b"oversized.omg".to_vec(),
        relative_path: PathBuf::from("oversized.omg"),
        oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        size: 2,
        kind: GitTreeEntryKind::File {
            executable: false,
            bytes: GitBlobBytes::empty(),
        },
    };
    let error = git_batch_output_limit(
        &[oversized],
        LocalSourceLimits {
            max_bytes: 1,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("aggregate batch payload must honor the source byte ceiling");
    assert!(matches!(
        error,
        SourceResolveError::TooManyBytes { limit: 1 }
    ));

    let _ = std::fs::remove_dir_all(&repo);
}
#[test]
fn git_batch_request_creation_and_cleanup_remain_in_the_retained_entry() {
    let (repo, _) = create_git_source("retained-batch-request-source");
    let cache = temp_root("retained-batch-request-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let verified = open_verified_git_repository(&cache, &request);
    let entry = verified.entry_root.clone();
    let displaced = entry.with_file_name("entry.displaced");
    std::fs::rename(&entry, &displaced).expect("displace retained entry");
    std::fs::create_dir(&entry).expect("create replacement entry");

    let mut batch = PendingGitBatchRequest::create(&verified.entry, &verified.entry_root)
        .expect("create request through retained entry");
    let name = batch.name.clone();
    assert!(displaced.join(&name).is_file());
    assert!(!entry.join(&name).exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            std::fs::metadata(displaced.join(&name))
                .expect("read batch request mode")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    batch.remove().expect("remove retained batch request");
    assert!(!displaced.join(&name).exists());

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_batch_request_cleanup_does_not_remove_a_replacement_name() {
    let (repo, _) = create_git_source("replaced-batch-request-source");
    let cache = temp_root("replaced-batch-request-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let verified = open_verified_git_repository(&cache, &request);
    let batch = PendingGitBatchRequest::create(&verified.entry, &verified.entry_root)
        .expect("create retained batch request");
    let path = batch.display_path.clone();
    let displaced = path.with_extension("displaced");
    std::fs::rename(&path, &displaced).expect("displace batch request");
    std::fs::write(&path, b"replacement").expect("install replacement request name");

    drop(batch);
    assert_eq!(
        std::fs::read(&path).expect("read replacement request"),
        b"replacement"
    );

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_blob_batch_parser_binds_order_type_size_and_framing() {
    fn entry(oid: char, path: &str, size: u64, symlink: bool) -> GitTreeEntry {
        GitTreeEntry {
            relative_bytes: path.as_bytes().to_vec(),
            relative_path: PathBuf::from(path),
            oid: std::iter::repeat_n(oid, 40).collect(),
            size,
            kind: if symlink {
                GitTreeEntryKind::Symlink {
                    target_bytes: GitBlobBytes::empty(),
                }
            } else {
                GitTreeEntryKind::File {
                    executable: false,
                    bytes: GitBlobBytes::empty(),
                }
            },
        }
    }

    let first_oid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let second_oid = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let mut entries = vec![
        entry('a', "binary.omg", 3, false),
        entry('b', "link", 6, true),
    ];
    let mut valid = format!("{first_oid} blob 3\n").into_bytes();
    valid.extend_from_slice(&[0, 255, b'\n']);
    valid.push(b'\n');
    valid.extend_from_slice(format!("{second_oid} blob 6\n").as_bytes());
    valid.extend_from_slice(b"target\n");
    assign_git_batch_output(&mut entries, valid).expect("parse exact batch response");
    assert!(matches!(
        &entries[0].kind,
        GitTreeEntryKind::File { bytes, .. } if bytes.as_slice() == &[0, 255, b'\n']
    ));
    assert!(matches!(
        &entries[1].kind,
        GitTreeEntryKind::Symlink { target_bytes } if target_bytes.as_slice() == b"target"
    ));

    for malformed in [
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab blob 3\nabc\n".as_slice(),
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa tree 3\nabc\n".as_slice(),
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 4\nabc\n".as_slice(),
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 3".as_slice(),
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 3\nab".as_slice(),
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 3\nabc".as_slice(),
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 3\nabc\nextra".as_slice(),
    ] {
        let mut one = vec![entry('a', "file.omg", 3, false)];
        assert!(matches!(
            assign_git_batch_output(&mut one, malformed.to_vec()),
            Err(SourceResolveError::GitTreeInvalid { .. })
        ));
    }

    let mut escaping_link = vec![entry('a', "link", 2, true)];
    let response = format!("{first_oid} blob 2\n..\n");
    assert!(matches!(
        assign_git_batch_output(&mut escaping_link, response.into_bytes()),
        Err(SourceResolveError::GitTreeInvalid { .. })
    ));
}

fn authenticated_file_entry(oid: &str, path: &str, payload: &[u8]) -> GitTreeEntry {
    GitTreeEntry {
        relative_bytes: path.as_bytes().to_vec(),
        relative_path: PathBuf::from(path),
        oid: oid.to_owned(),
        size: payload.len() as u64,
        kind: GitTreeEntryKind::File {
            executable: false,
            bytes: GitBlobBytes {
                batch: Arc::new(payload.to_vec()),
                start: 0,
                end: payload.len(),
            },
        },
    }
}

#[test]
fn git_object_authentication_accepts_fixed_sha1_and_sha256_graphs() {
    for (algorithm, blob, tree, commit) in [
        (
            GitObjectIdAlgorithm::Sha1,
            "ce013625030ba8dba906f756967f9e9ca394464a",
            "6e3b5fe3c2f6b56c4d150929f0df706a5356004a",
            "63338d8e114523a7087c391b234d776baae7af51",
        ),
        (
            GitObjectIdAlgorithm::Sha256,
            "2cf8d83d9ee29543b34a87727421fdecb7e3f3a183d337639025de576db9ebb4",
            "2ff2fdf5e33d610f8013a2eba140fd1660dac0491d9cac96ac024c5789c44e07",
            "5145c89465c4d7f1ab705bb9e032ef1a9ac879a5e137733bdab3b1d6cd354ff7",
        ),
    ] {
        assert_eq!(
            git_object_identity(b"blob", b"hello\n", algorithm).expect("hash fixed Git object"),
            blob
        );
        authenticate_git_tree(
            tree,
            &[authenticated_file_entry(blob, "main.omg", b"hello\n")],
        )
        .expect("fixed authenticated tree graph");
        let commit_payload = format!("tree {tree}\n\n");
        authenticate_git_commit_payload(commit, tree, commit_payload.as_bytes())
            .expect("fixed authenticated commit graph");
    }
}

#[test]
fn git_tree_graph_authenticates_before_blob_payloads_are_opened() {
    let blob = "ce013625030ba8dba906f756967f9e9ca394464a";
    let tree = "6e3b5fe3c2f6b56c4d150929f0df706a5356004a";
    let entry = authenticated_file_entry(blob, "main.omg", b"");

    authenticate_git_tree_graph(tree, std::slice::from_ref(&entry))
        .expect("tree graph uses the authenticated blob edge without opening its payload");
    assert!(matches!(
        authenticate_git_tree_payloads(tree, &[entry]),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));
}

#[test]
fn selective_git_member_opens_only_authenticated_declarations_and_member_payloads() {
    let (repo, _) = create_git_source("selective-member-payloads");
    std::fs::create_dir_all(repo.join("packages/member/src")).expect("create member source");
    std::fs::create_dir_all(repo.join("packages/member/tools")).expect("create member tools");
    std::fs::write(repo.join("build.omg"), b"root declaration\n").expect("write root declaration");
    std::fs::write(
        repo.join("packages/member/build.omg"),
        b"member declaration\n",
    )
    .expect("write member declaration");
    std::fs::write(
        repo.join("packages/member/src/lib.omg"),
        b"machine Member::value() {}\n",
    )
    .expect("write member source");
    std::fs::write(
        repo.join("packages/member/tools/generate"),
        b"#!/bin/sh\nexit 0\n",
    )
    .expect("write member executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let mut permissions = std::fs::metadata(repo.join("packages/member/tools/generate"))
            .expect("read executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(repo.join("packages/member/tools/generate"), permissions)
            .expect("make member tool executable");
        symlink("src/lib.omg", repo.join("packages/member/current"))
            .expect("create member symlink");
    }
    std::fs::write(repo.join("unrelated.bin"), vec![b'x'; 4096])
        .expect("write unrelated oversized blob");
    run_test_git(&repo, ["add", "."]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "add workspace member"]);

    let cache = temp_root("selective-member-payloads-cache");
    let request = local_git_request(&repo, "HEAD");
    let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("prime authenticated repository cache");
    let tree = resolved.tree().to_owned();
    let repository = open_verified_git_repository(&cache, &request);
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");
    let projection = inspect_git_tree_projection(
        &executor,
        &repository,
        &tree,
        &GitTreeProjectionRequest::new(
            [b"build.omg".to_vec(), b"packages/member/build.omg".to_vec()],
            b"packages/member".to_vec(),
        ),
        LocalSourceLimits {
            max_bytes: 1024,
            ..LocalSourceLimits::default()
        },
    )
    .expect("open only selected payloads beneath the package byte ceiling");

    assert_eq!(projection.repository_tree_oid(), tree);
    assert_eq!(projection.member().source_path(), b"packages/member");
    assert_eq!(
        projection.member().tree_oid(),
        run_test_git_with_input(&repo, ["rev-parse", "HEAD:packages/member"], b"")
    );
    assert_eq!(projection.declarations().len(), 2);
    assert!(matches!(
        &projection.declarations()[0].kind,
        GitTreeEntryKind::File { bytes, .. } if bytes.as_slice() == b"root declaration\n"
    ));
    assert!(projection.member().entries().iter().all(|entry| {
        !entry.relative_bytes.starts_with(b"packages/member")
            && entry.relative_bytes != b"unrelated.bin"
    }));
    assert!(projection.member().entries().iter().any(|entry| {
        entry.relative_bytes == b"src/lib.omg"
            && matches!(
                &entry.kind,
                GitTreeEntryKind::File { bytes, .. }
                    if bytes.as_slice() == b"machine Member::value() {}\n"
            )
    }));
    #[cfg(unix)]
    {
        assert!(projection.member().entries().iter().any(|entry| {
            entry.relative_bytes == b"tools/generate"
                && matches!(
                    &entry.kind,
                    GitTreeEntryKind::File {
                        executable: true,
                        ..
                    }
                )
        }));
        assert!(projection.member().entries().iter().any(|entry| {
            entry.relative_bytes == b"current"
                && matches!(
                    &entry.kind,
                    GitTreeEntryKind::Symlink { target_bytes }
                        if target_bytes.as_slice() == b"src/lib.omg"
                )
        }));
    }

    drop(repository);
    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn selective_git_projection_rejects_duplicate_missing_and_wrong_type_paths() {
    let (repo, _) = create_git_source("selective-member-path-errors");
    std::fs::create_dir_all(repo.join("member/empty")).expect("create member directories");
    std::fs::write(repo.join("member/build.omg"), b"declaration\n").expect("write declaration");
    #[cfg(unix)]
    std::os::unix::fs::symlink("build.omg", repo.join("member/declaration-link"))
        .expect("create declaration symlink");
    run_test_git(&repo, ["add", "."]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "add member paths"]);

    let cache = temp_root("selective-member-path-errors-cache");
    let request = local_git_request(&repo, "HEAD");
    let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("prime authenticated repository cache");
    let tree = resolved.tree().to_owned();
    let repository = open_verified_git_repository(&cache, &request);
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");

    for (projection_request, expected_message) in [
        (
            GitTreeProjectionRequest::new(
                [b"main.omg".to_vec(), b"main.omg".to_vec()],
                b"member".to_vec(),
            ),
            "requested more than once",
        ),
        (
            GitTreeProjectionRequest::new([b"absent.omg".to_vec()], b"member".to_vec()),
            "declaration path is absent",
        ),
        (
            GitTreeProjectionRequest::new([b"member".to_vec()], b"member".to_vec()),
            "declaration path is not a regular file",
        ),
        (
            GitTreeProjectionRequest::new([b"main.omg".to_vec()], b"absent".to_vec()),
            "member tree is absent",
        ),
        (
            GitTreeProjectionRequest::new([b"main.omg".to_vec()], b"main.omg".to_vec()),
            "member root is not a tree",
        ),
    ] {
        let error = inspect_git_tree_projection(
            &executor,
            &repository,
            &tree,
            &projection_request,
            LocalSourceLimits::default(),
        )
        .expect_err("invalid exact projection path must fail closed");
        assert!(matches!(
            error,
            SourceResolveError::GitTreeInvalid { ref message, .. }
                if message.contains(expected_message)
        ));
    }

    #[cfg(unix)]
    for projection_request in [
        GitTreeProjectionRequest::new([b"member/declaration-link".to_vec()], b"member".to_vec()),
        GitTreeProjectionRequest::new([b"main.omg".to_vec()], b"member/declaration-link".to_vec()),
    ] {
        assert!(matches!(
            inspect_git_tree_projection(
                &executor,
                &repository,
                &tree,
                &projection_request,
                LocalSourceLimits::default(),
            ),
            Err(SourceResolveError::GitTreeInvalid { .. })
        ));
    }

    drop(repository);
    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn selective_git_projection_authenticates_an_empty_member_tree() {
    let (repo, _) = create_git_source("selective-empty-member");
    let revision = add_empty_tree_commit(&repo);
    let cache = temp_root("selective-empty-member-cache");
    let request = local_git_request(&repo, &revision);
    let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("prime authenticated empty-tree repository cache");
    let repository = open_verified_git_repository(&cache, &request);
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");
    let projection = inspect_git_tree_projection(
        &executor,
        &repository,
        resolved.tree(),
        &GitTreeProjectionRequest::new([b"main.omg".to_vec()], b"empty".to_vec()),
        LocalSourceLimits {
            max_entries: 1,
            max_bytes: 64,
            max_depth: 0,
        },
    )
    .expect("authenticate and project an empty member tree");

    assert_eq!(
        projection.member().tree_oid(),
        "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
    );
    assert!(projection.member().entries().is_empty());

    drop(repository);
    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn selective_git_graph_rejects_omitted_forged_and_duplicate_edges() {
    let blob = "ce013625030ba8dba906f756967f9e9ca394464a";
    let tree = "6e3b5fe3c2f6b56c4d150929f0df706a5356004a";
    assert!(matches!(
        authenticate_git_tree_graph(tree, &[]),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));
    assert!(matches!(
        authenticate_git_tree_graph(
            tree,
            &[authenticated_file_entry(
                "1111111111111111111111111111111111111111",
                "main.omg",
                b""
            )]
        ),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));

    let record = format!("100644 blob {blob} 6\tmain.omg\0");
    let listing = [record.as_bytes(), record.as_bytes()].concat();
    assert!(matches!(
        parse_git_tree_entries(
            &listing,
            Path::new("duplicate-listing.git"),
            LocalSourceLimits::default()
        ),
        Err(SourceResolveError::GitTreeInvalid { .. })
    ));
}

#[test]
fn git_object_authentication_rejects_mismatched_bytes_and_edges() {
    let blob = "ce013625030ba8dba906f756967f9e9ca394464a";
    let tree = "6e3b5fe3c2f6b56c4d150929f0df706a5356004a";
    let commit = "63338d8e114523a7087c391b234d776baae7af51";
    assert!(matches!(
        verify_git_object_identity(blob, b"blob", b"tampered\n", GitObjectIdAlgorithm::Sha1),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));

    let commit_payload = format!("tree {tree}\n\n");
    assert!(matches!(
        authenticate_git_commit_payload(
            "0000000000000000000000000000000000000000",
            tree,
            commit_payload.as_bytes()
        ),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));
    assert!(matches!(
        authenticate_git_commit_payload(
            commit,
            "1111111111111111111111111111111111111111",
            commit_payload.as_bytes()
        ),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));

    let replacement = b"replacement\n";
    let replacement_oid = git_object_identity(b"blob", replacement, GitObjectIdAlgorithm::Sha1)
        .expect("hash replacement Git object");
    assert!(matches!(
        authenticate_git_tree(
            tree,
            &[authenticated_file_entry(
                &replacement_oid,
                "main.omg",
                replacement
            )]
        ),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));

    let false_empty_tree = GitTreeEntry {
        relative_bytes: b"empty".to_vec(),
        relative_path: PathBuf::from("empty"),
        oid: "0000000000000000000000000000000000000000".to_owned(),
        size: 0,
        kind: GitTreeEntryKind::Tree,
    };
    assert!(matches!(
        authenticate_git_tree(tree, &[false_empty_tree]),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));
}

#[test]
fn exact_git_revision_must_equal_the_selected_commit() {
    let revision = "0123456789abcdef0123456789abcdef01234567";
    verify_exact_git_revision(revision, &revision.to_ascii_uppercase())
        .expect("hexadecimal case does not change an object identity");
    verify_exact_git_revision("refs/heads/main", revision)
        .expect("symbolic selectors are bound by ordinary fetch resolution");
    assert!(matches!(
        verify_exact_git_revision(revision, "1123456789abcdef0123456789abcdef01234567"),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));
}

#[test]
fn checked_sha1_rejects_a_known_collision_attack() {
    // SHA-MBLES collision-detection vector, distributed by sha1-checked under MIT/Apache-2.0.
    let encoded = "99040d047fe81780012000ff4b65792069732070617274206f66206120636f6c6c6973696f6e212049742773206120747261702179c61af0afcc054515d9274e7307624b1dc7fb23988bb8de8b575dba7b9eab31c1674b6d974378a827732ff5851c76a2e60772b5a47ce1eac40bb993c12d8c70e24a4f8d5fcdedc1b32c9cf19e31af2429759d42e4dfdb31719f587623ee552939b6dcdc459fca53553b70f87ede30a247ea3af6c759a2f20b320d760db64ff479084fd3ccb3cdd48362d96a9c430617caff6c36c637e53fde28417f626fec54ed7943a46e5f5730f2bb38fb1df6e0090010d00e24ad78bf92641993608e8d158a789f34c46fe1e6027f35a4cbfb827076c50eca0e8b7cca69bb2c2b790259f9bf9570dd8d4437a3115faff7c3cac09ad25266055c27104755178eaeff825a2caa2acfb5de64ce7641dc59a541a9fc9c756756e2e23dc713c8c24c9790aa6b0e38a7f55f14452a1ca2850ddd9562fd9a18ad42496aa97008f74672f68ef461eb88b09933d626b4f918749cc027fddd6c425fc4216835d0134d15285bab2cb784a4f7cbb4fb514d4bf0f6237cf00a9e9f132b9a066e6fd17f6c42987478586ff651af96747fb426b9872b9a88e4063f59bb334cc00650f83a80c42751b71974d300fc2819a2e8f1e32c1b51cb18e6bfc4db9baef675d4aaf5b1574a047f8f6dd2ec153a93412293974d928f88ced9363cfef97ce2e742bf34c96b8ef3875676fea5cca8e5f7dea0bab2413d4de00ee71ee01f162bdb6d1eafd925e6aebaae6a354ef17cf205a404fbdb12fc454d41fdd95cf2459664a2ad032d1da60a73264075d7f1e0d6c1403ae7a0d861df3fe5707188dd5e07d1589b9f8b6630553f8fc352b3e0c27da80bddba4c64020d";
    let collision = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_digit(pair[0]).unwrap() << 4) | hex_digit(pair[1]).unwrap())
        .collect::<Vec<_>>();
    let mut hasher = CheckedSha1::new();
    hasher.update(&collision);

    assert!(matches!(
        finalize_checked_sha1(hasher),
        Err(SourceResolveError::GitObjectInvalid { .. })
    ));
}

#[test]
fn git_object_rejection_precedes_snapshot_staging() {
    let entry_root = temp_root("git-object-rejection-before-stage");
    let error = preflight_git_snapshot(
        "6e3b5fe3c2f6b56c4d150929f0df706a5356004a",
        &[authenticated_file_entry(
            "ce013625030ba8dba906f756967f9e9ca394464a",
            "main.omg",
            b"tampered\n",
        )],
    )
    .expect_err("mismatched object bytes must reject before staging");
    assert!(matches!(error, SourceResolveError::GitObjectInvalid { .. }));
    assert!(
        !entry_root.exists(),
        "object authentication failure must not create a cache or snapshot path"
    );

    let mut escaping = authenticated_file_entry(
        "ce013625030ba8dba906f756967f9e9ca394464a",
        "main.omg",
        b"hello\n",
    );
    escaping.relative_path = std::env::temp_dir().join("omega-escaped-snapshot.omg");
    let error = preflight_git_snapshot("6e3b5fe3c2f6b56c4d150929f0df706a5356004a", &[escaping])
        .expect_err("destination escape must reject before staging");
    assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
    assert!(
        !entry_root.exists(),
        "destination preflight failure must not create a cache or snapshot path"
    );
}
