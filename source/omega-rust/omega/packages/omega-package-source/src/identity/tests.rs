use super::*;

fn lineage(locator: &str) -> SourceLineage {
    SourceLineage::git(locator).unwrap()
}

#[test]
fn github_https_scp_and_ssh_url_share_one_repository_lineage() {
    let https = lineage("https://GitHub.com/CathedralOS/Arithmetic-Kernels.git");
    let scp = lineage("git@github.com:cathedralos/arithmetic-kernels");
    let ssh = lineage("ssh://git@GITHUB.COM/CATHEDRALOS/ARITHMETIC-KERNELS.git");

    assert_eq!(https, scp);
    assert_eq!(https, ssh);
    let SourceLineage::GitHub(lineage) = https else {
        panic!("GitHub locator did not use known-host normalization");
    };
    assert_eq!(lineage.owner(), "cathedralos");
    assert_eq!(lineage.repository(), "arithmetic-kernels");
}

#[test]
fn github_only_strips_a_terminal_lowercase_dot_git() {
    assert_eq!(
        lineage("https://github.com/CathedralOS/tool.git"),
        lineage("https://github.com/CathedralOS/tool")
    );
    assert_ne!(
        lineage("https://github.com/CathedralOS/tool.git.git"),
        lineage("https://github.com/CathedralOS/tool")
    );
    assert_ne!(
        lineage("https://github.com/CathedralOS/tool.GIT"),
        lineage("https://github.com/CathedralOS/tool")
    );
}

#[test]
fn github_rejects_credentials_queries_fragments_ports_and_bad_namespaces() {
    for locator in [
        "https://token@github.com/CathedralOS/tool.git",
        "https://github.com/CathedralOS/tool.git?ref=main",
        "https://github.com/CathedralOS/tool.git#readme",
        "https://github.com:443/CathedralOS/tool.git",
        "ssh://root@github.com/CathedralOS/tool.git",
        "ssh://git@github.com:22/CathedralOS/tool.git",
        "https://github.com/CathedralOS/tool/extra",
        "https://github.com/CathedralOS/../tool",
        "https://github.com/CathedralOS/%74ool",
        "https://gіthub.com/CathedralOS/tool",
    ] {
        assert!(SourceLineage::git(locator).is_err(), "accepted {locator:?}");
    }
}

#[test]
fn github_lookalike_hosts_do_not_receive_github_equivalence() {
    let github = lineage("https://github.com/CathedralOS/tool");
    let lookalike = lineage("https://github.com.evil.example/CathedralOS/tool");

    assert_ne!(github, lookalike);
    assert!(matches!(lookalike, SourceLineage::Git(_)));
}

#[test]
fn gitlab_https_scp_and_ssh_url_share_one_nested_repository_lineage() {
    let https = lineage("https://GitLab.com/CathedralOS/libraries/Exact-Math.git");
    let scp = lineage("git@gitlab.com:CathedralOS/libraries/Exact-Math");
    let ssh = lineage("ssh://git@GITLAB.COM/CathedralOS/libraries/Exact-Math.git");

    assert_eq!(https, scp);
    assert_eq!(https, ssh);
    let SourceLineage::GitLab(lineage) = https else {
        panic!("GitLab locator did not use known-host normalization");
    };
    assert_eq!(
        lineage.repository_path(),
        "CathedralOS/libraries/Exact-Math"
    );
}

#[test]
fn gitlab_preserves_path_case_and_rejects_ambiguous_known_host_forms() {
    assert_ne!(
        lineage("https://gitlab.com/CathedralOS/libraries/Exact-Math.git"),
        lineage("https://gitlab.com/cathedralos/libraries/exact-math.git")
    );
    for locator in [
        "https://token@gitlab.com/CathedralOS/tool.git",
        "https://gitlab.com:443/CathedralOS/tool.git",
        "ssh://deploy@gitlab.com/CathedralOS/tool.git",
        "git@gitlab.com:tool.git",
        "https://gitlab.com/CathedralOS/../tool.git",
    ] {
        assert!(SourceLineage::git(locator).is_err(), "accepted {locator:?}");
    }
}

