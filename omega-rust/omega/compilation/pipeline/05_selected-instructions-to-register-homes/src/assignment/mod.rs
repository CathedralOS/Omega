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

pub(crate) mod baseline;
pub(crate) mod home_assignment;
pub(crate) mod post_allocation_manifest;
pub(crate) mod recovery;
pub(crate) mod runtime_spill;
mod same_home;
pub(crate) mod transformed;
