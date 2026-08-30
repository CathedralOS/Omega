//! Validated Git locators, revisions, transports, and endpoint identity.

#[cfg(any(test, feature = "test-fixtures"))]
use crate::identity::digest::format_sha256;
use crate::identity::{GitRequestedNetworkEndpoint, GitTransport, IdentityError, SourceLineage};
use crate::limits::{GIT_LOCATOR_BYTE_LIMIT, GIT_REVISION_BYTE_LIMIT};
use omega_resolver_execution::ResolverExecutionNetworkTransport;
#[cfg(any(test, feature = "test-fixtures"))]
use sha2::{Digest, Sha256};
use std::fmt;
#[cfg(any(test, feature = "test-fixtures"))]
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSourceRequest {
    pub(crate) requested_locator: String,
    pub(crate) fetch_locator: String,
    pub(crate) locator_identity: String,
    pub(crate) requested_revision: String,
    pub(crate) lineage: SourceLineage,
    pub(crate) execution_transport: GitExecutionTransport,
    pub(crate) requested_network_endpoint: GitRequestedNetworkEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitExecutionTransport {
    Https,
    Ssh,
    #[cfg(any(test, feature = "test-fixtures"))]
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitTransportProfile {
    Https,
    Ssh,
    TestFile,
}

impl GitTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Ssh => "ssh",
            Self::TestFile => "test-file",
        }
    }
}

impl GitExecutionTransport {
    fn from_locator_transport(transport: GitTransport) -> Self {
        match transport {
            GitTransport::Https => Self::Https,
            GitTransport::SshUrl | GitTransport::ScpLike => Self::Ssh,
        }
    }

    pub(crate) fn resolver_network_transport(self) -> ResolverExecutionNetworkTransport {
        match self {
            Self::Ssh => ResolverExecutionNetworkTransport::Ssh,
            Self::Https => ResolverExecutionNetworkTransport::Https,
            #[cfg(any(test, feature = "test-fixtures"))]
            Self::File => ResolverExecutionNetworkTransport::Ssh,
        }
    }