#[test]
fn gitlab_lookalikes_and_self_hosted_instances_remain_generic() {
    let hosted = lineage("https://gitlab.com/CathedralOS/tool.git");
    let lookalike = lineage("https://gitlab.com.evil.example/CathedralOS/tool.git");
    let self_hosted = lineage("https://gitlab.example/CathedralOS/tool.git");

    assert_ne!(hosted, lookalike);
    assert_ne!(hosted, self_hosted);
    assert!(matches!(lookalike, SourceLineage::Git(_)));
    assert!(matches!(self_hosted, SourceLineage::Git(_)));
}

#[test]
fn generic_git_keeps_transport_path_user_and_port_distinct() {
    let https = lineage("https://gitlab.example/Group/tool.git");
    let ssh = lineage("ssh://git@gitlab.example/Group/tool.git");
    let scp = lineage("git@gitlab.example:Group/tool.git");
    let other_user = lineage("ssh://deploy@gitlab.example/Group/tool.git");
    let ssh_port = lineage("ssh://git@gitlab.example:2222/Group/tool.git");
    let no_suffix = lineage("https://gitlab.example/Group/tool");

    assert_ne!(https, ssh);
    assert_ne!(ssh, scp);
    assert_ne!(ssh, other_user);
    assert_ne!(ssh, ssh_port);
    assert_ne!(https, no_suffix);
    assert_eq!(lineage("https://GITLAB.EXAMPLE/Group/tool.git"), https);
    assert_ne!(lineage("https://gitlab.example/group/tool.git"), https);
}

#[test]
fn generic_git_rejects_secrets_ambiguous_paths_and_unknown_protocols() {
    for locator in [
        "https://token@gitlab.example/group/tool",
        "ssh://git:secret@gitlab.example/group/tool",
        "ssh://git@gitlab.example/group/../tool",
        "ssh://git@gitlab.example/group//tool",
        "git@gitlab.example:group/%74ool",
        "ftp://gitlab.example/group/tool",
        "file:///tmp/tool",
        "git+https://gitlab.example/group/tool",
    ] {
        assert!(SourceLineage::git(locator).is_err(), "accepted {locator:?}");
    }
}

#[test]
fn workspace_member_paths_are_normalized_and_traversal_free() {
    assert_eq!(
        SourceRelativePath::parse("packages/arithmetic-kernels")
            .unwrap()
            .as_str(),
        "packages/arithmetic-kernels"
    );
    for path in [
        "",
        ".",
        "..",
        "../outside",
        "packages/../outside",
        "/absolute",
        "packages//tool",
        "packages/tool/",
        "packages\\tool",
        "packages/naïve",
    ] {
        assert!(
            SourceRelativePath::parse(path).is_err(),
            "accepted {path:?}"
        );
    }
}

#[test]
fn workspace_lineage_binds_root_identity_and_member_path() {
    let root = lineage("https://github.com/CathedralOS/workspace.git");
    let workspace = WorkspaceLineageIdentity::from_root_source(&root).unwrap();
    let first = SourceLineage::Workspace(WorkspaceMemberLineage::new(
        workspace.clone(),
        SourceRelativePath::parse("packages/first").unwrap(),
    ));
    let second = SourceLineage::Workspace(WorkspaceMemberLineage::new(
        workspace,
        SourceRelativePath::parse("packages/second").unwrap(),
    ));

    assert_ne!(first, second);
    assert!(WorkspaceLineageIdentity::from_root_source(&first).is_err());
}

