//! Reading and conservatively editing the package manifest in `build.omg`.

pub(crate) mod declaration;
pub(crate) mod dependency_edit;
pub(crate) mod dependency_projection;

pub use declaration::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    PackageDeclaration, PackageDeclarationError, WorkspaceDeclaration, extract_build_declaration,
    extract_package_declaration,
};
pub use dependency_edit::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildFileReplacement, canonical_dependency_statement,
    plan_dependency_addition, plan_dependency_replacement,
};
pub use dependency_projection::{
    BuildDependencyProjection, DependencyProjectionError, DependencySourceRequest,
    extract_build_dependency_projection, extract_dependency_projection,
};
