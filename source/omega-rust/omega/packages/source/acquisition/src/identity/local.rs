use super::{IdentityError, SourceLineage, decode_hex, encode_hex, hash_field};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceMemberLineage {
    pub(super) workspace_identity: WorkspaceLineageIdentity,
    pub(super) member_path: WorkspaceMemberPath,
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
        omega_build_declarations::WorkspaceMemberPath::parse(value)
            .map_err(|_| IdentityError::InvalidWorkspaceMemberPath)?;
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<omega_build_declarations::WorkspaceMemberPath> for WorkspaceMemberPath {
    fn from(value: omega_build_declarations::WorkspaceMemberPath) -> Self {
        Self(value.into_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalLocalLineage {
    pub(super) canonical_path: String,
    pub(super) source_context: ExternalSourceContext,
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

    /// Reconstructs already-canonical external-local lineage from a bounded
    /// review baseline without consulting the current filesystem.
    ///
    /// This does not establish that the path still exists or recover source
    /// custody. It only validates the lexical invariant previously established
    /// by the source adapter.
    #[doc(hidden)]
    pub fn from_recovered_canonical_path(
        canonical_path: String,
        source_context: ExternalSourceContext,
    ) -> Result<Self, IdentityError> {
        let path = Path::new(&canonical_path);
        let normalized = path.components().collect::<PathBuf>();
        if canonical_path.is_empty()
            || !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
            || path.to_str() != Some(canonical_path.as_str())
            || normalized.as_os_str() != path.as_os_str()
        {
            return Err(IdentityError::CanonicalPath {
                path: path.to_path_buf(),
                error: "recovered external source path was not absolute normalized UTF-8"
                    .to_owned(),
            });
        }
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

impl WorkspaceLineageIdentity {
    pub(super) fn as_bytes(&self) -> &[u8] {
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
    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
