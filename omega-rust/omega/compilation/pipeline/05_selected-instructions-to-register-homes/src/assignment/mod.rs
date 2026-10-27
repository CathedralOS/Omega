//! Optimizer module role: stage group. Register homes for admitted selected-program facts.
//!
//! `crate::register_allocation::stage_register_allocation` sequences
//! `transformed` homes after a completed selected-lowering run, the optional
//! `recovery` rules, `runtime_spill` pressure recovery, `baseline` homes over
//! the direct `home_assignment`, and the `post_allocation_manifest` every
//! route publishes. Runtime-spill recovery selects and rewrites physical
//! victims directly. The crate root re-exports each owner's public names by
//! module.

/// Every assignment context hands callers its produced homes and the
/// post-allocation manifest it was built from — one getter pair each
/// assignment-mode context used to restate.
macro_rules! assignment_context_accessors {
    () => {
        pub const fn homes(&self) -> &crate::ValidatedRegisterHomes {
            &self.homes
        }
        pub const fn post_allocation_manifest(
            &self,
        ) -> &crate::ValidatedPostAllocationOptimizationManifest {
            &self.manifest
        }
    };
}

/// The one post-home custody receipt every staging route publishes: the source
/// receipt that route retained, the homes and manifest it built over it, and
/// the two counts it published. A route whose source receipt is a different
/// type parameterizes this owner instead of re-declaring its shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostHomeCustodyReceipt<Source> {
    source: Source,
    homes: register_homes::RegisterHomeIdentity,
    post_allocation_manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl<Source> PostHomeCustodyReceipt<Source> {
    receipt_accessor_rows!(borrow PostHomeCustodyReceipt {
        source: &Source,
    });
    receipt_accessor_rows!(ref PostHomeCustodyReceipt {
        homes: register_homes::RegisterHomeIdentity,
        post_allocation_manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
        function_count: usize,
        assignment_count: usize,
    });
}

pub(crate) mod baseline;
pub(crate) mod home_assignment;
pub(crate) mod post_allocation_manifest;
pub(crate) mod recovery;
pub(crate) mod runtime_spill;
mod same_home;
pub(crate) mod transformed;
