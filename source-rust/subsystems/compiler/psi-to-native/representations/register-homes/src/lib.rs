#![forbid(unsafe_code)]

/*
We represent physical register homes, allocation-legality constraints,
recovery records, and post-allocation optimization manifests as target-bound
data structures independent of any allocating algorithm.

In our representation:
- `constraints` owns allocator availability (`AllocatorAvailabilityPlan`),
  virtual-register allocation legality (`AllocationLegalityPlan`), fixed
  precolored point intervals (`FixedPrecoloredIntervalPlan`), and fixed
  precolored split requirements (`FixedPrecoloredSplitRequirementPlan`).
- `RegisterHomePlan` and `FixedPrecoloredSegmentHomePlan` own per-function
  physical-view assignments (`VirtualRegisterHome` and
  `FixedPrecoloredSourceSegmentHome`).
- `AllocatedCalleeSavedRequirementPlan` records allocation-visible ABI
  preservation requirements and modification witnesses
  (`CalleeSavedModificationWitness`) without choosing stack frames or save/restore
  instructions.
- `recovery` owns pressure-recovery classifications, spill choices, and
  `FixedViewCopyPlan` rewrite records.
- `post_allocation_manifest` owns the structured report
  (`PostAllocationOptimizationManifest`) at the physical-home boundary.
*/

pub(crate) use ::wire::digest_encoder;

#[macro_export]
macro_rules! decode_error_display {
    ($error:ty, $description:literal) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}: {self:?}", $description)
            }
        }

        impl std::error::Error for $error {}
    };
}

#[macro_export]
macro_rules! identity_newtype {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($field_vis:vis [u8; 32]);) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis struct $name($field_vis [u8; 32]);

        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

pub(crate) mod codec_bytes {
    pub(crate) use ::wire::codec_bytes::push_encoded;

    pub(crate) fn length(bytes: &mut Vec<u8>, value: usize, artifact: &'static str) {
        bytes.extend_from_slice(
            &u64::try_from(value)
                .unwrap_or_else(|_| panic!("{artifact} length fits u64"))
                .to_le_bytes(),
        );
    }
}

pub mod constraints;
pub mod post_allocation_manifest;
pub mod publication;
pub use publication::AllocationPublicationFacts;
pub mod recovery;

pub use constraints::{
    AllocationLegalityPlan, AllocatorAvailabilityIdentity, AllocatorAvailabilityPlan,
    AllocatorAvailabilityPolicy, EntryFixedViewTransition, FixedPrecoloredInterval,
    FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPolicy,
    FixedPrecoloredRegisterSplitRequirements, FixedPrecoloredSourceFragmentRequirements,
    FixedPrecoloredSourceSegment, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPolicy, FunctionAllocationLegality,
    FunctionFixedPrecoloredIntervals, FunctionFixedPrecoloredSplitRequirements,
    RegisterClassAvailability, VirtualEarlyClobberPointLegality, VirtualLegalitySpan,
    VirtualRegisterAllocationLegality, allocation_legality, allocator_availability,
    allocator_availability_identity, fixed_precolored_intervals,
    fixed_precolored_split_requirements,
};
pub use post_allocation_manifest::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
    PostAllocationSpillStatus, PostAllocationStatistics, PostAllocationUnavailableData,
};
pub use recovery::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
    FunctionRecoveryClassification, FunctionSpillChoices, NoAdmittedRecoveryReason,
    PressureContender, PressureRecoveryClassification, PressureRematerializationPolicy,
    PressureResident, RecoveryClassification, RecoveryClassificationPlan,
    RecoveryClassificationPolicy, RecoveryFutureUse, RecoveryVictimRole, SpillChoice,
    SpillChoicePlan, SpillChoicePolicy, classification, fixed_view_copy, spill_choice,
};

