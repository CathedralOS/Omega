//! Execute a verified repository catalog before selecting one Product source.
//! Acquisition supplies immutable code; the application returns declarations
//! from its compiler. Unselected members supply catalog observations only.

use super::build_resolution::{
    AcquiredBuildDependency, BuildSourceAcquisition, ExecutedSourceResolution, PendingBuildSource,
};
use super::{
    AcquiredBuildSource, BuildSourceAcquisitions, GitPackageSourceRequest, PackageSourceNavigation,
    PackageSourceSelectionEvidence,
};
use crate::declarations::dependencies::read::PackageSelection;
use crate::resolution::graph::PackageRootSourceRequest;
use crate::resolution::graph::reconcile::PendingSourceResolution;
use build_declarations::{BuildDeclaration, WorkspaceMemberPath};
use compiler::SourceBuildDeclaration;
use package_compilation::CapturedLocalSource;
use package_source::{GitCommitId, GitTreeId, ImmutableSourceResolution, ResolvedGitSource};
use std::path::Path;

pub struct AcquiredWorkspaceBuild {
    pending: PendingSourceResolution,
    source: ResolvedGitSource,
    request: GitPackageSourceRequest,
    root: AcquiredBuildSource,
}

pub struct AcquiredWorkspaceMembers {
    pending: PendingSourceResolution,
    source: ResolvedGitSource,
    request: GitPackageSourceRequest,
    root: SourceBuildDeclaration,
    members: Vec<(WorkspaceMemberPath, AcquiredBuildSource)>,
}

pub(super) struct ExecutedWorkspace {
    source: ResolvedGitSource,
    resolution: ImmutableSourceResolution,
    acquisition: package_source::GitSourceRequest,
    root: SourceBuildDeclaration,
    members: Vec<(
        WorkspaceMemberPath,
        AcquiredBuildSource,
        SourceBuildDeclaration,
    )>,
}

impl AcquiredWorkspaceBuild {
    pub fn source_capture(&self) -> &CapturedLocalSource {
        self.root.source_capture()
    }
    pub fn entry_path(&self) -> std::path::PathBuf {
        self.source_capture()
            .canonical_live_root()
            .join("build.omg")
    }

    /// The catalog must have completed its own obligations. Member code comes
    /// from exact declared subtrees of the same authenticated root capture.
    pub fn members(self, root: SourceBuildDeclaration) -> Result<AcquiredWorkspaceMembers, String> {
        if !root.is_settled() || root.source_capture() != self.root.source_capture() {
            return Err("workspace catalog return does not match its acquired source".into());
        }
        let BuildDeclaration::Workspace(workspace) = root.declaration() else {
            return Err("named package selection requires an executed workspace".into());
        };
        if workspace.members.len() > super::git::workspace::MAX_WORKSPACE_MEMBERS {
            return Err("workspace catalog exceeds its member resource ceiling".into());
        }
        let mut members = Vec::with_capacity(workspace.members.len());
        let mut entries = 0usize;
        let mut bytes = 0u64;
        for path in &workspace.members {
            let prefix = Path::new(path.as_str());
            if !root.source_capture().tree().entries().any(|(entry, node)| {
                entry == prefix && matches!(node, source::CapturedSourceNode::Directory)
            }) {
                return Err(format!(
                    "workspace member `{}` is not an authenticated directory",
                    path.as_str()
                ));
            }
            let projected = root
                .source_capture()
                .tree()
                .entries()
                .filter_map(|(entry, node)| {
                    let relative = entry.strip_prefix(prefix).ok()?;
                    if relative.as_os_str().is_empty() {
                        return None;
                    }
                    Some((relative.to_path_buf(), node.clone()))
                })
                .collect::<Vec<_>>();
            entries = entries
                .checked_add(projected.len())
                .ok_or("workspace inventory entry count overflowed")?;
            for (_, node) in &projected {
                if let source::CapturedSourceNode::File { bytes: content, .. } = node {
                    bytes = bytes
                        .checked_add(
                            u64::try_from(content.len())
                                .map_err(|_| "workspace file size overflowed")?,
                        )
                        .ok_or("workspace inventory byte count overflowed")?;
                }
            }
            if entries > self.root.limits.max_entries || bytes > self.root.limits.max_bytes {
                return Err(
                    "workspace catalog exceeds its captured-inventory resource ceiling".into(),
                );
            }
            let expected =
                source::CapturedSourceTree::new(projected).map_err(|error| error.to_string())?;
            let capture = package_source::local::operations::capture_local_source_input(
                &root.source_capture().canonical_live_root().join(prefix),
                self.root.limits,
            )
            .map_err(|error| error.to_string())?;
            if capture.tree().as_ref() != &expected {
                return Err(
                    "workspace member capture differs from its authenticated subtree".into(),
                );
            }
            members.push((
                path.clone(),
                AcquiredBuildSource {
                    capture,
                    lineage: self.root.lineage.clone(),
                    limits: self.root.limits,
                    resolution: self.root.resolution.clone(),
                    root_request: PackageRootSourceRequest::Git(self.request.clone()),
                    navigation: PackageSourceNavigation::Member(
                        super::workspace_path::source_relative_path(path),
                    ),
                    selection: PackageSourceSelectionEvidence::Root,
                },
            ));
        }
        Ok(AcquiredWorkspaceMembers {
            pending: self.pending,
            source: self.source,
            request: self.request,
            root,
            members,
        })
    }
}

impl AcquiredWorkspaceMembers {
    pub fn source_entries(
        &self,
    ) -> impl Iterator<Item = (&CapturedLocalSource, std::path::PathBuf)> {
        self.members.iter().map(|(_, source)| {
            let capture = source.source_capture();
            (
                capture,
                super::package_entry(capture.canonical_live_root(), Some(capture.tree())),
            )
        })
    }
}

