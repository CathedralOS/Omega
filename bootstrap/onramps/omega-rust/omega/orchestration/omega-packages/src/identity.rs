use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Component, Path, PathBuf};

macro_rules! domain_digest {
    ($name:ident, $domain:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn derive(canonical_evidence: &[u8]) -> Self {
                let mut hasher = Sha256::new();
                hash_field(&mut hasher, $domain);
                hash_field(&mut hasher, canonical_evidence);
                Self(hasher.finalize().into())
            }

            pub fn parse_hex(value: &str) -> Result<Self, IdentityError> {
                let bytes = decode_hex(value).ok_or(IdentityError::InvalidDigest)?;
                let bytes = bytes.try_into().map_err(|_| IdentityError::InvalidDigest)?;
                Ok(Self(bytes))
            }

            pub fn to_hex(&self) -> String {
                encode_hex(&self.0)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageName(String);

impl PackageName {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if is_kebab_case(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "package identity `{value}` must start with a lowercase letter and use kebab-case lowercase words"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn default_alias(&self) -> AliasName {
        AliasName(self.0.replace('-', "_"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AliasName(String);

impl AliasName {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if is_snake_case(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "dependency alias `{value}` must use snake_case Omega identifier spelling"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageKey {
    name: PackageName,
    source_lineage: SourceLineage,
}

impl PackageKey {
    pub fn new(name: PackageName, source_lineage: SourceLineage) -> Self {
        Self {
            name,
            source_lineage,
        }
    }

    pub fn name(&self) -> &PackageName {
        &self.name
    }

    pub fn source_lineage(&self) -> &SourceLineage {
        &self.source_lineage
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageInstance {
    key: PackageKey,
    source_resolution: ImmutableSourceResolution,
    toolchain: ToolchainIdentity,
    compiler_evidence: CompilerEvidenceFingerprint,
}

impl PackageInstance {
    pub fn new(
        key: PackageKey,
        source_resolution: ImmutableSourceResolution,
        toolchain: ToolchainIdentity,
        compiler_evidence: CompilerEvidenceFingerprint,
    ) -> Result<Self, IdentityError> {
        if !source_resolution.matches_lineage(key.source_lineage()) {
            return Err(IdentityError::ResolutionLineageMismatch);
        }
        Ok(Self {
            key,
            source_resolution,
            toolchain,
            compiler_evidence,
        })
    }

    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn source_resolution(&self) -> &ImmutableSourceResolution {
        &self.source_resolution
    }

    pub fn toolchain(&self) -> &ToolchainIdentity {
        &self.toolchain
    }

    pub fn compiler_evidence(&self) -> &CompilerEvidenceFingerprint {
        &self.compiler_evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceLineage {
    GitHub(GitHubRepositoryLineage),
    Git(GenericGitLineage),
    Workspace(WorkspaceMemberLineage),
    ExternalLocal(ExternalLocalLineage),
}

impl SourceLineage {
    /// Parses a locator already selected by the Git source adapter.
    pub fn git(locator: &str) -> Result<Self, IdentityError> {
        let parsed = ParsedGitLocator::parse(locator)?;
        if parsed.host == "github.com" {
            GitHubRepositoryLineage::from_parsed(parsed).map(Self::GitHub)
        } else {
            GenericGitLineage::from_parsed(parsed).map(Self::Git)
        }
    }

    fn family(&self) -> SourceLineageFamily {
        match self {
            Self::GitHub(_) | Self::Git(_) => SourceLineageFamily::Git,
            Self::Workspace(_) => SourceLineageFamily::Workspace,
            Self::ExternalLocal(_) => SourceLineageFamily::ExternalLocal,
        }
    }

    fn hash_canonical(&self, hasher: &mut Sha256) {
        match self {
            Self::GitHub(lineage) => {
                hash_field(hasher, b"github");
                hash_field(hasher, lineage.owner.as_bytes());
                hash_field(hasher, lineage.repository.as_bytes());
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceMemberLineage {
    workspace_identity: WorkspaceLineageIdentity,
    member_path: WorkspaceMemberPath,
}

impl WorkspaceMemberLineage {
    pub fn new(
        workspace_identity: WorkspaceLineageIdentity,
        member_path: WorkspaceMemberPath,
    ) -> Self {
        Self {
            workspace_identity,
            member_path,
        }
    }

    pub fn workspace_identity(&self) -> &WorkspaceLineageIdentity {
        &self.workspace_identity
    }

    pub fn member_path(&self) -> &WorkspaceMemberPath {
        &self.member_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceMemberPath(String);

impl WorkspaceMemberPath {
    pub fn parse(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(IdentityError::InvalidWorkspaceMemberPath);
        }
        for component in value.split('/') {
            if component.is_empty()
                || matches!(component, "." | "..")
                || !component.bytes().all(is_portable_path_byte)
            {
                return Err(IdentityError::InvalidWorkspaceMemberPath);
            }
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalLocalLineage {
    canonical_path: String,
    source_context: ExternalSourceContext,
}

impl ExternalLocalLineage {
    pub fn canonicalize(
        path: impl AsRef<Path>,
        source_context: ExternalSourceContext,
    ) -> Result<Self, IdentityError> {
        let canonical =
            std::fs::canonicalize(path.as_ref()).map_err(|error| IdentityError::CanonicalPath {
                path: path.as_ref().to_path_buf(),
                error: error.to_string(),
            })?;
        if !canonical.is_absolute()
            || canonical
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(IdentityError::CanonicalPath {
                path: canonical,
                error: "canonicalized path was not absolute and normalized".to_owned(),
            });
        }
        let canonical_path = canonical
            .to_str()
            .ok_or_else(|| IdentityError::UnsupportedNonUtf8Path(canonical.clone()))?
            .to_owned();

        Ok(Self {
            canonical_path,
            source_context,
        })
    }

    pub fn canonical_absolute_path(&self) -> &Path {
        Path::new(&self.canonical_path)
    }

    pub fn source_context(&self) -> &ExternalSourceContext {
        &self.source_context
    }

    pub fn is_portable(&self) -> bool {
        false
    }
}

domain_digest!(WorkspaceLineageIdentity, b"omega-workspace-lineage-v1");
domain_digest!(ExternalSourceContext, b"omega-external-source-context-v1");
domain_digest!(SourceContentDigest, b"omega-source-content-v1");
domain_digest!(ToolchainIdentity, b"omega-toolchain-identity-v1");
domain_digest!(CompilerEvidenceFingerprint, b"omega-compiler-evidence-v1");

impl WorkspaceLineageIdentity {
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn from_root_source(root_source: &SourceLineage) -> Result<Self, IdentityError> {
        if matches!(root_source, SourceLineage::Workspace(_)) {
            return Err(IdentityError::RecursiveWorkspaceLineage);
        }
        let mut canonical = Sha256::new();
        hash_field(&mut canonical, b"omega-source-lineage-canonical-v1");
        root_source.hash_canonical(&mut canonical);
        Ok(Self::derive(&canonical.finalize()))
    }
}

impl ExternalSourceContext {
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImmutableSourceResolution {
    Git {
        commit: GitCommitId,
        tree: GitTreeId,
        content: SourceContentDigest,
    },
    Workspace {
        content: SourceContentDigest,
    },
    ExternalLocal {
        content: SourceContentDigest,
    },
}

impl ImmutableSourceResolution {
    pub fn git(
        commit: GitCommitId,
        tree: GitTreeId,
        content: SourceContentDigest,
    ) -> Result<Self, IdentityError> {
        if commit.algorithm() != tree.algorithm() {
            return Err(IdentityError::GitObjectFormatMismatch);
        }
        Ok(Self::Git {
            commit,
            tree,
            content,
        })
    }

    pub fn workspace(content: SourceContentDigest) -> Self {
        Self::Workspace { content }
    }

    pub fn external_local(content: SourceContentDigest) -> Self {
        Self::ExternalLocal { content }
    }

    pub fn content(&self) -> &SourceContentDigest {
        match self {
            Self::Git { content, .. }
            | Self::Workspace { content }
            | Self::ExternalLocal { content } => content,
        }
    }

    pub(crate) fn matches_lineage(&self, lineage: &SourceLineage) -> bool {
        matches!(
            (self, lineage.family()),
            (Self::Git { .. }, SourceLineageFamily::Git)
                | (Self::Workspace { .. }, SourceLineageFamily::Workspace)
                | (
                    Self::ExternalLocal { .. },
                    SourceLineageFamily::ExternalLocal
                )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitCommitId(GitObjectId);

impl GitCommitId {
    pub fn parse_hex(value: &str) -> Result<Self, IdentityError> {
        GitObjectId::parse_hex(value).map(Self)
    }

    pub fn algorithm(&self) -> GitObjectIdAlgorithm {
        self.0.algorithm()
    }

    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitTreeId(GitObjectId);

impl GitTreeId {
    pub fn parse_hex(value: &str) -> Result<Self, IdentityError> {
        GitObjectId::parse_hex(value).map(Self)
    }

    pub fn algorithm(&self) -> GitObjectIdAlgorithm {
        self.0.algorithm()
    }

    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitObjectIdAlgorithm {
    Sha1,
    Sha256,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum GitObjectId {
    Sha1([u8; 20]),
    Sha256([u8; 32]),
}

impl GitObjectId {
    fn parse_hex(value: &str) -> Result<Self, IdentityError> {
        match value.len() {
            40 => decode_hex(value)
                .and_then(|bytes| bytes.try_into().ok())
                .map(Self::Sha1)
                .ok_or(IdentityError::InvalidGitObjectId),
            64 => decode_hex(value)
                .and_then(|bytes| bytes.try_into().ok())
                .map(Self::Sha256)
                .ok_or(IdentityError::InvalidGitObjectId),
            _ => Err(IdentityError::InvalidGitObjectId),
        }
    }

    fn algorithm(&self) -> GitObjectIdAlgorithm {
        match self {
            Self::Sha1(_) => GitObjectIdAlgorithm::Sha1,
            Self::Sha256(_) => GitObjectIdAlgorithm::Sha256,
        }
    }

    fn to_hex(&self) -> String {
        match self {
            Self::Sha1(bytes) => encode_hex(bytes),
            Self::Sha256(bytes) => encode_hex(bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    UnsupportedGitProtocol { scheme: String },
    MalformedGitLocator,
    MalformedRepositoryPath,
    CredentialsNotAllowed,
    QueryOrFragmentNotAllowed,
    PortNotAllowed,
    UnexpectedGitHubSshUser,
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
            Self::PortNotAllowed => {
                formatter.write_str("ports are not part of the stable GitHub repository namespace")
            }
            Self::UnexpectedGitHubSshUser => {
                formatter.write_str("GitHub SSH repository identity requires the `git` user")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceLineageFamily {
    Git,
    Workspace,
    ExternalLocal,
}

#[derive(Debug)]
struct ParsedGitLocator {
    transport: GitTransport,
    user: Option<String>,
    host: String,
    port: Option<CanonicalPort>,
    repository_path: String,
}

impl ParsedGitLocator {
    fn parse(locator: &str) -> Result<Self, IdentityError> {
        if locator.contains('?') || locator.contains('#') {
            return Err(IdentityError::QueryOrFragmentNotAllowed);
        }

        if let Some((scheme, remainder)) = locator.split_once("://") {
            return match scheme.to_ascii_lowercase().as_str() {
                "https" => Self::parse_url(GitTransport::Https, remainder),
                "ssh" => Self::parse_url(GitTransport::SshUrl, remainder),
                _ => Err(IdentityError::UnsupportedGitProtocol {
                    scheme: scheme.to_owned(),
                }),
            };
        }

        Self::parse_scp_like(locator)
    }

    fn parse_url(transport: GitTransport, remainder: &str) -> Result<Self, IdentityError> {
        let (authority, path) = remainder
            .split_once('/')
            .ok_or(IdentityError::MalformedGitLocator)?;
        if authority.is_empty() || path.is_empty() || path.starts_with('/') {
            return Err(IdentityError::MalformedGitLocator);
        }

        let (user, host_and_port) = match authority.rsplit_once('@') {
            Some((user_info, host)) => {
                if user_info.is_empty()
                    || user_info.contains(':')
                    || authority.matches('@').count() != 1
                {
                    return Err(IdentityError::CredentialsNotAllowed);
                }
                if transport == GitTransport::Https {
                    return Err(IdentityError::CredentialsNotAllowed);
                }
                validate_ssh_user(user_info)?;
                (Some(user_info.to_owned()), host)
            }
            None => {
                if transport == GitTransport::SshUrl {
                    return Err(IdentityError::MalformedGitLocator);
                }
                (None, authority)
            }
        };
        let (host, port) = parse_host_and_port(host_and_port)?;

        Ok(Self {
            transport,
            user,
            host,
            port,
            repository_path: validate_repository_path(path)?,
        })
    }

    fn parse_scp_like(locator: &str) -> Result<Self, IdentityError> {
        let (user_and_host, path) = locator
            .split_once(':')
            .ok_or(IdentityError::MalformedGitLocator)?;
        let (user, host) = user_and_host
            .split_once('@')
            .ok_or(IdentityError::MalformedGitLocator)?;
        if user.is_empty() || host.is_empty() || user.contains(':') || host.contains('@') {
            return Err(IdentityError::MalformedGitLocator);
        }
        validate_ssh_user(user)?;

        Ok(Self {
            transport: GitTransport::ScpLike,
            user: Some(user.to_owned()),
            host: validate_host(host)?,
            port: None,
            repository_path: validate_repository_path(path)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CanonicalPort(String);

impl CanonicalPort {
    fn parse(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || value.starts_with('0')
        {
            return Err(IdentityError::MalformedGitLocator);
        }
        let port = value
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(IdentityError::MalformedGitLocator)?;
        Ok(Self(port.to_string()))
    }

    fn get(&self) -> u16 {
        self.0.parse().expect("canonical port is a valid u16")
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

fn parse_host_and_port(value: &str) -> Result<(String, Option<CanonicalPort>), IdentityError> {
    if value.starts_with('[') || value.contains(']') || value.matches(':').count() > 1 {
        return Err(IdentityError::MalformedGitLocator);
    }
    let (host, port) = match value.rsplit_once(':') {
        Some((host, port)) => (host, Some(CanonicalPort::parse(port)?)),
        None => (value, None),
    };
    Ok((validate_host(host)?, port))
}

fn validate_host(value: &str) -> Result<String, IdentityError> {
    if value.is_empty()
        || value.starts_with('.')
        || value.ends_with('.')
        || value.split('.').any(|label| {
            label.is_empty()
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(IdentityError::MalformedGitLocator);
    }
    Ok(value.to_ascii_lowercase())
}

fn validate_ssh_user(value: &str) -> Result<(), IdentityError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(IdentityError::MalformedGitLocator);
    }
    Ok(())
}

fn validate_repository_path(value: &str) -> Result<String, IdentityError> {
    if value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.contains('%')
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(IdentityError::MalformedRepositoryPath);
    }
    for component in value.split('/') {
        if component.is_empty()
            || matches!(component, "." | "..")
            || !component.bytes().all(is_repository_path_byte)
        {
            return Err(IdentityError::MalformedRepositoryPath);
        }
    }
    Ok(value.to_owned())
}

fn is_repository_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
}

fn is_portable_path_byte(byte: u8) -> bool {
    is_repository_path_byte(byte)
}

fn is_github_owner(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn is_github_repository(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !matches!(value, "." | "..")
        && value.bytes().all(is_repository_path_byte)
}

fn is_kebab_case(value: &str) -> bool {
    if !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase) || value.ends_with('-') {
        return false;
    }

    let mut previous_separator = false;
    for byte in value.bytes() {
        if byte == b'-' {
            if previous_separator {
                return false;
            }
            previous_separator = true;
            continue;
        }
        previous_separator = false;
        if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
            return false;
        }
    }
    true
}

fn is_snake_case(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && !value.ends_with('_')
        && !value.contains("__")
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hash_optional_field(hasher: &mut Sha256, bytes: Option<&[u8]>) {
    match bytes {
        Some(bytes) => {
            hasher.update([1]);
            hash_field(hasher, bytes);
        }
        None => hasher.update([0]),
    }
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_value(pair[0])?;
            let low = hex_value(pair[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package_name() -> PackageName {
        PackageName::parse("arithmetic-kernels").unwrap()
    }

    fn lineage(locator: &str) -> SourceLineage {
        SourceLineage::git(locator).unwrap()
    }

    fn git_resolution(seed: u8) -> ImmutableSourceResolution {
        ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&format!("{seed:02x}").repeat(20)).unwrap(),
            GitTreeId::parse_hex(&format!("{:02x}", seed.wrapping_add(1)).repeat(20)).unwrap(),
            SourceContentDigest::derive(&[seed]),
        )
        .unwrap()
    }

    fn instance(key: PackageKey, seed: u8) -> PackageInstance {
        PackageInstance::new(
            key,
            git_resolution(seed),
            ToolchainIdentity::derive(&[seed, 1]),
            CompilerEvidenceFingerprint::derive(&[seed, 2]),
        )
        .unwrap()
    }

    #[test]
    fn package_names_require_canonical_kebab_case_and_reject_spoofs() {
        assert!(PackageName::parse("arithmetic-kernels").is_ok());
        assert!(PackageName::parse("sha256").is_ok());
        assert!(PackageName::parse("codec-2").is_ok());
        for invalid in [
            "",
            "Arithmetic-kernels",
            "arithmetic_kernels",
            "-arithmetic",
            "arithmetic-",
            "arithmetic--kernels",
            "arithmetic.kernels",
            "123-tools",
            "arithmetіc-kernels",
        ] {
            assert!(PackageName::parse(invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn aliases_require_canonical_snake_case_identifiers() {
        for valid in ["arithmetic_kernels", "sha256", "codec_2"] {
            assert_eq!(AliasName::parse(valid).unwrap().as_str(), valid);
        }
        for invalid in [
            "",
            "Arithmetic_kernels",
            "arithmetic-kernels",
            "_arithmetic",
            "arithmetic_",
            "arithmetic__kernels",
            "123_tools",
            "arithmetіc_kernels",
        ] {
            assert!(AliasName::parse(invalid).is_err(), "accepted {invalid:?}");
        }
        assert_eq!(
            package_name().default_alias().as_str(),
            "arithmetic_kernels"
        );
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
            WorkspaceMemberPath::parse("packages/arithmetic-kernels")
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
                WorkspaceMemberPath::parse(path).is_err(),
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
            WorkspaceMemberPath::parse("packages/first").unwrap(),
        ));
        let second = SourceLineage::Workspace(WorkspaceMemberLineage::new(
            workspace,
            WorkspaceMemberPath::parse("packages/second").unwrap(),
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
    fn source_or_name_change_is_replacement_while_revision_change_is_update() {
        let original_key = PackageKey::new(
            package_name(),
            lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
        );
        let transport_equivalent_key = PackageKey::new(
            package_name(),
            lineage("git@github.com:cathedralos/arithmetic-kernels"),
        );
        let other_source_key = PackageKey::new(
            package_name(),
            lineage("https://github.com/Other/arithmetic-kernels.git"),
        );
        let other_name_key = PackageKey::new(
            PackageName::parse("arithmetic-core").unwrap(),
            lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
        );

        assert_eq!(original_key, transport_equivalent_key);
        assert_ne!(original_key, other_source_key);
        assert_ne!(original_key, other_name_key);
        assert_ne!(instance(original_key.clone(), 1), instance(original_key, 2));
    }

    #[test]
    fn every_instance_evidence_axis_changes_instance_but_not_key() {
        let key = PackageKey::new(
            package_name(),
            lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
        );
        let base = PackageInstance::new(
            key.clone(),
            git_resolution(1),
            ToolchainIdentity::derive(b"toolchain-a"),
            CompilerEvidenceFingerprint::derive(b"evidence-a"),
        )
        .unwrap();
        let changed_source = PackageInstance::new(
            key.clone(),
            git_resolution(2),
            ToolchainIdentity::derive(b"toolchain-a"),
            CompilerEvidenceFingerprint::derive(b"evidence-a"),
        )
        .unwrap();
        let changed_toolchain = PackageInstance::new(
            key.clone(),
            git_resolution(1),
            ToolchainIdentity::derive(b"toolchain-b"),
            CompilerEvidenceFingerprint::derive(b"evidence-a"),
        )
        .unwrap();
        let changed_evidence = PackageInstance::new(
            key.clone(),
            git_resolution(1),
            ToolchainIdentity::derive(b"toolchain-a"),
            CompilerEvidenceFingerprint::derive(b"evidence-b"),
        )
        .unwrap();

        assert_eq!(base.key(), &key);
        assert_ne!(base, changed_source);
        assert_ne!(base, changed_toolchain);
        assert_ne!(base, changed_evidence);
    }

    #[test]
    fn commit_tree_and_content_each_independently_change_the_instance() {
        fn source(commit: u8, tree: u8, content: u8) -> ImmutableSourceResolution {
            ImmutableSourceResolution::git(
                GitCommitId::parse_hex(&format!("{commit:02x}").repeat(20)).unwrap(),
                GitTreeId::parse_hex(&format!("{tree:02x}").repeat(20)).unwrap(),
                SourceContentDigest::derive(&[content]),
            )
            .unwrap()
        }

        let key = PackageKey::new(
            package_name(),
            lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
        );
        let make_instance = |resolution| {
            PackageInstance::new(
                key.clone(),
                resolution,
                ToolchainIdentity::derive(b"toolchain"),
                CompilerEvidenceFingerprint::derive(b"evidence"),
            )
            .unwrap()
        };
        let base = make_instance(source(1, 2, 3));

        assert_ne!(base, make_instance(source(4, 2, 3)));
        assert_ne!(base, make_instance(source(1, 4, 3)));
        assert_ne!(base, make_instance(source(1, 2, 4)));
    }

    #[test]
    fn digest_domains_do_not_collapse() {
        let source = SourceContentDigest::derive(b"same bytes");
        let toolchain = ToolchainIdentity::derive(b"same bytes");
        let evidence = CompilerEvidenceFingerprint::derive(b"same bytes");

        assert_ne!(source.to_hex(), toolchain.to_hex());
        assert_ne!(source.to_hex(), evidence.to_hex());
        assert_ne!(toolchain.to_hex(), evidence.to_hex());
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
            ImmutableSourceResolution::git(commit, tree, SourceContentDigest::derive(b"content")),
            Err(IdentityError::GitObjectFormatMismatch)
        );
    }

    #[test]
    fn source_resolution_family_must_match_lineage() {
        let key = PackageKey::new(
            package_name(),
            lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
        );
        let result = PackageInstance::new(
            key,
            ImmutableSourceResolution::workspace(SourceContentDigest::derive(b"content")),
            ToolchainIdentity::derive(b"toolchain"),
            CompilerEvidenceFingerprint::derive(b"evidence"),
        );

        assert_eq!(result, Err(IdentityError::ResolutionLineageMismatch));
    }
}
