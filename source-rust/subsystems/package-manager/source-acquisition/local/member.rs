//! Project a local workspace member from the catalog's captured source inventory.
use super::resolution_observations::CapturedLocalSource;
use crate::SourceResolveError;
use crate::limits::CANONICAL_DIRECTORY_MODE;
use crate::tree::ResolvedLocalSource;
use crate::tree::capture::{
    SourceTreePolicy, canonical_relative_path_bytes, excluded_from_capture,
};
use crate::tree::filesystem::{io_error, raw_os_bytes};
use crate::tree::identity::SourceIdentityHasher;
use source_capture::{CapturedSourceNode, CapturedSourceTree};
use std::path::Path;
use std::sync::Arc;

impl CapturedLocalSource {
    /// The member uses its ordinary package identity and local package metadata exclusions.
    /// All source bytes, executable bits and link targets come from this frozen tree.
    pub fn workspace_member(
        &self,
        member: &Path,
        selection: &source_selection::SourceCaptureRequest,
    ) -> Result<Self, SourceResolveError> {
        let root = self.canonical_live_root().join(member);
        let covers_member = selection
            .entries()
            .any(|(path, _)| std::str::from_utf8(path).is_ok_and(|path| member.starts_with(path)));
        if !covers_member
            || super::operations::selection_identity(selection) != self.selection_identity
        {
            return Err(SourceResolveError::Io {
                path: root,
                message: "checking a workspace member requires its complete directory in the catalog's captured inventory".into(),
            });
        }
        if member.as_os_str().is_empty()
            || !self.tree.is_dir(member)
            || self
                .tree
                .canonicalize(member)
                .map_err(|error| io_error(&root, error))?
                != member
        {
            return Err(SourceResolveError::NotDirectory { path: root });
        }
        let mut entries = Vec::new();
        for (path, node) in self.tree.entries() {
            let Ok(path) = path.strip_prefix(member) else {
                continue;
            };
            let Some(name) = path.components().next() else {
                continue;
            };
            if !excluded_from_capture(
                SourceTreePolicy::LocalPackage,
                Path::new(""),
                name.as_os_str(),
            ) {
                entries.push((
                    canonical_relative_path_bytes(path),
                    path.to_owned(),
                    node.clone(),
                ));
            }
        }
        entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut identity = SourceIdentityHasher::new(entries.len());
        let mut file_count = 0;
        for (path, _, node) in &entries {
            match node {
                CapturedSourceNode::Directory => {
                    identity.add_directory(path, CANONICAL_DIRECTORY_MODE)
                }
                CapturedSourceNode::File { bytes, executable } => {
                    identity.add_file(path, *executable, bytes)?;
                    file_count += 1;
                }
                CapturedSourceNode::Symlink { target } => {
                    identity.add_symlink(path, &raw_os_bytes(target.as_os_str()));
                    file_count += 1;
                }
            }
        }
        let tree = CapturedSourceTree::new(entries.into_iter().map(|(_, path, node)| (path, node)))
            .map_err(|error| io_error(&root, error))?;
        for (path, node) in tree.entries() {
            if matches!(node, CapturedSourceNode::Symlink { .. }) {
                tree.canonicalize(path)
                    .map_err(|error| io_error(&root.join(path), error))?;
            }
        }
        let (byte_count, content_identity) = identity.finish();
        Ok(Self {
            requested_root: root.clone(),
            normalized: ResolvedLocalSource {
                root,
                file_count,
                byte_count,
                content_identity,
            },
            tree: Arc::new(tree),
            selection_identity: super::operations::selection_identity(
                &source_selection::SourceCaptureRequest::entire_tree(),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::LocalSourceLimits;
    use crate::local::operations::capture_local_source_input;
    use std::fs;
    use std::path::Path;

    #[cfg(unix)]
    #[test]
    fn a_link_between_members_cannot_escape_the_projected_package() {
        let root = crate::test_support::temp_root("workspace-link-between-members");
        fs::create_dir_all(root.join("first")).unwrap();
        fs::create_dir_all(root.join("second")).unwrap();
        fs::write(root.join("second/source.omg"), "private sibling").unwrap();
        std::os::unix::fs::symlink("../second/source.omg", root.join("first/escaped.omg")).unwrap();
        let catalog = capture_local_source_input(&root, LocalSourceLimits::default()).unwrap();
        let result = catalog.workspace_member(
            Path::new("first"),
            &source_selection::SourceCaptureRequest::entire_tree(),
        );
        fs::remove_dir_all(&root).unwrap();
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("escapes captured root")
        );
    }

    #[test]
    fn frozen_member_retains_ordinary_identity_and_its_original_bytes() {
        let root = crate::test_support::temp_root("frozen-workspace-member");
        let member = root.join("nested/alpha");
        fs::create_dir_all(member.join("units")).unwrap();
        fs::write(member.join("units/x.omg"), "original bytes").unwrap();
        fs::write(member.join("omega.lock"), "local control").unwrap();
        fs::write(member.join("font.bin"), [0_u8, 127, 255]).unwrap();
        let catalog = capture_local_source_input(&root, LocalSourceLimits::default()).unwrap();
        let standalone = capture_local_source_input(&member, LocalSourceLimits::default()).unwrap();
        fs::remove_dir_all(&root).unwrap();
        let frozen = catalog
            .workspace_member(
                Path::new("nested/alpha"),
                &source_selection::SourceCaptureRequest::entire_tree(),
            )
            .unwrap();
        assert_eq!(frozen, standalone);
        assert!(frozen.tree().entries().any(|(path, node)| {
            path == Path::new("font.bin") && matches!(node, source_capture::CapturedSourceNode::File{bytes,..} if bytes.as_ref() == [0, 127, 255])
        }));
        assert_eq!(
            &*frozen
                .tree()
                .read_to_string(Path::new("units/x.omg"))
                .unwrap(),
            "original bytes"
        );
    }
}
