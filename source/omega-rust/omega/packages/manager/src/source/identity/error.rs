use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    UnsupportedGitProtocol { scheme: String },
    MalformedGitLocator,
    MalformedRepositoryPath,
    CredentialsNotAllowed,
    QueryOrFragmentNotAllowed,
    PortNotAllowed,
    UnexpectedGitHubSshUser,
    UnexpectedGitLabSshUser,
    InvalidWorkspaceMemberPath,
    RecursiveWorkspaceLineage,
    CanonicalPath { path: PathBuf, error: String },
    UnsupportedNonUtf8Path(PathBuf),
    InvalidDigest,
    InvalidGitObjectId,
    GitObjectFormatMismatch,
    ResolutionLineageMismatch,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedGitProtocol { scheme } => {
                write!(formatter, "unsupported Git protocol `{scheme}`")
            }
            Self::MalformedGitLocator => formatter.write_str("malformed Git repository locator"),
            Self::MalformedRepositoryPath => formatter.write_str("malformed Git repository path"),
            Self::CredentialsNotAllowed => formatter
                .write_str("credentials and embedded secrets are not allowed in source identity"),
            Self::QueryOrFragmentNotAllowed => formatter
                .write_str("query strings and fragments are not allowed in source identity"),
            Self::PortNotAllowed => formatter
                .write_str("ports are not part of a normalized known-host repository namespace"),
            Self::UnexpectedGitHubSshUser => {
                formatter.write_str("GitHub SSH repository identity requires the `git` user")
            }
            Self::UnexpectedGitLabSshUser => {
                formatter.write_str("GitLab SSH repository identity requires the `git` user")
            }
            Self::InvalidWorkspaceMemberPath => formatter
                .write_str("workspace member path must be a normalized portable relative path"),
            Self::RecursiveWorkspaceLineage => {
                formatter.write_str("a workspace lineage cannot be derived from a workspace member")
            }
            Self::CanonicalPath { path, error } => write!(
                formatter,
                "cannot establish canonical external source path `{}`: {error}",
                path.display()
            ),
            Self::UnsupportedNonUtf8Path(path) => write!(
                formatter,
                "non-UTF-8 external source path `{}` is unsupported in v1",
                path.display()
            ),
            Self::InvalidDigest => {
                formatter.write_str("digest must be exactly 32 bytes or 64 hexadecimal characters")
            }
            Self::InvalidGitObjectId => formatter.write_str(
                "Git object ID must be a complete 40- or 64-character hexadecimal value",
            ),
            Self::GitObjectFormatMismatch => {
                formatter.write_str("Git commit and tree IDs must use the same object format")
            }
            Self::ResolutionLineageMismatch => formatter
                .write_str("immutable source resolution does not match package source lineage"),
        }
    }
}

impl std::error::Error for IdentityError {}
