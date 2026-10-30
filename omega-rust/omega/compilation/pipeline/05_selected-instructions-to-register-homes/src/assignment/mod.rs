//! Optimizer module role: stage group. Register homes for admitted selected-program facts.
//!
//! `crate::register_allocation::stage_register_allocation` sequences
//! `transformed` homes after a completed selected-lowering run, the optional
//! `recovery` rules, `runtime_spill` pressure recovery, `baseline` homes over
//! the direct `home_assignment`, and the `post_allocation_manifest` every
//! route publishes. Runtime-spill recovery selects and rewrites physical
//! victims directly. The crate root re-exports each owner's public names by
//! module.

/// Every assignment context hands callers its produced homes, the
/// post-allocation manifest it was built from, and the retained
/// optimized-target proof input — one getter trio each assignment-mode context
/// used to restate beside its own retained-stage path.
macro_rules! assignment_context_accessors {
    ($field:ident $(. $step:ident())*) => {
        pub const fn homes(&self) -> &crate::ValidatedRegisterHomes {
            &self.homes
        }
        pub const fn post_allocation_manifest(
            &self,
        ) -> &crate::ValidatedPostAllocationOptimizationManifest {
            &self.manifest
        }
        /// The retained optimized-target proof input, kept as replay evidence;
        /// downstream custody checks compare the owner handle by identity.
        pub fn optimized_target_owner(
            &self,
        ) -> &std::sync::Arc<
            abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
        > {
            self.$field$(.$step())*.optimized_target_owner()
        }
    };
}

/// Register environment and per-pass budget from the admitted legality stage,
/// plus the optimizer selections that stage or its run owns. Each route
/// supplies the stage; a run whose selections are not on that stage passes
/// them separately.
macro_rules! admitted_stage_accessors {
    (const $vis:vis, $stage:ident $(. $step:ident())*) => {
        admitted_stage_accessors!(const $vis, $stage $(. $step())*, selections $stage $(. $step())*);
    };
    (const $vis:vis, $stage:ident $(. $step:ident())*, selections $selections:ident $(. $selection_step:ident())*) => {
        /// The register environment admitted with the source legality stage.
        /// The stage preserves it, so that environment is the current one.
        $vis const fn register_environment(
            &self,
        ) -> &register_environment::ValidatedTargetRegisterEnvironment {
            self.$stage$(.$step())*.register_environment()
        }
        /// The governing optimizer selections admitted with this assignment.
        $vis fn selections(&self) -> &optimization_core::OptimizationSelections {
            self.$selections$(.$selection_step())*.selections()
        }
        /// The per-pass work budget admitted beside the same evidence.
        $vis fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
            self.$stage$(.$step())*.budget_per_pass()
        }
    };
    (fn $vis:vis, $method:ident) => {
        /// The register environment admitted with the source legality stage.
        $vis fn register_environment(
            &self,
        ) -> &register_environment::ValidatedTargetRegisterEnvironment {
            self.$method().register_environment()
        }
        /// The governing optimizer selections admitted with this assignment.
        $vis fn selections(&self) -> &optimization_core::OptimizationSelections {
            self.$method().selections()
        }
        /// The per-pass work budget admitted beside the same evidence.
        $vis fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
            self.$method().budget_per_pass()
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
    /// The one constructor every staging route publishes through: the source
    /// receipt that route retained, the homes built over it, the manifest that
    /// admitted them, and the two counts. Routes delegate here instead of
    /// restating the same five-field shape beside their own type alias.
    pub(crate) fn new(
        source: Source,
        homes: crate::RegisterHomeValidationReceipt,
        post_allocation_manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
    ) -> Self {
        Self {
            source,
            homes: homes.identity(),
            post_allocation_manifest,
            function_count: homes.function_count(),
            assignment_count: homes.assignment_count(),
        }
    }

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
