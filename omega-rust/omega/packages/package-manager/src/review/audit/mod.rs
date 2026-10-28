//! Assemble bounded audit inputs and derive deterministic review guidance.

mod source_diff;

pub(crate) use source_diff::{PackageSourcePatchError, render_package_source_patch};
