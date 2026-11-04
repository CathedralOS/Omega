#![forbid(unsafe_code)]

//! Shared validated Build declarations, requests, and ordinary prelude source.
//!
//! Interpreted activations supply package and dependency authority. Syntax
//! projection is confined to conservative source edits and diagnostic naming;
//! it never binds a package to acquired bytes or recovers historical identity.
//! `build_declaration` owns the records and validated parts; `dependencies`
//! shares dependency vocabulary with editing tools. `prelude` supplies the
//! compiler-authored Build API as ordinary source.

mod build_declaration;
mod build_request;
mod declaration_errors;
mod dependencies;
mod direct_build_call;
mod prelude;
mod syntax_projection;
#[cfg(test)]
mod tests;

pub use build_declaration::{
    ApplicationDeclaration, BUILD_FILE_NAME, BUILD_TYPE_NAME, BuildDeclaration,
    BuildDeclarationKind, PackageDeclaration, ProjectName, WorkspaceDeclaration,
    WorkspaceMemberPath,
};
pub use build_request::{BuildDependencySource, BuildRequest};
pub use declaration_errors::BuildDeclarationError;
pub use dependencies::{
    DependencyOperation, DependencyPurpose, DependencyRow, DependencyRowError,
    is_dependency_call_name, project_dependency_rows,
};
pub use prelude::{build_prelude_source, default_build_source};
pub use syntax_projection::{
    BuildEntrySyntaxProjection, package_authored_type_name, project_build_declaration_from_source,
    project_build_declaration_in_entry, project_build_entry_syntax,
};
