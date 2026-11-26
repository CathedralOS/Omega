//! Values returned by an executed Build and the toolchain vocabulary it receives.

#![forbid(unsafe_code)]

mod build_declaration;
mod build_request;
mod dependencies;
mod prelude;
#[cfg(test)]
mod tests;

pub use build_declaration::{
    ApplicationDeclaration, BUILD_FILE_NAME, BUILD_TYPE_NAME, BuildDeclaration,
    BuildDeclarationKind, PackageDeclaration, ProjectName, WorkspaceDeclaration,
    WorkspaceMemberPath,
};
pub use build_request::{BuildDependencySource, BuildEntryPath, BuildRequest};
pub use dependencies::{DependencyOperation, DependencyPurpose};
pub use prelude::{BuildPrelude, build_file, build_prelude};
