use super::locator::{
    CanonicalPort, ParsedGitLocator, is_github_owner, is_github_repository,
    validate_repository_path,
};
use super::resolution::SourceLineageFamily;
use super::{
    ExternalLocalLineage, IdentityError, WorkspaceMemberLineage, hash_field, hash_optional_field,
};
use sha2::Sha256;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceLineage {
    GitHub(GitHubRepositoryLineage),
    GitLab(GitLabRepositoryLineage),
    Git(GenericGitLineage),
    Workspace(WorkspaceMemberLineage),
    ExternalLocal(ExternalLocalLineage),
}

impl SourceLineage {
    /// Preflight owned payload allowance for `git`, including discarded parser
    /// strings. The parsed user/host/port/path together fit within the locator;
    /// GitHub/GitLab normalization copies at most the path once more. Scheme
    /// normalization (or its unsupported-protocol error) is charged separately.
    /// No caller-owned locator copy is included.
    #[doc(hidden)]
    pub fn git_recovery_owned_bytes(locator: &str) -> Option<usize> {
        let scheme = locator
            .split_once("://")
            .map_or(0, |(scheme, _)| scheme.len());
        locator.len().checked_mul(2)?.checked_add(scheme)
    }

    /// Parses a locator already selected by the Git source adapter.
    pub fn git(locator: &str) -> Result<Self, IdentityError> {
        Self::git_with_transport(locator).map(|(lineage, _)| lineage)
    }

    pub(crate) fn git_with_transport(locator: &str) -> Result<(Self, GitTransport), IdentityError> {
        let parsed = ParsedGitLocator::parse(locator)?;
        let transport = parsed.transport;
        let lineage = if parsed.host == "github.com" {
            GitHubRepositoryLineage::from_parsed(parsed).map(Self::GitHub)
        } else if parsed.host == "gitlab.com" {
            GitLabRepositoryLineage::from_parsed(parsed).map(Self::GitLab)
        } else {
            GenericGitLineage::from_parsed(parsed).map(Self::Git)
        }?;
        Ok((lineage, transport))
    }

    pub(super) fn family(&self) -> SourceLineageFamily {
        match self {
            Self::GitHub(_) | Self::GitLab(_) | Self::Git(_) => SourceLineageFamily::Git,
            Self::Workspace(_) => SourceLineageFamily::Workspace,
            Self::ExternalLocal(_) => SourceLineageFamily::ExternalLocal,
        }
    }

    /// Append this lineage's canonical source-owned identity fields.
    #[doc(hidden)]
    pub fn hash_canonical(&self, hasher: &mut Sha256) {
        match self {
            Self::GitHub(lineage) => {
                hash_field(hasher, b"github");
                hash_field(hasher, lineage.owner.as_bytes());
                hash_field(hasher, lineage.repository.as_bytes());
            }
            Self::GitLab(lineage) => {
                hash_field(hasher, b"gitlab");
                hash_field(hasher, lineage.repository_path.as_bytes());
            }
            Self::Git(lineage) => {
                hash_field(hasher, b"git");
                hash_field(hasher, lineage.transport.tag());
                hash_optional_field(hasher, lineage.user.as_deref().map(str::as_bytes));
                hash_field(hasher, lineage.host.as_bytes());
                hash_optional_field(hasher, lineage.port.as_ref().map(|port| port.as_bytes()));
                hash_field(hasher, lineage.repository_path.as_bytes());
            }
            Self::Workspace(lineage) => {
                hash_field(hasher, b"workspace");
                hash_field(hasher, lineage.workspace_identity.as_bytes());
                hash_field(hasher, lineage.member_path.as_str().as_bytes());
            }
            Self::ExternalLocal(lineage) => {
                hash_field(hasher, b"external-local");
                hash_field(hasher, lineage.source_context.as_bytes());
                hash_field(hasher, lineage.canonical_path.as_bytes());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitHubRepositoryLineage {
    owner: String,
    repository: String,
}

impl GitHubRepositoryLineage {
    fn from_parsed(parsed: ParsedGitLocator) -> Result<Self, IdentityError> {
        if parsed.port.is_some() {
            return Err(IdentityError::PortNotAllowed);
        }
        match parsed.transport {
            GitTransport::Https if parsed.user.is_some() => {
                return Err(IdentityError::CredentialsNotAllowed);
            }
            GitTransport::SshUrl | GitTransport::ScpLike
                if parsed.user.as_deref() != Some("git") =>
            {
                return Err(IdentityError::UnexpectedGitHubSshUser);
            }
            _ => {}
        }

        let mut components = parsed.repository_path.split('/');
        let owner = components
            .next()
            .ok_or(IdentityError::MalformedRepositoryPath)?;
        let repository = components
            .next()
            .ok_or(IdentityError::MalformedRepositoryPath)?;
        if components.next().is_some() {
            return Err(IdentityError::MalformedRepositoryPath);
        }
        let repository = repository.strip_suffix(".git").unwrap_or(repository);
        if !is_github_owner(owner) || !is_github_repository(repository) {
            return Err(IdentityError::MalformedRepositoryPath);
        }

        Ok(Self {
            owner: owner.to_ascii_lowercase(),
            repository: repository.to_ascii_lowercase(),
        })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitLabRepositoryLineage {
    repository_path: String,
}

impl GitLabRepositoryLineage {
    fn from_parsed(parsed: ParsedGitLocator) -> Result<Self, IdentityError> {
        if parsed.port.is_some() {
            return Err(IdentityError::PortNotAllowed);
        }
        match parsed.transport {
            GitTransport::Https if parsed.user.is_some() => {
                return Err(IdentityError::CredentialsNotAllowed);
            }
            GitTransport::SshUrl | GitTransport::ScpLike
                if parsed.user.as_deref() != Some("git") =>
            {
                return Err(IdentityError::UnexpectedGitLabSshUser);
            }
            _ => {}
        }

        let repository_path = parsed
            .repository_path
            .strip_suffix(".git")
            .unwrap_or(&parsed.repository_path);
        let repository_path = validate_repository_path(repository_path)?;
        if repository_path.split('/').count() < 2 {
            return Err(IdentityError::MalformedRepositoryPath);
        }
        Ok(Self { repository_path })
    }

    pub fn repository_path(&self) -> &str {
        &self.repository_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenericGitLineage {
    transport: GitTransport,
    user: Option<String>,
    host: String,
    port: Option<CanonicalPort>,
    repository_path: String,
}

impl GenericGitLineage {
    fn from_parsed(parsed: ParsedGitLocator) -> Result<Self, IdentityError> {
        Ok(Self {
            transport: parsed.transport,
            user: parsed.user,
            host: parsed.host,
            port: parsed.port,
            repository_path: parsed.repository_path,
        })
    }

    pub fn transport(&self) -> GitTransport {
        self.transport
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> Option<u16> {
        self.port.as_ref().map(CanonicalPort::get)
    }

    pub fn repository_path(&self) -> &str {
        &self.repository_path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitTransport {
    Https,
    SshUrl,
    ScpLike,
}

impl GitTransport {
    fn tag(self) -> &'static [u8] {
        match self {
            Self::Https => b"https",
            Self::SshUrl => b"ssh-url",
            Self::ScpLike => b"scp-like",
        }
    }
}
