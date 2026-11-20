//! Literal roles and dependency rows that package commands can edit in place.

mod declaration_errors;
mod dependency_rows;
mod direct_build_call;
mod role;
#[cfg(test)]
mod tests;

pub(crate) use declaration_errors::BuildDeclarationError;
pub(crate) use dependency_rows::{
    DEPENDENCY_DESCRIPTION_TYPE_NAME, DEPENDENCY_HELPER_OWNER, DependencyRowError,
    FROM_SOURCE_HELPER_NAME, WITH_ALIAS_HELPER_NAME, is_dependency_call, project_dependency_rows,
};
pub(crate) use role::{
    BuildEntrySyntaxProjection, package_authored_type_name, project_build_declaration_in_entry,
    project_build_entry_syntax,
};

pub(crate) const BUILD_MACHINE_NAME: &str = "build";
pub(crate) const BUILDER_PARAMETER_NAME: &str = "builder";
pub(crate) const PACKAGE_MACHINE_NAME: &str = "package";
pub(crate) const APPLICATION_MACHINE_NAME: &str = "application";
pub(crate) const MEMBER_MACHINE_NAME: &str = "member";
pub(crate) const ARTIFACT_ONLY_MACHINE_NAME: &str = "artifact_only";
