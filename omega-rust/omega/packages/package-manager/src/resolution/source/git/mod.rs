//! Resolve one Git request into exact package source custody.
//!
//! Read in operation order: [`request`] owns the acquisition/selection input,
//! [`recorded`] acquires recorded revisions, [`workspace`] applies
//! Omega declaration semantics for named members, and [`binding`] joins the
//! selected immutable snapshot to package identity and dependency rows.

mod binding;
mod recorded;
mod request;
pub(crate) mod workspace;

pub(crate) use recorded::resolve_selected_git_source_at_revision_in_lanes;
pub(crate) use request::GitPackageSourceRequest;