    pub(crate) fn cache_tag(self) -> &'static [u8] {
        match self {
            Self::Https => b"https",
            Self::Ssh => b"ssh",
            #[cfg(any(test, feature = "test-fixtures"))]
            Self::File => b"test-file",
        }
    }

    pub(crate) fn allowed_protocol(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Ssh => "ssh",
            #[cfg(any(test, feature = "test-fixtures"))]
            Self::File => "file",
        }
    }

    pub(crate) fn permits(self, transport: Self) -> &'static str {
        if self == transport { "always" } else { "never" }
    }

    pub(crate) fn profile(self) -> GitTransportProfile {
        match self {
            Self::Https => GitTransportProfile::Https,
            Self::Ssh => GitTransportProfile::Ssh,
            #[cfg(any(test, feature = "test-fixtures"))]
            Self::File => GitTransportProfile::TestFile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitSourceRequestError {
    EmptyLocator,
    LocatorTooLong { limit: usize },
    InvalidLocator(IdentityError),
    EmptyRevision,
    RevisionTooLong { limit: usize },
    InvalidRevision,
}

impl GitSourceRequest {
    pub fn new(
        locator: impl Into<String>,
        revision: Option<String>,
    ) -> Result<Self, GitSourceRequestError> {
        let locator = locator.into();
        if locator.is_empty() {
            return Err(GitSourceRequestError::EmptyLocator);
        }
        if locator.len() > GIT_LOCATOR_BYTE_LIMIT {
            return Err(GitSourceRequestError::LocatorTooLong {
                limit: GIT_LOCATOR_BYTE_LIMIT,
            });
        }
        if locator.trim() != locator {
            return Err(GitSourceRequestError::InvalidLocator(
                IdentityError::MalformedGitLocator,
            ));
        }
        let (lineage, locator_transport, requested_network_endpoint) =
            SourceLineage::git_with_transport(&locator)
                .map_err(GitSourceRequestError::InvalidLocator)?;
        let requested_revision = revision.unwrap_or_else(|| "HEAD".to_owned());
        validate_git_revision(&requested_revision)?;
        let locator_identity = canonical_git_locator(&lineage);
        Ok(Self {
            requested_locator: locator.clone(),
            fetch_locator: locator,
            locator_identity,
            requested_revision,
            lineage,
            execution_transport: GitExecutionTransport::from_locator_transport(locator_transport),
            requested_network_endpoint,
        })
    }

    pub fn locator_identity(&self) -> &str {
        &self.locator_identity
    }

    /// The exact validated locator spelling supplied by the caller.
    ///
    /// This remains distinct from normalized lineage and locator identity so a
    /// future lock can retain the selector that was actually resolved. Request
    /// validation rejects embedded credentials before this value exists.
    pub fn requested_locator(&self) -> &str {
        &self.requested_locator
    }

    pub fn requested_revision(&self) -> &str {
        &self.requested_revision
    }

    pub fn lineage(&self) -> &SourceLineage {
        &self.lineage
    }

    pub(crate) fn fetch_locator(&self) -> &str {
        &self.fetch_locator
    }

    pub(crate) fn execution_transport(&self) -> GitExecutionTransport {
        self.execution_transport
    }

    // Retained for the later broker integration without exposing endpoint injection.
    #[allow(dead_code)]
    pub(crate) fn requested_network_endpoint(&self) -> &GitRequestedNetworkEndpoint {
        &self.requested_network_endpoint
    }

    pub fn transport_profile(&self) -> GitTransportProfile {
        self.execution_transport.profile()
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    #[doc(hidden)]
    pub fn for_local_test_repository(
        repository: &Path,
        revision: Option<String>,
    ) -> Result<Self, GitSourceRequestError> {
        let path_identity = Sha256::digest(repository.as_os_str().to_string_lossy().as_bytes());
        let mut request = Self::new(
            format!(
                "https://local-fixture.invalid/{}.git",
                format_sha256(&path_identity)
            ),
            revision,
        )?;
        request.fetch_locator = repository.display().to_string();
        request.execution_transport = GitExecutionTransport::File;
        Ok(request)
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    #[doc(hidden)]
    pub fn for_local_test_repository_with_lineage(
        repository: &Path,
        revision: Option<String>,
        remote_locator: &str,
    ) -> Result<Self, GitSourceRequestError> {
        let mut request = Self::new(remote_locator, revision)?;
        request.fetch_locator = repository.display().to_string();
        request.execution_transport = GitExecutionTransport::File;
        Ok(request)
    }
}

impl fmt::Display for GitSourceRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLocator => formatter.write_str("Git source locator is empty"),
            Self::LocatorTooLong { limit } => {
                write!(formatter, "Git source locator exceeds {limit} bytes")
            }
            Self::InvalidLocator(error) => write!(formatter, "invalid Git source locator: {error}"),
            Self::EmptyRevision => formatter.write_str("Git source revision is empty"),
            Self::RevisionTooLong { limit } => {
                write!(formatter, "Git source revision exceeds {limit} bytes")
            }
            Self::InvalidRevision => formatter.write_str(
                "Git source revision must be one closed selector without refspec syntax",
            ),
        }
    }
}

impl std::error::Error for GitSourceRequestError {}

fn validate_git_revision(revision: &str) -> Result<(), GitSourceRequestError> {
    if revision.is_empty() {
        return Err(GitSourceRequestError::EmptyRevision);
    }
    if revision.len() > GIT_REVISION_BYTE_LIMIT {
        return Err(GitSourceRequestError::RevisionTooLong {
            limit: GIT_REVISION_BYTE_LIMIT,
        });
    }
    if revision.starts_with(['-', '/', '.'])
        || revision.ends_with(['/', '.'])
        || revision.contains("..")
        || revision.contains("//")
        || revision.contains("@{")
        || revision.split('/').any(|component| {
            component.is_empty() || component.starts_with('.') || component.ends_with(".lock")
        })
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        return Err(GitSourceRequestError::InvalidRevision);
    }
    Ok(())
}

fn canonical_git_locator(lineage: &SourceLineage) -> String {
    match lineage {
        SourceLineage::GitHub(lineage) => format!(
            "https://github.com/{}/{}.git",
            lineage.owner(),
            lineage.repository()
        ),
        SourceLineage::GitLab(lineage) => {
            format!("https://gitlab.com/{}.git", lineage.repository_path())
        }
        SourceLineage::Git(lineage) => {
            let user = lineage
                .user()
                .map(|user| format!("{user}@"))
                .unwrap_or_default();
            match lineage.transport() {
                GitTransport::Https => format!(
                    "https://{}{}{}/{}",
                    user,
                    lineage.host(),
                    lineage
                        .port()
                        .map(|port| format!(":{port}"))
                        .unwrap_or_default(),
                    lineage.repository_path()
                ),
                GitTransport::SshUrl => format!(
                    "ssh://{}{}{}/{}",
                    user,
                    lineage.host(),
                    lineage
                        .port()
                        .map(|port| format!(":{port}"))
                        .unwrap_or_default(),
                    lineage.repository_path()
                ),
                GitTransport::ScpLike => {
                    format!("{}{}:{}", user, lineage.host(), lineage.repository_path())
                }
            }
        }
        SourceLineage::Workspace(_) | SourceLineage::ExternalLocal(_) => {
            unreachable!("validated Git requests always carry Git lineage")
        }
    }
}
