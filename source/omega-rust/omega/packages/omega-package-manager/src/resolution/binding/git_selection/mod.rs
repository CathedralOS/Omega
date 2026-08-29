//! Select one package from authenticated Git workspace declarations.
//!
//! The planner consumes declaration bytes supplied by its caller. It does not
//! navigate a filesystem, inspect Git objects, or depend on package-source
//! custody types.

mod commitment;
mod error;
mod model;
mod planner;

pub use commitment::BuildDeclarationCommitment;
pub use error::{GitWorkspaceSelectionError, GitWorkspaceSelectionLimit};
pub use model::{
    BuildDeclarationEvidence, GitWorkspaceDiscovery, GitWorkspaceEvidence, GitWorkspaceMemberBuild,
    GitWorkspaceMemberPlan, GitWorkspaceSelectionPlan,
};
pub use omega_build_declarations::{ProjectName as PackageName, WorkspaceMemberPath};
pub(crate) use planner::account_declaration_bytes;
pub use planner::{
    MAX_BUILD_DECLARATION_BYTES, MAX_TOTAL_BUILD_DECLARATION_BYTES, MAX_WORKSPACE_MEMBERS,
    discover_git_workspace, plan_git_workspace_selection,
};

#[cfg(test)]
mod tests;
