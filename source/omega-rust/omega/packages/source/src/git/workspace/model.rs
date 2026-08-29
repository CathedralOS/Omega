use crate::{ResolvedGitSource, SourceRelativePath, SourceResolveError};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitWorkspaceDeclarationLimits {
    maximum_members: usize,
    maximum_declaration_bytes: u64,
    maximum_total_declaration_bytes: u64,
}

impl GitWorkspaceDeclarationLimits {
    pub const fn new(
        maximum_members: usize,
        maximum_declaration_bytes: u64,
        maximum_total_declaration_bytes: u64,
    ) -> Self {
        Self {
            maximum_members,
            maximum_declaration_bytes,
            maximum_total_declaration_bytes,
        }
    }

    pub const fn maximum_members(self) -> usize {
        self.maximum_members
    }

    pub const fn maximum_declaration_bytes(self) -> u64 {
        self.maximum_declaration_bytes
    }

    pub const fn maximum_total_declaration_bytes(self) -> u64 {
        self.maximum_total_declaration_bytes
    }
}

/// One authenticated package declaration read from an exact repository path.
///
/// These bytes remain resolver custody. They are never placed in the selected
/// member's compilation root merely to support workspace navigation replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceDeclaration {
    member_path: Option<SourceRelativePath>,
    repository_path: String,
    object_id: String,
    bytes: Vec<u8>,
}

/// Exact source-layer custody retained for one selected workspace member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceProjectionCustody {
    root_declaration: GitWorkspaceDeclaration,
    member_declarations: Vec<GitWorkspaceDeclaration>,
    selected_member_path: SourceRelativePath,
    selected_member_tree: String,
}

impl GitWorkspaceProjectionCustody {
    pub(crate) fn new(
        root_declaration: GitWorkspaceDeclaration,
        member_declarations: Vec<GitWorkspaceDeclaration>,
        selected_member_path: SourceRelativePath,
        selected_member_tree: String,
    ) -> Self {
        Self {
            root_declaration,
            member_declarations,
            selected_member_path,
            selected_member_tree,
        }
    }

    pub const fn root_declaration(&self) -> &GitWorkspaceDeclaration {
        &self.root_declaration
    }

    pub fn member_declarations(&self) -> &[GitWorkspaceDeclaration] {
        &self.member_declarations
    }

    pub const fn selected_member_path(&self) -> &SourceRelativePath {
        &self.selected_member_path
    }

    pub fn selected_member_tree(&self) -> &str {
        &self.selected_member_tree
    }
}

impl GitWorkspaceDeclaration {
    pub(crate) fn root(repository_path: String, object_id: String, bytes: Vec<u8>) -> Self {
        Self {
            member_path: None,
            repository_path,
            object_id,
            bytes,
        }
    }

    pub(crate) fn member(
        member_path: SourceRelativePath,
        repository_path: String,
        object_id: String,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            member_path: Some(member_path),
            repository_path,
            object_id,
            bytes,
        }
    }

    pub const fn member_path(&self) -> Option<&SourceRelativePath> {
        self.member_path.as_ref()
    }

    pub fn repository_path(&self) -> &str {
        &self.repository_path
    }

    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Manager-owned interpretation of source-authenticated declaration bytes.
///
/// The source resolver deliberately knows neither Omega syntax nor package
/// declaration semantics. It authenticates and opens bytes; this planner
/// discovers declared paths and selects by declared package identity.
pub trait GitWorkspaceProjectionPlanner {
    type Error;
    type Evidence;

    fn discover_members(
        &mut self,
        root_declaration: &GitWorkspaceDeclaration,
    ) -> Result<Vec<SourceRelativePath>, Self::Error>;

    fn select_member(
        &mut self,
        root_declaration: &GitWorkspaceDeclaration,
        member_declarations: &[GitWorkspaceDeclaration],
    ) -> Result<GitWorkspaceSelection<Self::Evidence>, Self::Error>;
}

#[derive(Debug)]
pub struct GitWorkspaceSelection<Evidence> {
    member_path: SourceRelativePath,
    evidence: Evidence,
}

impl<Evidence> GitWorkspaceSelection<Evidence> {
    pub fn new(member_path: SourceRelativePath, evidence: Evidence) -> Self {
        Self {
            member_path,
            evidence,
        }
    }

    pub const fn member_path(&self) -> &SourceRelativePath {
        &self.member_path
    }

    pub fn into_parts(self) -> (SourceRelativePath, Evidence) {
        (self.member_path, self.evidence)
    }
}

#[derive(Debug)]
pub struct GitWorkspaceProjectionResult<Evidence> {
    source: ResolvedGitSource,
    evidence: Evidence,
}

impl<Evidence> GitWorkspaceProjectionResult<Evidence> {
    pub(crate) fn new(source: ResolvedGitSource, evidence: Evidence) -> Self {
        Self { source, evidence }
    }

    pub const fn source(&self) -> &ResolvedGitSource {
        &self.source
    }

    pub const fn evidence(&self) -> &Evidence {
        &self.evidence
    }

    pub fn into_parts(self) -> (ResolvedGitSource, Evidence) {
        (self.source, self.evidence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitWorkspaceProjectionError<PlannerError> {
    Source(SourceResolveError),
    Planner(PlannerError),
}

impl<PlannerError: fmt::Display> fmt::Display for GitWorkspaceProjectionError<PlannerError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Planner(error) => write!(formatter, "Git workspace selection failed: {error}"),
        }
    }
}

impl<PlannerError: fmt::Debug + fmt::Display> std::error::Error
    for GitWorkspaceProjectionError<PlannerError>
{
}

impl<PlannerError> From<SourceResolveError> for GitWorkspaceProjectionError<PlannerError> {
    fn from(error: SourceResolveError) -> Self {
        Self::Source(error)
    }
}
