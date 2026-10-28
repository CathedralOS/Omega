//! Close a catalog's complete authenticated inventory without publishing its
//! unrelated files. Build sees the ordinary captured-source representation;
//! selected member publication keeps the session's independent package limits.

use super::{GitWorkspaceSession, source_error};
use crate::git::cache::identity::git_cache_identity;
use crate::git::executable::selection::resolver_package_controlled_roots;
use crate::git::objects::GitTreeEntryKind;
use crate::git::resolution::acquisition::with_git_repository_from_retained_cache;
use crate::git::resolution::selection::GitRevisionSelection;
use crate::limits::CANONICAL_DIRECTORY_MODE;
use crate::limits::{
    GIT_TREE_GRAPH_ENTRY_LIMIT, SOURCE_BYTE_ABSOLUTE_LIMIT, SOURCE_DEPTH_ABSOLUTE_LIMIT,
};
use crate::local::resolution_observations::CapturedLocalSource;
use crate::tree::ResolvedLocalSource;
use crate::tree::capture::{SourceTreePolicy, excluded_from_capture};
use crate::tree::identity::SourceIdentityHasher;
use crate::{LocalSourceLimits, SourceResolveError, SourceResolver};
use std::sync::Arc;

impl GitWorkspaceSession {
    /// Capture the complete immutable catalog input. Authored Build code may
    /// import local helpers or inspect assets outside any eventual member.
    /// Catalog capture has its own bounded inventory budget; it neither parses
    /// the Build nor widens a selected package's source budget.
    pub fn capture_inventory(
        &self,
        storage: &SourceResolver,
    ) -> Result<CapturedLocalSource, SourceResolveError> {
        storage.verify_path_identity()?;
        let lane = storage.git_sources();
        let controlled_roots = resolver_package_controlled_roots(&[lane.path()])?;
        let revision = self.recorded_revision()?;
        let catalog_limits = LocalSourceLimits {
            max_entries: GIT_TREE_GRAPH_ENTRY_LIMIT,
            max_bytes: SOURCE_BYTE_ABSOLUTE_LIMIT,
            max_depth: SOURCE_DEPTH_ABSOLUTE_LIMIT,
        };
        let namespace = git_cache_identity(
            self.request.locator_identity(),
            self.request.requested_revision(),
            self.request.execution_transport(),
        );
        // This directory routes an invocation-owned immutable inventory. It
        // contains no source publication and grants no permission to reopen it.
        let route = storage
            .workspace_members()
            .retain_child(&format!("catalog-{namespace}"))?
            .retain_child(&self.commit)?;
        let root = route.path().to_path_buf();
        let result = with_git_repository_from_retained_cache(
            storage.primary_git()?,
            &controlled_roots,
            &self.request,
            lane.path(),
            lane.directory(),
            catalog_limits,
            GitRevisionSelection::Recorded(&revision),
            |executor, repository, _, _, _| {
                let entries =
                    self.inspection
                        .open_catalog_inventory(executor, repository, catalog_limits)?;
                // Apply the same control/output exclusions as physical capture
                // after the retained graph and actual blob bytes authenticate.
                let entries = entries
                    .into_iter()
                    .filter(|entry| {
                        !entry.relative_path.ancestors().any(|ancestor| {
                            ancestor.file_name().is_some_and(|name| {
                                excluded_from_capture(
                                    SourceTreePolicy::LocalPackage,
                                    ancestor
                                        .parent()
                                        .unwrap_or_else(|| std::path::Path::new("")),
                                    name,
                                )
                            })
                        })
                    })
                    .collect::<Vec<_>>();
                let mut identity = SourceIdentityHasher::new(entries.len());
                let mut file_count = 0;
                let mut rows = Vec::with_capacity(entries.len());
                for entry in entries {
                    match &entry.kind {
                        GitTreeEntryKind::Tree => {
                            identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE)
                        }
                        GitTreeEntryKind::File { bytes, executable } => {
                            identity.add_file(
                                &entry.relative_bytes,
                                *executable,
                                bytes.as_slice(),
                            )?;
                            file_count += 1;
                        }
                        GitTreeEntryKind::Symlink { target_bytes } => {
                            identity.add_symlink(&entry.relative_bytes, target_bytes.as_slice());
                            file_count += 1;
                        }
                        GitTreeEntryKind::Gitlink => {
                            unreachable!("authenticated source projection rejects gitlinks")
                        }
                    }

                    let node = match entry.kind {
                        GitTreeEntryKind::Tree => source::CapturedSourceNode::Directory,
                        GitTreeEntryKind::File { executable, bytes } => {
                            source::CapturedSourceNode::File {
                                bytes: Arc::from(bytes.as_slice()),
                                executable,
                            }
                        }
                        GitTreeEntryKind::Symlink { target_bytes } => {
                            source::CapturedSourceNode::Symlink {
                                target: crate::git::objects::tree::git_path_from_bytes(
                                    target_bytes.as_slice(),
                                )?,
                            }
                        }
                        GitTreeEntryKind::Gitlink => {
                            unreachable!("authenticated source projection rejects gitlinks")
                        }
                    };
                    rows.push((entry.relative_path, node));
                }
                let tree = source::CapturedSourceTree::new(rows)
                    .map_err(|error| crate::tree::filesystem::io_error(&root, error))?;
                for (path, node) in tree.entries() {
                    if matches!(node, source::CapturedSourceNode::Symlink { .. }) {
                        tree.canonicalize(path).map_err(|error| {
                            crate::tree::filesystem::io_error(&root.join(path), error)
                        })?;
                    }
                }
                let (byte_count, content_identity) = identity.finish();
                Ok(CapturedLocalSource {
                    requested_root: root.clone(),
                    normalized: ResolvedLocalSource {
                        root: root.clone(),
                        file_count,
                        byte_count,
                        content_identity,
                    },
                    tree: Arc::new(tree),
                })
            },
            |capture, _, _| Ok(capture),
        );
        route.verify_path_identity()?;
        storage.verify_path_identity()?;
        result.map_err(source_error)
    }
}
