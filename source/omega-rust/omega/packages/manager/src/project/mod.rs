//! Project roles and dependency declarations read from `build.omg`.
//!
//! [`roles`] owns the selected project declaration. [`dependencies`] reads
//! exact dependency rows and plans conservative edits.

pub(crate) mod dependencies;
pub(crate) mod roles;

pub use dependencies::{
    ActiveDependencyAliasError, ActiveDependencyAliasScope, BuildDependencyEditError,
    BuildDependencyEditPlan, BuildDependencyManualPatch, BuildDependencyManualReason,
    BuildDependencyProjection, BuildFileReplacement, DependencyPathProvenance, DependencyPathTaint,
    DependencyProjectionError, DependencySourceRequest, PackageSelection, ProjectedDependencies,
    TARGET_DEPENDENCY_CONDITION_SCHEMA_VERSION, TargetDependencyColumn,
    TargetDependencyConditionSchema, canonical_dependency_statement,
    extract_build_dependency_projection, extract_dependency_projection, plan_dependency_addition,
    plan_dependency_replacement,
};
pub use roles::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    PackageDeclaration, WorkspaceDeclaration, WorkspaceMemberPath, extract_build_declaration,
    extract_package_declaration,
};
