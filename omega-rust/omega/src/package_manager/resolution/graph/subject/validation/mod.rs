//! Canonical closure validation, organized by the question being checked.

mod closure;
mod dependency;
mod root;
mod source;
mod walk;

pub(super) use closure::{validate_subject, validate_subject_with_budget};
pub(super) use root::canonical_root_request;
pub(super) use source::{validate_package_key, validate_source_lineage};
