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
