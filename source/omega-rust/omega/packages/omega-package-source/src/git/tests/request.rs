use super::*;

#[test]
fn git_request_validates_transport_and_emits_sanitized_identity() {
    let https = GitSourceRequest::new(
        "https://GitHub.com/CathedralOS/Arithmetic-Kernels",
        Some("refs/tags/v1.0.0".to_owned()),
    )
    .expect("valid HTTPS request");
    let ssh = GitSourceRequest::new("git@github.com:CathedralOS/Arithmetic-Kernels.git", None)
        .expect("valid SSH request");

    assert_eq!(
        https.locator_identity(),
        "https://github.com/cathedralos/arithmetic-kernels.git"
    );
    assert_eq!(https.locator_identity(), ssh.locator_identity());
    assert_eq!(ssh.requested_revision(), "HEAD");
    assert_eq!(https.lineage(), ssh.lineage());
    assert_eq!(https.execution_transport(), GitExecutionTransport::Https);
    assert_eq!(ssh.execution_transport(), GitExecutionTransport::Ssh);
    assert_ne!(
        git_cache_identity(
            https.locator_identity(),
            https.requested_revision(),
            https.execution_transport(),
        ),
        git_cache_identity(
            ssh.locator_identity(),
            ssh.requested_revision(),
            ssh.execution_transport(),
        )
    );
}

#[test]
fn git_request_derives_requested_endpoint_from_accepted_lineage() {
    for (locator, expected_host, expected_port) in [
        (
            "https://GitHub.com/CathedralOS/Arithmetic-Kernels.git",
            "github.com",
            443,
        ),
        (
            "ssh://git@GITHUB.COM/CathedralOS/Arithmetic-Kernels.git",
            "github.com",
            22,
        ),
        (
            "git@github.com:CathedralOS/Arithmetic-Kernels.git",
            "github.com",
            22,
        ),
        (
            "https://GitLab.com/CathedralOS/libraries/Exact-Math.git",
            "gitlab.com",
            443,
        ),
        (
            "ssh://git@GITLAB.COM/CathedralOS/libraries/Exact-Math.git",
            "gitlab.com",
            22,
        ),
        (
            "git@gitlab.com:CathedralOS/libraries/Exact-Math.git",
            "gitlab.com",
            22,
        ),
        ("https://Git.Example/Group/tool.git", "git.example", 443),
        (
            "https://Git.Example:8443/Group/tool.git",
            "git.example",
            8443,
        ),
        ("ssh://deploy@Git.Example/Group/tool.git", "git.example", 22),
        (
            "ssh://deploy@Git.Example:2222/Group/tool.git",
            "git.example",
            2222,
        ),
        ("deploy@Git.Example:Group/tool.git", "git.example", 22),
    ] {
        let request = GitSourceRequest::new(locator, None).expect("accepted Git locator");
        assert_eq!(
            request.requested_network_endpoint().host(),
            expected_host,
            "wrong endpoint host for {locator:?}"
        );
        assert_eq!(
            request.requested_network_endpoint().port(),
            expected_port,
            "wrong endpoint port for {locator:?}"
        );
    }
}

#[test]
fn requested_endpoint_equality_is_sensitive_to_normalized_host_and_port() {
    let first =
        GitSourceRequest::new("https://Git.Example/Group/first.git", None).expect("first request");
    let same_endpoint = GitSourceRequest::new("https://git.example/Other/second.git", None)
        .expect("same endpoint request");
    let other_host = GitSourceRequest::new("https://other.example/Group/first.git", None)
        .expect("other host request");
    let other_port = GitSourceRequest::new("https://git.example:444/Group/first.git", None)
        .expect("other port request");

    assert_eq!(
        first.requested_network_endpoint(),
        same_endpoint.requested_network_endpoint()
    );
    assert_ne!(
        first.requested_network_endpoint(),
        other_host.requested_network_endpoint()
    );
    assert_ne!(
        first.requested_network_endpoint(),
        other_port.requested_network_endpoint()
    );
}

#[test]
fn git_request_rejects_malformed_and_ambiguous_endpoint_ports() {
    for locator in [
        "https://git.example:/Group/tool.git",
        "https://git.example:0/Group/tool.git",
        "https://git.example:01/Group/tool.git",
        "https://git.example:65536/Group/tool.git",
        "https://git.example:-1/Group/tool.git",
        "https://git.example:port/Group/tool.git",
        "ssh://git@git.example:/Group/tool.git",
        "ssh://git@git.example:0/Group/tool.git",
        "ssh://git@git.example:01/Group/tool.git",
        "ssh://git@git.example:65536/Group/tool.git",
        "ssh://git@git.example:-1/Group/tool.git",
        "ssh://git@git.example:port/Group/tool.git",
        "https://github.com:443/CathedralOS/tool.git",
        "ssh://git@github.com:22/CathedralOS/tool.git",
        "https://gitlab.com:443/CathedralOS/tool.git",
        "ssh://git@gitlab.com:22/CathedralOS/tool.git",
    ] {
        assert!(
            matches!(
                GitSourceRequest::new(locator, None),
                Err(GitSourceRequestError::InvalidLocator(_))
            ),
            "accepted {locator:?}"
        );
    }
}

#[test]
fn git_request_rejects_insecure_secret_bearing_and_local_forms() {
    for locator in [
        "http://github.com/CathedralOS/tool.git",
        "https://token@github.com/CathedralOS/tool.git",
        "ssh://git:secret@github.com/CathedralOS/tool.git",
        "git://github.com/CathedralOS/tool.git",
        "file:///tmp/tool.git",
        "/tmp/tool.git",
    ] {
        assert!(
            matches!(
                GitSourceRequest::new(locator, None),
                Err(GitSourceRequestError::InvalidLocator(_))
            ),
            "accepted {locator:?}"
        );
    }
}

#[test]
fn git_request_rejects_unbounded_or_refspec_shaped_inputs() {
    assert_eq!(
        GitSourceRequest::new("x".repeat(GIT_LOCATOR_BYTE_LIMIT + 1), None),
        Err(GitSourceRequestError::LocatorTooLong {
            limit: GIT_LOCATOR_BYTE_LIMIT
        })
    );
    assert_eq!(
        GitSourceRequest::new(
            "https://example.com/group/tool.git",
            Some("x".repeat(GIT_REVISION_BYTE_LIMIT + 1)),
        ),
        Err(GitSourceRequestError::RevisionTooLong {
            limit: GIT_REVISION_BYTE_LIMIT
        })
    );
    for revision in ["", "--upload-pack=tool", "main:refs/heads/owned", "a..b"] {
        assert!(
            matches!(
                GitSourceRequest::new(
                    "https://example.com/group/tool.git",
                    Some(revision.to_owned())
                ),
                Err(GitSourceRequestError::EmptyRevision)
                    | Err(GitSourceRequestError::InvalidRevision)
            ),
            "accepted {revision:?}"
        );
    }
}

#[test]
fn compiler_owned_source_ceilings_bound_caller_limits() {
    assert_eq!(
        LocalSourceLimits {
            max_entries: usize::MAX,
            max_bytes: u64::MAX,
            max_depth: usize::MAX,
        }
        .compiler_bounded(),
        LocalSourceLimits {
            max_entries: SOURCE_ENTRY_ABSOLUTE_LIMIT,
            max_bytes: SOURCE_BYTE_ABSOLUTE_LIMIT,
            max_depth: SOURCE_DEPTH_ABSOLUTE_LIMIT,
        }
    );
}
