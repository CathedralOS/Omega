//! Canonical closure validation, organized by the question being checked.

mod closure;
mod dependency;
mod root;
mod source;

pub(super) use closure::validate_subject;
pub(super) use root::canonical_root_request;
pub(super) use source::{validate_package_key, validate_source_lineage};
