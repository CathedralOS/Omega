use super::{IdentityError, SourceContentDigest, SourceLineage, decode_hex, encode_hex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceLineageFamily {
    Git,
    Workspace,
    ExternalLocal,
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

    #[doc(hidden)]
    pub fn matches_lineage(&self, lineage: &SourceLineage) -> bool {
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
