/*
    We select one package from executed declarations bound to authenticated Git
    workspace inputs, retaining domain-separated SHA-256 declaration commitments
    (`BuildDeclarationCommitment`) and member selection plans
    (`GitWorkspaceSelectionPlan`) without storing package-authored source text.

    Even when storage publication gives a workspace member an independent
    physical root, `executed_selection_plan` verifies that its complete closed
    inventory equals the exact declared subtree of the catalog, enforces
    per-declaration (`MAX_BUILD_DECLARATION_BYTES`) and aggregate
    (`MAX_TOTAL_BUILD_DECLARATION_BYTES`) byte ceilings, and keeps unselected
    members out of the selected member's closure.
*/

use crate::resolution::source::PackageSourceSelectionEvidenceError;
use build_declarations::{
    BuildDeclaration, BuildDeclarationKind, ProjectName, WorkspaceMemberPath,
};
use compiler::SourceBuildDeclaration;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const BUILD_DECLARATION_COMMITMENT_DOMAIN: &[u8] = b"omega-git-workspace-build-declaration-v1";

pub(crate) const MAX_BUILD_DECLARATION_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_TOTAL_BUILD_DECLARATION_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_WORKSPACE_MEMBERS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct BuildDeclarationCommitment([u8; 32]);

impl BuildDeclarationCommitment {
    pub fn derive(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(
            u64::try_from(BUILD_DECLARATION_COMMITMENT_DOMAIN.len())
                .expect("fixed domain length fits u64")
                .to_be_bytes(),
        );
        hasher.update(BUILD_DECLARATION_COMMITMENT_DOMAIN);
        hasher.update(
            u64::try_from(bytes.len())
                .expect("host slice length fits u64")
                .to_be_bytes(),
        );
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitWorkspaceSelectionLimit {
    DeclarationBytes,
    TotalDeclarationBytes,
}

impl GitWorkspaceSelectionLimit {
    const fn description(self) -> &'static str {
        match self {
            Self::DeclarationBytes => "per-declaration byte",
            Self::TotalDeclarationBytes => "aggregate declaration byte",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitWorkspaceSelectionError {
    ResourceLimit {
        limit: GitWorkspaceSelectionLimit,
        maximum: usize,
        observed: usize,
    },
    DeclarationEvidenceChanged,
}

impl fmt::Display for GitWorkspaceSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                limit,
                maximum,
                observed,
            } => write!(
                formatter,
                "Git workspace selection exceeded its {} ceiling of {maximum} (observed {observed})",
                limit.description()
            ),
            Self::DeclarationEvidenceChanged => formatter.write_str(
                "authenticated Git workspace declaration evidence changed during replay",
            ),
        }
    }
}

impl std::error::Error for GitWorkspaceSelectionError {}

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

