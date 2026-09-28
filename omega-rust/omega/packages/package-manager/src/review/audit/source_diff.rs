//! Bounded, hostile-data-safe patches for source review.

mod diff;
mod output;
mod patch;
mod snapshot;

pub use patch::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    render_package_source_patch,
};