impl BuildSourceAcquisitions<'_> {
    pub(super) fn acquire_workspace_member(
        &mut self,
        pending: PendingBuildSource,
        source: ResolvedGitSource,
        request: GitPackageSourceRequest,
        selected: &crate::declarations::PackageName,
    ) -> Result<BuildSourceAcquisition, String> {
        if let Some(workspace) = self.workspaces.iter().find(|workspace| {
            workspace.source.lineage() == source.lineage()
                && workspace.source.commit() == source.commit()
                && workspace.source.tree() == source.tree()
        }) {
            return workspace.select(pending, request, selected);
        }
        let root = AcquiredBuildSource::capture_git(&source, request.clone())?;
        Ok(BuildSourceAcquisition::Workspace(AcquiredWorkspaceBuild {
            pending: pending.request,
            source,
            request,
            root,
        }))
    }

    pub fn complete_workspace(
        &mut self,
        acquired: AcquiredWorkspaceMembers,
        declarations: Vec<SourceBuildDeclaration>,
    ) -> Result<BuildSourceAcquisition, String> {
        if acquired.members.len() != declarations.len()
            || acquired
                .members
                .iter()
                .zip(&declarations)
                .any(|((_, source), declaration)| {
                    source.source_capture() != declaration.source_capture()
                })
        {
            return Err("workspace observations differ from their acquired member roster".into());
        }
        let PackageSelection::Named(selected) = acquired.request.selection() else {
            return Err("workspace acquisition lost its selected package name".into());
        };
        let selected = selected.clone();
        let resolution = ImmutableSourceResolution::git(
            GitCommitId::parse_hex(acquired.source.commit()).map_err(|error| error.to_string())?,
            GitTreeId::parse_hex(acquired.source.tree()).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let workspace = ExecutedWorkspace {
            source: acquired.source,
            root: acquired.root,
            resolution,
            acquisition: acquired.request.acquisition().clone(),
            members: acquired
                .members
                .into_iter()
                .zip(declarations)
                .map(|((path, source), declaration)| (path, source, declaration))
                .collect(),
        };
        let result = workspace.select(
            PendingBuildSource {
                request: acquired.pending,
            },
            acquired.request,
            &selected,
        )?;
        self.workspaces.push(workspace);
        Ok(result)
    }

    pub(super) fn acquire_workspace_path(
        &self,
        pending: PendingBuildSource,
        location: &str,
    ) -> Result<BuildSourceAcquisition, String> {
        let requester = pending.request.requester();
        let workspace = self
            .workspaces
            .iter()
            .find(|workspace| {
                workspace.source.lineage() == requester.key().source_lineage()
                    && &workspace.resolution == requester.resolution()
            })
            .ok_or(
                "Git-relative Path has no executed workspace catalog at its repository resolution",
            )?;
        let base = match requester.navigation() {
            PackageSourceNavigation::Root => None,
            PackageSourceNavigation::Member(path) => Some(path.as_str()),
        };
        let path = super::workspace_path::normalize_member_path(base, location)?;
        let (_, _, declaration) = workspace
            .members
            .iter()
            .find(|(member, _, _)| member == &path)
            .ok_or_else(|| {
                format!(
                    "Git-relative Path selects undeclared workspace member `{}`",
                    path.as_str()
                )
            })?;
        let name = match declaration.declaration() {
            BuildDeclaration::Package(package) => &package.name,
            BuildDeclaration::Application(application) => &application.name,
            BuildDeclaration::Workspace(_) => {
                return Err("a workspace member cannot itself be a catalog".into());
            }
        };
        let selected = crate::declarations::PackageName::parse(name.as_str())
            .map_err(|error| error.to_string())?;
        let request = GitPackageSourceRequest::new(
            workspace.acquisition.clone(),
            PackageSelection::Named(selected.clone()),
        );
        workspace.select(pending, request, &selected)
    }
}

impl ExecutedWorkspace {
    fn select(
        &self,
        pending: PendingBuildSource,
        request: GitPackageSourceRequest,
        selected: &crate::declarations::PackageName,
    ) -> Result<BuildSourceAcquisition, String> {
        let selected = build_declarations::ProjectName::parse(selected.as_str())
            .map_err(|error| error.to_string())?;
        let evidence = super::GitWorkspaceSelectionEvidence::from_executed(
            &selected,
            self.root.clone(),
            self.members
                .iter()
                .map(|(_, _, declaration)| declaration.clone())
                .collect(),
        )?;
        let path = evidence.plan().selected_member_path();
        let (_, source, _) = self
            .members
            .iter()
            .find(|(member, _, _)| member == path)
            .ok_or("executed workspace lost its selected member")?;
        if let Some(custody) = pending
            .request
            .acquired_source(&source.lineage, &source.navigation)
            .cloned()
        {
            if custody.resolution() != &source.resolution
                || custody.materialization()
                    != &super::PackageSourceMaterialization::from_local(source.capture.normalized())
            {
                return Err(
                    "workspace member requests select conflicting immutable content".into(),
                );
            }
            let next = pending
                .request
                .complete::<String>(custody)
                .map_err(|error| error.to_string())?;
            return Ok(BuildSourceAcquisition::Reused(
                ExecutedSourceResolution::from_step(next),
            ));
        }
        let mut source = source.clone();
        source.root_request = PackageRootSourceRequest::Git(request);
        source.selection = PackageSourceSelectionEvidence::GitWorkspace(evidence);
        Ok(BuildSourceAcquisition::Execute(AcquiredBuildDependency {
            pending: pending.request,
            acquired: source,
        }))
    }
}
