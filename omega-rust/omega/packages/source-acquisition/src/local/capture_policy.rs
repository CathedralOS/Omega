//! Original compiler-owned output extents excluded from mutable source capture.
//!
//! Coordinates are relative to this capture root, never semantic identities or
//! host grants. The compiler supplies its selected output and any resolved alias
//! before acquisition; exact published materializations keep their own policy.
//! Directory ancestors of owned output are retained only when traversal captures
//! an ordinary child. An empty authored ancestor and an ancestor created by an
//! earlier output therefore supply the same input inventory. Traversal still
//! retains their raw membership and identity observations for drift checks.

use crate::SourceResolveError;
use std::path::{Component, Path, PathBuf};

use crate::limits::DEFAULT_BUILD_OUTPUT_DIRECTORY;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LocalSourceCapturePolicy<'input> {
    owned_outputs: &'input [PathBuf],
}

impl<'input> LocalSourceCapturePolicy<'input> {
    pub fn new(owned_outputs: &'input [PathBuf]) -> Result<Self, SourceResolveError> {
        for path in owned_outputs {
            if path.as_os_str().is_empty()
                || path
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(SourceResolveError::InvalidOwnedOutputPath { path: path.clone() });
            }
        }
        Ok(Self { owned_outputs })
    }

    /// Membership is relative to the consuming root, including when its bytes
    /// are projected from a larger original capture. Metadata/output reservations
    /// do not acquire a different meaning at an outer filesystem mount.
    pub(crate) fn excludes_member(self, relative_path: &Path) -> bool {
        self.excludes_metadata(relative_path) || self.excludes_output(relative_path)
    }

    // Reserve lock case variants on both case-sensitive and folding hosts.
    pub(crate) fn excludes_metadata(self, relative_path: &Path) -> bool {
        relative_path
            .components()
            .any(|component| component.as_os_str() == ".git")
            || relative_path.components().next().is_some_and(|component| {
                component
                    .as_os_str()
                    .as_encoded_bytes()
                    .eq_ignore_ascii_case(b"omega.lock")
            })
    }

    pub(crate) fn excludes_output(self, relative_path: &Path) -> bool {
        self.contains_output(relative_path)
            || relative_path
                .components()
                .next()
                .is_some_and(|component| component.as_os_str() == DEFAULT_BUILD_OUTPUT_DIRECTORY)
    }

    pub(crate) fn contains_output(self, relative_path: &Path) -> bool {
        self.owned_outputs
            .iter()
            .any(|output| relative_path.starts_with(output))
    }

    pub(crate) fn is_output_ancestor(self, relative_path: &Path) -> bool {
        self.owned_outputs
            .iter()
            .any(|output| output != relative_path && output.starts_with(relative_path))
    }

    pub(crate) fn excluded_entry_allowance(self) -> usize {
        self.owned_outputs.len()
    }
}
