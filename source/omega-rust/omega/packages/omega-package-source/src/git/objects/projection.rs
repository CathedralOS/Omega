//! Exact declaration selection and independently authenticated member re-rooting.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::limits::{LocalSourceLimits, SOURCE_DEPTH_ABSOLUTE_LIMIT};

use super::authentication::{authenticate_git_tree, authenticate_git_tree_graph};
use super::batch::read_git_blobs_batch;
use super::graph::AuthenticatedGitTreeGraph;
use super::tree::{git_path_from_bytes, git_tree_invalid, validate_git_path};
use super::{GitTreeEntry, GitTreeEntryKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitTreeProjectionRequest {
    declaration_paths: Vec<Vec<u8>>,
    member_tree_path: Vec<u8>,
}

impl GitTreeProjectionRequest {
    pub(crate) fn new(
        declaration_paths: impl IntoIterator<Item = Vec<u8>>,
        member_tree_path: Vec<u8>,
    ) -> Self {
        Self {
            declaration_paths: declaration_paths.into_iter().collect(),
            member_tree_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatedGitTreeProjection {
    repository_tree_oid: String,
    declarations: Vec<GitTreeEntry>,
    member: AuthenticatedGitMemberTree,
}

impl AuthenticatedGitTreeProjection {
    pub(crate) fn repository_tree_oid(&self) -> &str {
        &self.repository_tree_oid
    }

    pub(crate) fn declarations(&self) -> &[GitTreeEntry] {
        &self.declarations
    }

    pub(crate) fn member(&self) -> &AuthenticatedGitMemberTree {
        &self.member
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatedGitMemberTree {
    source_path: Vec<u8>,
    tree_oid: String,
    entries: Vec<GitTreeEntry>,
}

impl AuthenticatedGitMemberTree {
    pub(crate) fn source_path(&self) -> &[u8] {
        &self.source_path
    }

    pub(crate) fn tree_oid(&self) -> &str {
        &self.tree_oid
    }

    pub(crate) fn entries(&self) -> &[GitTreeEntry] {
        &self.entries
    }

    /// Transfer the authenticated rows to the later materialization layer.
    /// The caller lands separately from this object-layer implementation.
    #[allow(dead_code)]
    pub(crate) fn into_entries(self) -> Vec<GitTreeEntry> {
        self.entries
    }
}

#[derive(Debug)]
pub(super) struct GitTreeProjectionPlan {
    repository_tree_oid: String,
    declaration_paths: Vec<Vec<u8>>,
    member_source_path: Vec<u8>,
    member_tree_oid: String,
    member_rows: Vec<ProjectedGitTreeEntry>,
    selected_payload_rows: Vec<GitTreeEntry>,
}

#[derive(Debug)]
struct ProjectedGitTreeEntry {
    source_path: Vec<u8>,
    projected: GitTreeEntry,
}

impl GitTreeProjectionPlan {
    pub(super) fn from_graph(
        graph: &AuthenticatedGitTreeGraph,
        request: &GitTreeProjectionRequest,
        limits: LocalSourceLimits,
    ) -> Result<Self, SourceResolveError> {
        validate_request_paths(request)?;
        let declaration_paths = select_declarations(graph, request)?;
        let (member_tree_oid, member_rows) = project_member(graph, request, limits)?;

        let mut selected_paths = BTreeSet::new();
        selected_paths.extend(declaration_paths.iter().cloned());
        selected_paths.extend(
            member_rows
                .iter()
                .filter(|row| !matches!(&row.projected.kind, GitTreeEntryKind::Tree))
                .map(|row| row.source_path.clone()),
        );
        let selected_payload_rows = selected_paths
            .into_iter()
            .map(|path| {
                graph
                    .entry(&path)
                    .expect("selected paths came from authenticated graph")
                    .clone()
            })
            .collect::<Vec<_>>();
        if selected_payload_rows.len() > limits.max_files {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }

        Ok(Self {
            repository_tree_oid: graph.root_tree_oid().to_owned(),
            declaration_paths,
            member_source_path: request.member_tree_path.clone(),
            member_tree_oid,
            member_rows,
            selected_payload_rows,
        })
    }

    pub(super) fn open_and_authenticate(
        mut self,
        executor: &GitExecutor,
        repository: &VerifiedGitRepository,
        limits: LocalSourceLimits,
    ) -> Result<AuthenticatedGitTreeProjection, SourceResolveError> {
        read_git_blobs_batch(
            executor,
            repository,
            &mut self.selected_payload_rows,
            limits,
        )?;
        super::authentication::authenticate_git_tree_payloads(
            &self.repository_tree_oid,
            &self.selected_payload_rows,
        )?;

        let opened = self
            .selected_payload_rows
            .into_iter()
            .map(|entry| (entry.relative_bytes.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let declarations = self
            .declaration_paths
            .iter()
            .map(|path| {
                opened
                    .get(path)
                    .expect("every declaration was selected for opening")
                    .clone()
            })
            .collect::<Vec<_>>();
        let member_entries = self
            .member_rows
            .into_iter()
            .map(|row| hydrate_projected_entry(row, &opened))
            .collect::<Result<Vec<_>, _>>()?;
        authenticate_git_tree(&self.member_tree_oid, &member_entries)?;

        Ok(AuthenticatedGitTreeProjection {
            repository_tree_oid: self.repository_tree_oid,
            declarations,
            member: AuthenticatedGitMemberTree {
                source_path: self.member_source_path,
                tree_oid: self.member_tree_oid,
                entries: member_entries,
            },
        })
    }
}

fn validate_request_paths(request: &GitTreeProjectionRequest) -> Result<(), SourceResolveError> {
    let validation_limits = LocalSourceLimits {
        max_depth: SOURCE_DEPTH_ABSOLUTE_LIMIT,
        ..LocalSourceLimits::default()
    };
    let mut unique = BTreeSet::new();
    for path in &request.declaration_paths {
        validate_git_path(path, validation_limits)?;
        if !unique.insert(path.as_slice()) {
            return Err(git_tree_invalid(
                path,
                "declaration path was requested more than once",
            ));
        }
    }
    if !request.member_tree_path.is_empty() {
        validate_git_path(&request.member_tree_path, validation_limits)?;
    }
    Ok(())
}

fn select_declarations(
    graph: &AuthenticatedGitTreeGraph,
    request: &GitTreeProjectionRequest,
) -> Result<Vec<Vec<u8>>, SourceResolveError> {
    request
        .declaration_paths
        .iter()
        .map(|path| {
            let entry = graph.entry(path).ok_or_else(|| {
                git_tree_invalid(
                    path,
                    "requested declaration path is absent from the Git tree",
                )
            })?;
            if !matches!(&entry.kind, GitTreeEntryKind::File { .. }) {
                return Err(git_tree_invalid(
                    path,
                    "requested declaration path is not a regular file",
                ));
            }
            Ok(path.clone())
        })
        .collect()
}

fn project_member(
    graph: &AuthenticatedGitTreeGraph,
    request: &GitTreeProjectionRequest,
    limits: LocalSourceLimits,
) -> Result<(String, Vec<ProjectedGitTreeEntry>), SourceResolveError> {
    let (tree_oid, source_prefix) = if request.member_tree_path.is_empty() {
        (graph.root_tree_oid().to_owned(), Vec::new())
    } else {
        let root = graph.entry(&request.member_tree_path).ok_or_else(|| {
            git_tree_invalid(
                &request.member_tree_path,
                "requested member tree is absent from the Git tree",
            )
        })?;
        if !matches!(&root.kind, GitTreeEntryKind::Tree) {
            return Err(git_tree_invalid(
                &request.member_tree_path,
                "requested member root is not a tree",
            ));
        }
        let mut prefix = request.member_tree_path.clone();
        prefix.push(b'/');
        (root.oid.clone(), prefix)
    };

    let mut rows = Vec::new();
    for entry in graph.entries() {
        let projected_path = if source_prefix.is_empty() {
            entry.relative_bytes.as_slice()
        } else if let Some(path) = entry.relative_bytes.strip_prefix(source_prefix.as_slice()) {
            path
        } else {
            continue;
        };
        let projected_depth = projected_path
            .split(|byte| *byte == b'/')
            .count()
            .saturating_sub(1);
        if projected_depth > limits.max_depth {
            return Err(SourceResolveError::TooDeep {
                path: git_path_from_bytes(projected_path)?,
                limit: limits.max_depth,
            });
        }
        let projected_count =
            rows.len()
                .checked_add(1)
                .ok_or(SourceResolveError::TooManyFiles {
                    limit: limits.max_files,
                })?;
        if projected_count > limits.max_files {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        let mut projected = entry.clone();
        projected.relative_bytes = projected_path.to_vec();
        projected.relative_path = git_path_from_bytes(projected_path)?;
        rows.push(ProjectedGitTreeEntry {
            source_path: entry.relative_bytes.clone(),
            projected,
        });
    }

    let projected_entries = rows
        .iter()
        .map(|row| row.projected.clone())
        .collect::<Vec<_>>();
    authenticate_git_tree_graph(&tree_oid, &projected_entries)?;
    Ok((tree_oid, rows))
}

fn hydrate_projected_entry(
    row: ProjectedGitTreeEntry,
    opened: &BTreeMap<Vec<u8>, GitTreeEntry>,
) -> Result<GitTreeEntry, SourceResolveError> {
    if matches!(&row.projected.kind, GitTreeEntryKind::Tree) {
        return Ok(row.projected);
    }
    let source = opened.get(&row.source_path).ok_or_else(|| {
        git_tree_invalid(
            &row.source_path,
            "projected blob was not selected for authenticated opening",
        )
    })?;
    let mut projected = source.clone();
    projected.relative_bytes = row.projected.relative_bytes;
    projected.relative_path = row.projected.relative_path;
    Ok(projected)
}
