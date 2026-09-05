use super::{IdentityError, SourceLineage, decode_hex, encode_hex, hash_field};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceMemberLineage {
    pub(super) workspace_identity: WorkspaceLineageIdentity,
    pub(super) member_path: SourceRelativePath,
}

impl WorkspaceMemberLineage {
    pub fn new(
        workspace_identity: WorkspaceLineageIdentity,
        member_path: SourceRelativePath,
    ) -> Self {
        Self {
            workspace_identity,
            member_path,
        }
    }

    pub fn workspace_identity(&self) -> &WorkspaceLineageIdentity {
        &self.workspace_identity
    }

    pub fn member_path(&self) -> &SourceRelativePath {
        &self.member_path
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceRelativePath(String);

impl SourceRelativePath {
    pub fn parse(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.bytes().any(|byte| byte.is_ascii_control())
            || value.split('/').any(|component| {
                component.is_empty()
                    || matches!(component, "." | "..")
                    || !component.bytes().all(is_portable_path_byte)
            })
        {
            return Err(IdentityError::InvalidSourceRelativePath);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_portable_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalLocalLineage {
    pub(super) canonical_path: String,
    pub(super) source_context: ExternalSourceContext,
}

impl ExternalLocalLineage {
    /// Payload allowance for lexical recovery's normalized `PathBuf` and its
    /// possible error path copy. Each path allows two bytes per input byte for
    /// hosts whose owned path representation uses UTF-16; the fixed error text
    /// is included. The moved canonical UTF-8 input is charged by the caller.
    #[doc(hidden)]
    pub fn recovery_owned_bytes(path: &str) -> Option<usize> {
        path.len()
            .checked_mul(4)?
            .checked_add("recovered external source path was not absolute normalized UTF-8".len())
    }

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
