use super::{
    BuildStagedOutputTree, MAX_STAGED_OUTPUT_ENTRIES, StagedOutputEntry, StagedOutputEntryKind,
    diagnostics, finish_commitment, retained_native_path, validate_retained_tree,
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
    for (index, relative_path) in relative_paths.iter().enumerate() {
        retained_native_path(relative_path).map_err(|error| {
            diagnostics(format!(
                "receipted build output directory path is not canonical: {error}"
            ))
        })?;
        if relative_paths[..index]
            .iter()
            .any(|prior| *prior == *relative_path)
        {
            return Err(diagnostics(
                "receipted build output directory paths must be distinct",
            ));
        }
        if let Some(separator) = relative_path.iter().rposition(|byte| *byte == b'/') {
            let parent = &relative_path[..separator];
            if !relative_paths[..index].iter().any(|prior| *prior == parent) {
                return Err(diagnostics(
                    "receipted nested build output directory must follow its exact parent",
                ));
            }
        }
        entries.push(StagedOutputEntry {
            relative_path: relative_path.to_vec(),
            kind: StagedOutputEntryKind::Directory,
        });
    }
    let tree = finish_commitment(entries);
    validate_retained_tree(&tree).map_err(|error| {
        diagnostics(format!(
            "receipted build output directory failed canonical tree validation: {error}"
        ))
    })?;
    Ok(tree)
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