#[test]
fn external_local_lineage_is_canonical_nonportable_and_context_bound() {
    let current = std::env::current_dir().unwrap();
    let first = SourceLineage::ExternalLocal(
        ExternalLocalLineage::canonicalize(
            current.join("."),
            ExternalSourceContext::derive(b"lock-a"),
        )
        .unwrap(),
    );
    let same = SourceLineage::ExternalLocal(
        ExternalLocalLineage::canonicalize(&current, ExternalSourceContext::derive(b"lock-a"))
            .unwrap(),
    );
    let other_context = SourceLineage::ExternalLocal(
        ExternalLocalLineage::canonicalize(&current, ExternalSourceContext::derive(b"lock-b"))
            .unwrap(),
    );

    assert_eq!(first, same);
    assert_ne!(first, other_context);
    let SourceLineage::ExternalLocal(lineage) = first else {
        unreachable!()
    };
    assert!(lineage.canonical_absolute_path().is_absolute());
    assert!(!lineage.is_portable());
}

#[test]
fn recovered_external_local_lineage_rejects_noncanonical_separators() {
    let context = ExternalSourceContext::derive(b"review-baseline");
    let canonical = std::env::temp_dir().join("omega-recovered-source");
    let canonical = canonical.to_str().unwrap().to_owned();
    assert!(
        ExternalLocalLineage::from_recovered_canonical_path(canonical.clone(), context.clone())
            .is_ok()
    );
    assert!(
        ExternalLocalLineage::from_recovered_canonical_path(
            format!("{canonical}{}", std::path::MAIN_SEPARATOR),
            context.clone(),
        )
        .is_err()
    );
    assert!(
        ExternalLocalLineage::from_recovered_canonical_path(
            canonical.replacen(
                std::path::MAIN_SEPARATOR,
                &std::path::MAIN_SEPARATOR.to_string().repeat(2),
                1,
            ),
            context,
        )
        .is_err()
    );
}

#[test]
fn commit_and_tree_each_change_git_source_resolution() {
    fn source(commit: u8, tree: u8) -> ImmutableSourceResolution {
        ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&format!("{commit:02x}").repeat(20)).unwrap(),
            GitTreeId::parse_hex(&format!("{tree:02x}").repeat(20)).unwrap(),
        )
        .unwrap()
    }

    let base = source(1, 2);

    assert_ne!(base, source(4, 2));
    assert_ne!(base, source(1, 4));
}

#[test]
fn git_content_identity_is_derived_only_from_the_authenticated_root_tree() {
    let tree = GitTreeId::parse_hex(&"02".repeat(20)).unwrap();
    let first = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&"01".repeat(20)).unwrap(),
        tree.clone(),
    )
    .unwrap();
    let second =
        ImmutableSourceResolution::git(GitCommitId::parse_hex(&"03".repeat(20)).unwrap(), tree)
            .unwrap();

    assert_eq!(first.content(), second.content());
}

#[test]
fn source_digest_is_canonical_and_rejects_incomplete_hex() {
    let source = SourceContentDigest::derive(b"same bytes");
    assert_eq!(SourceContentDigest::parse_hex(&source.to_hex()), Ok(source));
    assert!(SourceContentDigest::parse_hex("abc").is_err());
}

#[test]
fn git_object_ids_are_complete_typed_and_canonical() {
    let commit = GitCommitId::parse_hex(&"AB".repeat(20)).unwrap();
    let tree = GitTreeId::parse_hex(&"cd".repeat(32)).unwrap();

    assert_eq!(commit.algorithm(), GitObjectIdAlgorithm::Sha1);
    assert_eq!(tree.algorithm(), GitObjectIdAlgorithm::Sha256);
    assert_eq!(commit.to_hex(), "ab".repeat(20));
    for invalid in ["abc123".to_owned(), "g0".repeat(20), "00".repeat(21)] {
        assert!(GitCommitId::parse_hex(&invalid).is_err());
    }
    assert_eq!(
        ImmutableSourceResolution::git(commit, tree),
        Err(IdentityError::GitObjectFormatMismatch)
    );
}
