/*
Shared validated Build declarations, evaluated project requests, and the
toolchain-authored Build prelude source.

We separate two ways the compiler inspects `build.omg`:
1. Interpreted Build activations (`BuildRequest`, `BuildDependencySource`),
   which execute inside the Psi evaluator and supply authoritative project role
   and dependency requests once the build machine runs.
2. Conservative syntax projection (`syntax_projection`, `dependencies`), which
   reads direct `Build::<operation>(builder, ...)` statements from parsed syntax
   trees without evaluating helpers, control flow, or imports.

Why keep syntax projection alongside interpreted evaluation? Package-manager
editing commands (`omega add`, `omega remove`) and early workspace/package
discovery need to inspect or rewrite literal declarations and report targeted
shape diagnostics before a full build graph exists. Syntax projection never
grants acquisition, execution, or publication authority and never recovers
identity from non-canonical call shapes.
*/

/*
Module layout:
- `build_declaration`: validated project roles (`Package`, `Application`,
  `Workspace`), `ProjectName`, and `WorkspaceMemberPath`.
- `build_request`: evaluated requests emitted by the `Build` host protocol.
- `dependencies`: the canonical `Build::dependency` / `Build::build_dependency`
  registration rows that package tooling reads and rewrites.
- `direct_build_call`: the single canonical `Build::<op>(builder, ...)` call
  shape check shared across role and dependency projectors.
- `syntax_projection`: AST-level role and entry validation over `build.omg`.
- `declaration_errors`: user-facing diagnostics for every syntax projection
  refusal.
- `prelude`: the toolchain-authored `Build` API source (`build_prelude_source`),
  assembled with the shared target and optimization catalogs.
*/

#![forbid(unsafe_code)]

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
    DEPENDENCY_DESCRIPTION_TYPE_NAME, DEPENDENCY_HELPER_OWNER, DependencyOperation,
    DependencyPurpose, DependencyRow, DependencyRowError, FROM_SOURCE_HELPER_NAME,
    WITH_ALIAS_HELPER_NAME, is_dependency_call, project_dependency_rows,
};
pub use prelude::{build_prelude_source, default_build_source};
pub use syntax_projection::{
    BuildEntrySyntaxProjection, package_authored_type_name, project_build_declaration_from_source,
    project_build_declaration_in_entry, project_build_entry_syntax,
};
