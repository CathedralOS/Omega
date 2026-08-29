use super::{
    BuildStagedOutputTree, StagedOutputEntry, StagedOutputEntryKind, diagnostics,
    finish_commitment, retained_native_path, validate_retained_tree,
};
use psi_diagnostics::Diagnostic;

/// Reconstruct the settled one-attempt Output-directory grammar as a complete
/// staged tree. The operation lane admits exactly one fresh direct child, so
/// no parent directories or child entries can be inferred here.
pub fn replayed_empty_directory(
    relative_path: &[u8],
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    let native = retained_native_path(relative_path).map_err(|error| {
        diagnostics(format!(
            "receipted build output directory path is not canonical: {error}"
        ))
    })?;
    if native
        .parent()
        .is_some_and(|parent| !parent.as_os_str().is_empty())
    {
        return Err(diagnostics(
            "receipted build output directory must be a direct child",
        ));
    }
    let tree = finish_commitment(vec![StagedOutputEntry {
        relative_path: relative_path.to_vec(),
        kind: StagedOutputEntryKind::Directory,
    }]);
    validate_retained_tree(&tree).map_err(|error| {
        diagnostics(format!(
            "receipted build output directory failed canonical tree validation: {error}"
        ))
    })?;
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::replayed_empty_directory;

    #[test]
    fn reconstructs_one_direct_child_empty_directory() {
        let tree = replayed_empty_directory(b"generated").expect("direct-child directory");
        assert_eq!(tree.entry_count(), 1);
        assert_eq!(tree.file_bytes(), 0);
    }

    #[test]
    fn rejects_nested_or_root_directory_shapes() {
        assert!(replayed_empty_directory(b"nested/generated").is_err());
        assert!(replayed_empty_directory(b"").is_err());
    }
}
