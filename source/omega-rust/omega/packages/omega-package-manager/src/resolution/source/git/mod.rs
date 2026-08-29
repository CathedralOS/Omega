//! Resolve one Git request into exact package source custody.
//!
//! Read in operation order: [`request`] owns the acquisition/selection input,
//! [`resolution`] delegates hostile source acquisition, [`workspace`] applies
//! Omega declaration semantics for named members, and [`binding`] joins the
//! selected immutable snapshot to package identity and dependency rows.

mod binding;
mod request;
mod resolution;
pub(crate) mod workspace;

pub use request::GitPackageSourceRequest;
#[cfg(test)]
pub(crate) use resolution::resolve_git_package_source;
pub(crate) use resolution::resolve_selected_git_package_source_from_pin_in_lanes;
pub(crate) use resolution::resolve_selected_git_project_source_from_pin_in_lanes;
pub use resolution::{
    resolve_git_package_source_with_storage, resolve_selected_git_package_source_with_storage,
    resolve_selected_git_project_source_with_storage,
};
