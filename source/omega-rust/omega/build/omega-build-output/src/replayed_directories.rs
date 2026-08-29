use super::{
    BuildStagedOutputTree, MAX_STAGED_OUTPUT_ENTRIES, ReplayedBuildOutputEntry, diagnostics,
    replayed_output_tree,
};
use psi_diagnostics::Diagnostic;

/// Reconstruct the bounded ordered Output-directory grammar as a complete
/// staged tree. Every nested directory must follow its exact parent; no child
/// entries or implicit directories are inferred here.
pub fn replayed_empty_directories(
    relative_paths: &[&[u8]],
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    if relative_paths.is_empty() || relative_paths.len() > MAX_STAGED_OUTPUT_ENTRIES {
        return Err(diagnostics(format!(
            "receipted build output directory tree must contain 1..={MAX_STAGED_OUTPUT_ENTRIES} entries"
        )));
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(relative_paths.len())
        .map_err(|_| {
            diagnostics("receipted build output directory allocation failed on this compiler host")
        })?;
    for relative_path in relative_paths {
        entries.push(ReplayedBuildOutputEntry::directory(relative_path));
    }
    replayed_output_tree(&entries)
}

#[cfg(test)]
mod tests {
    use super::replayed_empty_directories;

    #[test]
    fn reconstructs_an_ordered_empty_directory_tree() {
        let tree = replayed_empty_directories(&[b"generated", b"generated/nested", b"sibling"])
            .expect("ordered directory tree");
        assert_eq!(tree.entry_count(), 3);
        assert_eq!(tree.file_bytes(), 0);
    }

    #[test]
    fn rejects_missing_parent_duplicate_or_root_directory_shapes() {
        assert!(replayed_empty_directories(&[b"nested/generated"]).is_err());
        assert!(replayed_empty_directories(&[b"generated", b"generated"]).is_err());
        assert!(replayed_empty_directories(&[b""]).is_err());
    }
}
