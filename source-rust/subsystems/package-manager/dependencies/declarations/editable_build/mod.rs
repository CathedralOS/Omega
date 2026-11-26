//! Literal roles and dependency rows that package commands can edit in place.

mod declaration_errors;
mod dependency_import;
mod dependency_rows;
pub(crate) use dependency_import::{DependencyImport, helper as imported_dependency_helper};
mod direct_build_call;
mod role;
#[cfg(test)]
mod tests;

pub(crate) use declaration_errors::BuildDeclarationError;
pub(crate) use dependency_rows::{
    DEPENDENCY_DESCRIPTION_TYPE_NAME, DependencyRowError, FROM_SOURCE_HELPER_NAME,
    WITH_ALIAS_HELPER_NAME, project_dependency_rows,
};
pub(crate) use role::{
    BuildEntrySyntaxProjection, package_authored_type_name, project_build_declaration_in_entry,
    project_build_entry_syntax,
};

pub(crate) const BUILD_MACHINE_NAME: &str = "build";
pub(crate) const PACKAGE_MACHINE_NAME: &str = "package";
pub(crate) const APPLICATION_MACHINE_NAME: &str = "application";
pub(crate) const MEMBER_MACHINE_NAME: &str = "member";
pub(crate) const ARTIFACT_ONLY_MACHINE_NAME: &str = "artifact_only";
