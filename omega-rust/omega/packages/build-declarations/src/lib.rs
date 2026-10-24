#![forbid(unsafe_code)]

//! Shared Build declarations and syntactic projection of the project role in
//! an Omega `build.omg`.
//!
//! Projection parses syntax but never evaluates build code, follows imports,
//! or grants build-host authority. Package management and the compiler can
//! therefore consume one role grammar without depending on each other.
//!
//! Start at `build_declaration.rs`, the root: the declaration and its validated
//! parts. `syntax_projection` reads one from source, `declaration_errors`
//! names every rejection, and `dependencies` projects the dependency rows
//! beside the role; `direct_build_call` owns the one direct
//! `Build::<operation>(builder, ...)` call shape both projections consume.
//! `prelude` supplies the toolchain-authored Build API as ordinary source,
//! projecting optimization names from their shared catalog. These static
//! declarations confer no interpreter authority.

mod build_declaration;
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
pub use declaration_errors::BuildDeclarationError;
pub use dependencies::{
    DependencyOperation, DependencyPurpose, DependencyRow, DependencyRowError,
    is_dependency_call_name, project_dependency_rows,
};
pub use prelude::build_prelude_source;
pub use syntax_projection::{
    BuildEntrySyntaxProjection, extract_build_declaration, package_authored_type_name,
    project_build_declaration_from_source, project_build_declaration_in_entry,
    project_build_entry_syntax, read_build_file_source,
};
