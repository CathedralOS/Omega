// Syntax-neutral declaration records and size ceilings for one declared Git
// workspace member.

use crate::SourceRelativePath;

// `package-source` authenticates and reads declaration blobs from exact
// repository paths into `GitWorkspaceDeclaration` records without parsing Omega
// syntax. The package manager reads the root declaration from a
// `GitWorkspaceSession`, requests member declarations, and selects the member to
// materialize, while `GitWorkspaceProjectionCustody` retains the authenticated
// root and member declarations in resolver custody rather than copying
// workspace-level files into the projected member's compilation root.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceDeclaration {
    member_path: Option<SourceRelativePath>,
    repository_path: String,
    object_id: String,
    bytes: Vec<u8>,
}

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
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