pub(in crate::resolution::source) fn git_workspace_declaration_limits()
-> package_source::GitWorkspaceDeclarationLimits {
    package_source::GitWorkspaceDeclarationLimits::new(
        MAX_WORKSPACE_MEMBERS,
        u64::try_from(MAX_BUILD_DECLARATION_BYTES).expect("declaration limit fits canonical u64"),
        u64::try_from(MAX_TOTAL_BUILD_DECLARATION_BYTES)
            .expect("declaration aggregate limit fits canonical u64"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildDeclarationEvidence {
    repository_path: String,
    byte_count: usize,
    commitment: BuildDeclarationCommitment,
}

impl BuildDeclarationEvidence {
    fn from_bytes(repository_path: String, bytes: &[u8]) -> Self {
        Self {
            repository_path,
            byte_count: bytes.len(),
            commitment: BuildDeclarationCommitment::derive(bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceMemberPlan {
    member_path: WorkspaceMemberPath,
    package_name: ProjectName,
    role: BuildDeclarationKind,
    declaration: BuildDeclarationEvidence,
}

impl GitWorkspaceMemberPlan {
    fn new(
        member_path: WorkspaceMemberPath,
        package_name: ProjectName,
        role: BuildDeclarationKind,
        declaration: BuildDeclarationEvidence,
    ) -> Self {
        Self {
            member_path,
            package_name,
            role,
            declaration,
        }
    }

    pub const fn member_path(&self) -> &WorkspaceMemberPath {
        &self.member_path
    }

    pub const fn package_name(&self) -> &ProjectName {
        &self.package_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceEvidence {
    workspace_declaration: BuildDeclarationEvidence,
    members: Vec<GitWorkspaceMemberPlan>,
}

impl GitWorkspaceEvidence {
    fn new(
        workspace_declaration: BuildDeclarationEvidence,
        members: Vec<GitWorkspaceMemberPlan>,
    ) -> Self {
        Self {
            workspace_declaration,
            members,
        }
    }

    pub fn members(&self) -> &[GitWorkspaceMemberPlan] {
        &self.members
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceSelectionPlan {
    selected_member_path: WorkspaceMemberPath,
    workspace: GitWorkspaceEvidence,
}

impl GitWorkspaceSelectionPlan {
    fn new(
        selected_member_path: WorkspaceMemberPath,
        workspace_declaration: BuildDeclarationEvidence,
        members: Vec<GitWorkspaceMemberPlan>,
    ) -> Self {
        Self {
            selected_member_path,
            workspace: GitWorkspaceEvidence::new(workspace_declaration, members),
        }
    }

    pub const fn selected_member_path(&self) -> &WorkspaceMemberPath {
        &self.selected_member_path
    }

    pub fn selected_member(&self) -> &GitWorkspaceMemberPlan {
        self.workspace
            .members()
            .iter()
            .find(|member| member.member_path() == self.selected_member_path())
            .expect("selection plan retains its selected declared member")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceSelectionEvidence {
    plan: GitWorkspaceSelectionPlan,
    executed: (
        compiler::SourceBuildDeclaration,
        Vec<crate::resolution::source::WorkspaceMemberObservation>,
    ),
}

impl GitWorkspaceSelectionEvidence {
    pub(crate) fn from_executed(
        selected: &ProjectName,
        root: compiler::SourceBuildDeclaration,
        members: Vec<crate::resolution::source::WorkspaceMemberObservation>,
    ) -> Result<Self, String> {
        let declarations = members
            .iter()
            .map(|member| member.declaration().clone())
            .collect::<Vec<_>>();
        let plan = executed_selection_plan(selected, &root, &declarations)?;
        Ok(Self {
            plan,
            executed: (root, members),
        })
    }

    pub const fn plan(&self) -> &GitWorkspaceSelectionPlan {
        &self.plan
    }

    pub(crate) fn observations(&self) -> &[crate::resolution::source::WorkspaceMemberObservation] {
        &self.executed.1
    }

    pub fn revalidate(&self) -> Result<(), PackageSourceSelectionEvidenceError> {
        let (root, members) = &self.executed;
        let declarations = members
            .iter()
            .map(|member| member.declaration().clone())
            .collect::<Vec<_>>();
        let replayed = executed_selection_plan(
            self.plan().selected_member().package_name(),
            root,
            &declarations,
        )
        .map_err(PackageSourceSelectionEvidenceError::executed)?;
        if replayed == self.plan {
            Ok(())
        } else {
            Err(GitWorkspaceSelectionError::DeclarationEvidenceChanged.into())
        }
    }
}

pub(crate) fn executed_selection_plan(
    selected: &ProjectName,
    root: &SourceBuildDeclaration,
    members: &[SourceBuildDeclaration],
) -> Result<GitWorkspaceSelectionPlan, String> {
    if !root.is_settled() {
        return Err("workspace catalog has not settled its output obligations".into());
    }
    let BuildDeclaration::Workspace(workspace) = root.declaration() else {
        return Err("named Git selection requires an executed workspace declaration".into());
    };
    if workspace.members.len() != members.len() || workspace.members.len() > MAX_WORKSPACE_MEMBERS {
        return Err("workspace declaration roster differs from its executed members".into());
    }
    let root_bytes = root
        .source_capture()
        .tree()
        .read_to_string(Path::new("build.omg"))
        .map_err(|error| error.to_string())?;
    let mut total =
        account_declaration_bytes(0, root_bytes.as_bytes()).map_err(|error| error.to_string())?;
    let mut plans = Vec::with_capacity(members.len());
    let mut selected_path = None;
    for (path, observed) in workspace.members.iter().zip(members) {
        let prefix = Path::new(path.as_str());
        let expected = root
            .source_capture()
            .tree()
            .entries()
            .filter_map(|(entry, node)| {
                let relative = entry.strip_prefix(prefix).ok()?;
                (!relative.as_os_str().is_empty()).then_some((relative, node))
            });
        if !expected.eq(observed.source_capture().tree().entries()) {
            return Err(
                "workspace declaration came from a different authenticated member inventory".into(),
            );
        }
        let bytes = observed
            .source_capture()
            .tree()
            .read_to_string(Path::new("build.omg"))
            .map_err(|error| error.to_string())?;
        total = account_declaration_bytes(total, bytes.as_bytes())
            .map_err(|error| error.to_string())?;
        let (name, role) = match observed.declaration() {
            BuildDeclaration::Package(package) => (&package.name, observed.declaration().kind()),
            BuildDeclaration::Application(application) => {
                (&application.name, observed.declaration().kind())
            }
            BuildDeclaration::Workspace(_) => {
                return Err("workspace members cannot recursively declare another catalog".into());
            }
        };
        if plans
            .iter()
            .any(|member: &GitWorkspaceMemberPlan| member.package_name() == name)
        {
            return Err(format!(
                "workspace repeats declared package name `{}`",
                name.as_str()
            ));
        }
        if name == selected {
            selected_path = Some(path.clone());
        }
        plans.push(GitWorkspaceMemberPlan::new(
            path.clone(),
            name.clone(),
            role,
            BuildDeclarationEvidence::from_bytes(
                format!("{}/build.omg", path.as_str()),
                bytes.as_bytes(),
            ),
        ));
    }
    let selected_path = selected_path
        .ok_or_else(|| format!("workspace has no declared package `{}`", selected.as_str()))?;
    Ok(GitWorkspaceSelectionPlan::new(
        selected_path,
        BuildDeclarationEvidence::from_bytes("build.omg".into(), root_bytes.as_bytes()),
        plans,
    ))
}
