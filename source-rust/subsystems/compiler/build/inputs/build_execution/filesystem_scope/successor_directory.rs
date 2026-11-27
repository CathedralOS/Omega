//! Convert the returned Build's issued directory into one complete immutable source acquisition.
use crate::CapturedLocalSource;
use build_output::{BuildStagedOutputTree, CapturedBuildSourceInput, CapturedSourceEntry};
use source_acquisition::capture::CapturedSourceNode;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct SuccessorDirectory {
    pub(super) relative: Vec<u8>,
    pub(super) captured: Option<CapturedBuildSourceInput>,
}

impl SuccessorDirectory {
    /// Called after normal return and dependency joins, when the staging observation is final.
    pub fn freeze(
        self,
        parent: &Path,
        staging: Option<&BuildStagedOutputTree>,
    ) -> Result<CapturedLocalSource, String> {
        let directory = logical_path(&self.relative)?;
        let root = if directory.as_os_str().is_empty() {
            parent.to_owned()
        } else {
            parent.join(directory)
        };
        let nodes = match self.captured {
            Some(input) => {
                if !self.relative.is_empty()
                    && !matches!(
                        input.retained_entry(&self.relative),
                        Some(CapturedSourceEntry::Directory)
                    )
                {
                    return Err(format!(
                        "successor package_root is not a captured directory: {}",
                        self.relative.escape_ascii()
                    ));
                }
                input
                    .retained_entries()
                    .filter_map(|(path, entry)| {
                        let path = if self.relative.is_empty() {
                            path
                        } else {
                            path.strip_prefix(self.relative.as_slice())?
                                .strip_prefix(b"/")?
                        };
                        Some(node(path, entry.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            None => staging
                .ok_or("successor package_root lost its private staging observation")?
                .select_directory(&self.relative)
                .map_err(|rows| diagnostics::join_messages(&rows))?
                .into_source_entries()
                .map(|(path, entry)| node(&path, entry))
                .collect::<Result<Vec<_>, _>>()?,
        };
        CapturedLocalSource::from_frozen_entries(root, nodes).map_err(|error| error.to_string())
    }
}

fn node(path: &[u8], entry: CapturedSourceEntry) -> Result<(PathBuf, CapturedSourceNode), String> {
    let node = match entry {
        CapturedSourceEntry::Directory => CapturedSourceNode::Directory,
        CapturedSourceEntry::File {
            bytes, executable, ..
        } => CapturedSourceNode::File { bytes, executable },
        CapturedSourceEntry::Symlink { target, .. } => CapturedSourceNode::Symlink {
            target: logical_path(&target)?,
        },
    };
    Ok((logical_path(path)?, node))
}

fn logical_path(bytes: &[u8]) -> Result<PathBuf, String> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(std::ffi::OsStr::from_bytes(bytes).into())
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(bytes)
            .map(PathBuf::from)
            .map_err(|_| "source directory contains an unrepresentable pathname".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_acquisition::capture::CapturedSourceTree;

    #[test]
    fn a_captured_directory_shares_only_its_own_entries() {
        let file = |bytes: &'static [u8]| CapturedSourceNode::File {
            bytes: bytes.into(),
            executable: false,
        };
        let tree = CapturedSourceTree::new([
            ("board".into(), CapturedSourceNode::Directory),
            ("board/settings".into(), file(b"R")),
            ("secret".into(), file(b"S")),
            (
                "alias".into(),
                CapturedSourceNode::Symlink {
                    target: "board".into(),
                },
            ),
        ])
        .unwrap();
        let input = crate::source::source_snapshot::retained_source_input(&tree).unwrap();
        let parent = std::env::temp_dir().join("omega-captured-successor");
        let select = |relative: &[u8]| {
            SuccessorDirectory {
                relative: relative.into(),
                captured: Some(input.clone()),
            }
            .freeze(&parent, None)
        };
        let selected = select(b"board").unwrap();
        assert_eq!(selected.canonical_live_root(), parent.join("board"));
        assert_eq!(selected.tree().entries().count(), 1);
        assert_eq!(
            &*selected
                .tree()
                .read_to_string(Path::new("settings"))
                .unwrap(),
            "R"
        );
        assert!(select(b"board/settings").is_err());
        assert!(select(b"alias").is_err());
        assert!(select(b"missing").is_err());
        assert_eq!(
            select(b"").unwrap().canonical_live_root().as_os_str(),
            parent.as_os_str()
        );
    }
}