use std::sync::Arc;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId,
    RegisterViewId, RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    CurrentSelectedProgram, LiveRangePlan, PreAllocationMachineEffectPlan, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlan, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedProgram {
    pub selected: Arc<SelectedInstructionPlan>, // Selected instruction plan
    pub homes: Arc<RegisterHomePlan>,           // Assigned physical register homes
}

impl AllocatedProgram {
    pub fn as_ref(&self) -> AllocatedProgramRef<'_> {
        AllocatedProgramRef {
            selected: &self.selected,
            homes: &self.homes,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AllocatedProgramRef<'program> {
    pub selected: &'program SelectedInstructionPlan, // Borrowed selected instruction plan
    pub homes: &'program RegisterHomePlan,           // Borrowed register home plan
}

#[derive(Debug, Clone, Copy)]
pub struct AllocatedProgramFacts<'program> {
    pub publication: &'program Arc<AllocationPublicationFacts>,
    pub preservation: &'program AllocatedCalleeSavedRequirements,
    pub environment: &'program register_environment::ValidatedTargetRegisterEnvironment,
    pub selected: CurrentSelectedProgram<'program>, // Current selected program
    pub effects: &'program Arc<PreAllocationMachineEffectPlan>, // Pre-allocation machine effects
    pub ranges: &'program LiveRangePlan,            // Live-range plan
    pub legality: &'program AllocationLegalityPlan, // Allocation-legality plan
    pub homes: &'program Arc<RegisterHomePlan>,     // Register-home plan
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterHomePlan {
    pub selected: Arc<SelectedInstructionPlan>, // Program whose virtual registers get these homes
    pub register_environment: TargetRegisterEnvironmentIdentity, // Target register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity,   // Allocator availability digest
    pub functions: Vec<FunctionRegisterHomes>,                   // Per-function home assignments
}

impl RegisterHomePlan {
    pub fn function(&self, machine: MachineId) -> Option<&FunctionRegisterHomes> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRegisterHomes {
    pub machine: MachineId,                    // Owning machine identity
    pub assignments: Vec<VirtualRegisterHome>, // Virtual-register home assignments
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualRegisterHome {
    pub virtual_register: VirtualRegisterId, // Virtual register identity
    pub class: RegisterClassId,              // Physical register class
    pub view: RegisterViewId,                // Assigned physical register view
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredSegmentHomePolicy {
    MostConstrainedLowestCompatibleViewV1, // Place most constrained domain into preferred viable view
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedPrecoloredHomeDomainId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSegmentHomePlan {
    pub selected: Arc<SelectedInstructionPlan>, // Program the segment homes describe
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity, // Allocator availability digest
    pub optimization_unit: OptimizationUnitIdentity, // Optimization unit digest
    pub work_schedule: WorkScheduleIdentity,    // Work schedule identity
    pub target: NativeTarget,                   // Target architecture and format
    pub policy: FixedPrecoloredSegmentHomePolicy, // Segment-home selection policy
    pub budget: OptimizationWorkBudget,         // Optimization work budget
    pub usage: OptimizationWorkUsage,           // Optimization work usage
    pub functions: Vec<FunctionFixedPrecoloredSegmentHomes>, // Per-function segment homes
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFixedPrecoloredSegmentHomes {
    pub machine: MachineId, // Owning machine identity
    pub assignments: Vec<FixedPrecoloredSourceSegmentHome>, // Source segment home assignments
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSourceSegmentHome {
    pub virtual_register: VirtualRegisterId, // Virtual register identity
    pub class: RegisterClassId,              // Physical register class
    pub source_segment: crate::FixedPrecoloredSourceSegmentId, // Source segment identity
    pub allocation_domain: FixedPrecoloredHomeDomainId, // Function-local allocation domain
    pub view: RegisterViewId,                // Assigned physical register view
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocatedCalleeSavedRequirementPolicy {
    AllocatedSelectedWritesIntersectAbiPreservationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalleeSavedModificationWitness {
    OperandDefinition {
        block: SelectedBlockId,                  // Containing block identity
        instruction: SelectedInstructionId,      // Defining instruction identity
        operand: u16,                            // Operand ordinal
        virtual_register: VirtualRegisterId,     // Defined virtual register
        home_view: RegisterViewId,               // Assigned physical register view
        write_semantics: RegisterWriteSemantics, // Physical register write semantics
    },
    ImplicitDefinition {
        block: SelectedBlockId,             // Containing block identity
        instruction: SelectedInstructionId, // Defining instruction identity
    },
    ImplicitClobber {
        block: SelectedBlockId,             // Containing block identity
        instruction: SelectedInstructionId, // Clobbering instruction identity
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedCalleeSavedUnitRequirement {
    pub unit: RegisterUnitId, // Modified physical register unit
    pub witnesses: Vec<CalleeSavedModificationWitness>, // Selected traversal witnesses
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAllocatedCalleeSavedRequirements {
    pub machine: MachineId, // Owning machine identity
    pub modified_units: Vec<AllocatedCalleeSavedUnitRequirement>, // Modified ABI-preserved units
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedCalleeSavedRequirementPlan {
    pub selected: Arc<SelectedInstructionPlan>, // Selected instruction plan
    pub homes: Arc<RegisterHomePlan>,           // Register homes the requirements read
    pub post_allocation_manifest: Arc<PostAllocationOptimizationManifest>, // Allocation manifest
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub physical_register_model: PhysicalRegisterModelIdentity, // Physical register model digest
    pub target: NativeTarget,                   // Target architecture and format
    pub abi: FrameAbiPreservationConvention,    // Frame ABI preservation convention
    pub callee_saved_units: Vec<RegisterUnitId>, // ABI callee-saved register units
    pub policy: AllocatedCalleeSavedRequirementPolicy, // Requirement derivation policy
    pub budget: OptimizationWorkBudget,         // Optimization work budget
    pub usage: OptimizationWorkUsage,           // Optimization work usage
    pub functions: Vec<FunctionAllocatedCalleeSavedRequirements>, // Per-function requirements
}

impl AllocatedCalleeSavedRequirementPlan {
    pub fn modified_units(
        &self,
    ) -> impl Iterator<Item = &AllocatedCalleeSavedUnitRequirement> + '_ {
        self.functions
            .iter()
            .flat_map(|function| &function.modified_units)
    }
}

/// The allocation's settled preservation facts, without its executable graph.
/// Later forms share this exact body and compare it structurally.
pub type AllocatedCalleeSavedRequirements = Arc<AllocatedCalleeSavedRequirementPlan>;

pub mod preservation {
    pub use crate::{
        AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
        AllocatedCalleeSavedUnitRequirement, CalleeSavedModificationWitness,
        FunctionAllocatedCalleeSavedRequirements,
    };
}

pub mod fixed_precolored_segment_homes {
    pub use crate::{
        FixedPrecoloredHomeDomainId, FixedPrecoloredSegmentHomePlan,
        FixedPrecoloredSegmentHomePolicy, FixedPrecoloredSourceSegmentHome,
        FunctionFixedPrecoloredSegmentHomes,
    };
}

pub mod storage {
    pub use crate::{
        FixedPrecoloredHomeDomainId, FixedPrecoloredSegmentHomePlan,
        FixedPrecoloredSegmentHomePolicy, FixedPrecoloredSourceSegmentHome,
        FunctionFixedPrecoloredSegmentHomes, FunctionRegisterHomes, RegisterHomePlan,
        VirtualRegisterHome, fixed_precolored_segment_homes,
    };
}

pub mod view {
    pub use crate::{AllocatedProgramFacts, AllocatedProgramRef};
}

pub mod register_homes {
    pub use crate::*;
}

mod pressure_rematerialization;
pub use pressure_rematerialization::{
    FunctionPressureRematerialization, PressureRematerializationAction,
    PressureRematerializationPlan, PressureRematerializationRewrite,
};

pub mod literal_fold;
