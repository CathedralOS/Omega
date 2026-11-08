use super::{GitWorkspaceSelectionError, GitWorkspaceSelectionLimit};

pub(crate) const MAX_BUILD_DECLARATION_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_TOTAL_BUILD_DECLARATION_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_WORKSPACE_MEMBERS: usize = 4096;

fn check_declaration_size(bytes: &[u8]) -> Result<(), GitWorkspaceSelectionError> {
    if bytes.len() > MAX_BUILD_DECLARATION_BYTES {
        return Err(GitWorkspaceSelectionError::ResourceLimit {
            limit: GitWorkspaceSelectionLimit::DeclarationBytes,
            maximum: MAX_BUILD_DECLARATION_BYTES,
            observed: bytes.len(),
        });
    }
    Ok(())
}

pub(crate) fn account_declaration_bytes(
    current: usize,
    bytes: &[u8],
) -> Result<usize, GitWorkspaceSelectionError> {
    check_declaration_size(bytes)?;
    let observed = current.saturating_add(bytes.len());
    if observed > MAX_TOTAL_BUILD_DECLARATION_BYTES {
        return Err(GitWorkspaceSelectionError::ResourceLimit {
            limit: GitWorkspaceSelectionLimit::TotalDeclarationBytes,
            maximum: MAX_TOTAL_BUILD_DECLARATION_BYTES,
            observed,
        });
    }
    Ok(observed)
}
