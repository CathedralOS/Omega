//! Syntax-neutral coordination for selecting one declared Git workspace member.

mod workspace_declaration;

pub use workspace_declaration::{
    GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits, GitWorkspaceProjectionCustody,
    GitWorkspaceProjectionError, GitWorkspaceProjectionPlanner, GitWorkspaceProjectionResult,
    GitWorkspaceSelection,
};
