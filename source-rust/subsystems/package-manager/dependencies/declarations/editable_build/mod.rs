//! Dependency rows that package commands can edit in place.

mod dependency_import;
mod dependency_rows;
pub(crate) use dependency_import::{DependencyImport, helper as imported_dependency_helper};
mod direct_build_call;
mod entry;
#[cfg(test)]
mod tests;

pub(crate) use dependency_rows::{
    DEPENDENCY_DESCRIPTION_TYPE_NAME, DependencyRowError, FROM_SOURCE_HELPER_NAME,
    WITH_ALIAS_HELPER_NAME, project_dependency_rows,
};
pub(crate) use entry::{
    BuildEntrySyntaxProjection, package_authored_type_name, project_build_entry_syntax,
};

pub(crate) const BUILD_MACHINE_NAME: &str = "build";

pub(crate) const DEPENDENCY_REGISTRATION_NAME: &str = "dependency";
