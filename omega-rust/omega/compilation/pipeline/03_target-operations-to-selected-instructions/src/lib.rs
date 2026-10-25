#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Mandatory target legalization followed by instruction selection.
//!
//! Start at [`stage_optimized_instruction_selection`] in `optimized.rs`: it
//! owns the target-register environment, runs [`legalize_target_operations`]
//! and then [`select_instructions`], each of which seals its own plan with a
//! receipt, and records their identities in one custody receipt.
//! `legalization` is the raw-target to legal-operation join, `selection` the
//! legal-operation to selected-CFG join, and `structural_inputs` the
//! input-only reconstruction of structural storage, parameter shape and
//! incoming ABI placement both consume. Owned arrivals retain those contracts
//! regardless of which operations share their function; optional elimination
//! of unused storage happens only after a correct general transport exists.
//!
//! A scalar observation is not a shared-loan formation. Its native consumers
//! reconstruct declared access, backing storage, codecs and ordered crossings;
//! they do not replace Psi's admitted compatibility with another loan rule.
//! Calls and storage projection consume target rows with their retained homes
//! and placements; construction never replays reference custody. The explicit
//! [`verify_target_operation_plan`] audit retains the independent source/target
//! checks for corpus and receiving review, off the default compile path.

/// `Display` for the crate's `<context> failed: {self:?}` error convention.
macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

mod legalization;
mod optimized;
mod selection;
mod structural_inputs;

// Receiving audits use the same native storage geometry (including reference
// words), then independently prove the selected address expression. Portable
// schema geometry intentionally cannot describe those native descriptor slots.
pub use structural_inputs::structural_reference_input::{
    PointerSlot, ReferenceSourceGeometry, RuntimeElement, reference_formation_geometry,
};

// The stage entry and its custody join.
pub use abstract_operations_to_target_operations::{
    DescriptorInputGuard, descriptor_input_guard_for_call,
};
pub use optimized::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt, stage_optimized_instruction_selection,
};
// The two joins the entry sequences.
pub(crate) use legalization::{
    LegalizationError, ValidatedLegalizedOperations, legalize_target_operations,
};
pub(crate) use selection::{SelectedInstructionError, select_instructions};
pub use selection::{ValidatedSelectedInstructions, selected_instruction_plan_identity};

pub use legalization::{
    LegalizationError as TargetOperationAuditError, verify_target_operation_plan,
};

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.

/// The nine pipeline-custody identities every staged optimized receipt
/// re-proves — terminal psi, target, entry, optimization bundle, projection,
/// manifest, optimization unit, fuel schedule, and the validated register
/// environment. Downstream stages embed this record rather than repeating
/// its fields, so a custody mint copies `upstream.core()` whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedCustodyCore {
    /// Terminal Psi identity of the staged upstream result.
    pub psi: terminal_psi::TerminalPsiIdentity,
    /// Native target of the staged upstream result.
    pub target: target::NativeTarget,
    /// Entry machine the staged result lowers.
    pub entry: semantic_vocabulary::MachineId,
    /// Optimization identity bundle of the staged result.
    pub optimization: optimization_core::OptimizationIdentityBundleIdentity,
    /// Optimized-plan projection identity of the staged result.
    pub projection: optimization_core::OptimizedAbstractPlanProjectionIdentity,
    /// Pre-physical manifest identity of the staged result.
    pub manifest: optimization_core::PrePhysicalOptimizationManifestIdentity,
    /// Optimization-unit identity of the staged result.
    pub optimization_unit: optimization_core::OptimizationUnitIdentity,
    /// Fuel-schedule identity of the staged result.
    pub fuel_schedule: semantic_vocabulary::FuelScheduleIdentity,
    /// Validated target-register-environment identity of the staged result.
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
}

/// Minted by `staging_custody_core_accessors!`: the identity getters every
/// staged receipt shares, projecting out of the embedded
/// [`StagedOptimizedCustodyCore`].
#[macro_export]
macro_rules! staging_custody_core_accessors {
    () => {
        /// Terminal Psi identity of the staged upstream result.
        pub const fn psi(self) -> TerminalPsiIdentity {
            self.core.psi
        }
        /// Native target of the staged upstream result.
        pub const fn target(self) -> target::NativeTarget {
            self.core.target
        }
        /// Entry machine the staged result lowers.
        pub const fn entry(self) -> MachineId {
            self.core.entry
        }
        /// Optimization identity bundle of the staged result.
        pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
            self.core.optimization
        }
        /// Optimized-plan projection identity of the staged result.
        pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
            self.core.projection
        }
        /// Pre-physical manifest identity of the staged result.
        pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
            self.core.manifest
        }
        /// Optimization-unit identity of the staged result.
        pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
            self.core.optimization_unit
        }
        /// Fuel-schedule identity of the staged result.
        pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
            self.core.fuel_schedule
        }
        /// Validated target-register-environment identity of the staged result.
        pub const fn register_environment(
            self,
        ) -> register_model::TargetRegisterEnvironmentIdentity {
            self.core.register_environment
        }
        /// The nine re-proven upstream identities, as one record.
        pub const fn core(self) -> $crate::StagedOptimizedCustodyCore {
            self.core
        }
    };
}
