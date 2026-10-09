//! Assemble bounded audit inputs and derive deterministic review guidance.

mod input;
mod source_diff;
mod triage;

pub(crate) use source_diff::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    render_package_source_patch,
};
