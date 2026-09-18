#![forbid(unsafe_code)]

//! Compiler-neutral syntactic projection of the authoritative project role in
//! an Omega `build.omg`.
//!
//! Projection parses syntax but never evaluates build code, follows imports,
//! or grants build-host authority. Package management and the compiler can
//! therefore consume one role grammar without depending on each other.
//!
//! Start at `build_declaration.rs`, the root: the declaration and its validated
//! parts. `syntax_projection` reads one from source, `declaration_errors`
//! names every rejection, and `dependencies` projects the dependency rows
//! beside the role.

mod build_declaration;
mod declaration_errors;
mod dependencies;
mod syntax_projection;
#[cfg(test)]
mod tests;

pub use build_declaration::{
    ApplicationDeclaration, BUILD_FILE_NAME, BuildDeclaration, BuildDeclarationKind,
    InvalidWorkspaceMemberPath, PackageDeclaration, ProjectName, WorkspaceDeclaration,
    WorkspaceMemberPath,
};
pub use declaration_errors::BuildDeclarationError;
pub use dependencies::{
    CONDITIONAL_DEPENDENCY_CALL_NAMES, DependencyOperation, DependencyPurpose, DependencyRow,
    DependencyRowError, is_dependency_call_name, project_dependency_rows,
};
pub use syntax_projection::{
    BuildDeclarationSyntaxProjection, BuildEntrySyntaxProjection, extract_build_declaration,
    project_build_declaration_from_source, project_build_declaration_from_syntax_trees,
    project_build_declaration_in_entry, project_build_declaration_syntax,
    project_build_entry_syntax,
};
