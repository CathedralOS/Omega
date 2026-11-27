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

    pub(crate) fn option<T>(
        bytes: &mut Vec<u8>,
        value: Option<T>,
        encode: impl FnOnce(&mut Vec<u8>, T),
    ) {
        match value {
            None => bytes.push(0),
            Some(value) => {
                bytes.push(1);
                encode(bytes, value);
            }
        }
    }

    pub(crate) fn list<'a, T: 'a>(
        bytes: &mut Vec<u8>,
        values: &'a [T],
        artifact: &'static str,
        mut encode: impl FnMut(&mut Vec<u8>, &'a T),
    ) {
        length(bytes, values.len(), artifact);
        for value in values {
            encode(bytes, value);
        }
    }

    pub(crate) fn domain_identity(domain: &[u8], content: &[u8]) -> [u8; 32] {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
        digest.raw(content);
        digest.finish()
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
    PressureResident, RecoveryClassification, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, SpillChoice, SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy,
    classification, fixed_view_copy, fixed_view_copy_identity, recovery_classification_identity,
    spill_choice, spill_choice_identity,
};

use std::sync::Arc;

use optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    PostAllocationOptimizationManifestIdentity,
};
use register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId,
    RegisterViewId, RegisterWriteSemantics, Sha256RegisterDigest,
    TargetRegisterEnvironmentIdentity, write_semantics_tag,
};
use selected_instructions::{
    CurrentSelectedProgram, LiveRangePlan, PreAllocationMachineEffectPlan, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlan, VirtualFixedConstraintSite, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;

use crate::codec_bytes::{domain_identity, length, push_encoded};

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
    pub homes: &'program RegisterHomePlan,          // Register-home plan
    pub homes_identity: RegisterHomeIdentity,       // Register-home plan identity
}

identity_newtype! { pub struct RegisterHomeIdentity(pub(crate) [u8; 32]); }
identity_newtype! { pub struct AbstractSpillAccessConstraintPlanIdentity([u8; 32]); }

pub fn register_home_identity(plan: &RegisterHomePlan) -> RegisterHomeIdentity {
    RegisterHomeIdentity(domain_identity(
        b"omega.terminal-register-homes.v8\0",
        &encode_terminal_register_home_content(plan),
    ))
}

pub(crate) fn encode_terminal_register_home_content(plan: &RegisterHomePlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    length(&mut bytes, plan.functions.len(), "register-home identity");
    for function in &plan.functions {
        push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        length(
            &mut bytes,
            function.assignments.len(),
            "register-home identity",
        );
        for assignment in &function.assignments {
            push_encoded(&mut bytes, assignment.virtual_register.0.to_le_bytes());
            push_encoded(&mut bytes, assignment.class.0.to_le_bytes());
            push_encoded(&mut bytes, assignment.view.0.to_le_bytes());
        }
    }
    bytes
}

pub(crate) fn encode_fixed_constraint_site_identity(
    bytes: &mut Vec<u8>,
    site: VirtualFixedConstraintSite,
) {
    match site {
        VirtualFixedConstraintSite::Entry => bytes.push(0),
        VirtualFixedConstraintSite::Operand {
            position,
            point,
            instruction,
            operand,
            access,
        } => {
            bytes.push(1);
            push_encoded(bytes, position.0.to_le_bytes());
            push_encoded(bytes, point.0.to_le_bytes());
            push_encoded(bytes, instruction.0.to_le_bytes());
            push_encoded(bytes, operand.to_le_bytes());
            bytes.push(access.access_tag());
        }
    }
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

identity_newtype! { pub struct AllocatedCalleeSavedRequirementIdentity([u8; 32]); }

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

pub fn encode_callee_saved_modification_witness_identity<H: Sha256RegisterDigest + ?Sized>(
    hasher: &mut H,
    witness: CalleeSavedModificationWitness,
) {
    match witness {
        CalleeSavedModificationWitness::OperandDefinition {
            block,
            instruction,
            operand,
            virtual_register,
            home_view,
            write_semantics,
        } => {
            hasher.byte(0);
            hasher.raw(&block.0.to_le_bytes());
            hasher.raw(&instruction.0.to_le_bytes());
            hasher.raw(&operand.to_le_bytes());
            hasher.raw(&virtual_register.0.to_le_bytes());
            hasher.raw(&home_view.0.to_le_bytes());
            hasher.byte(write_semantics_tag(write_semantics));
        }
        CalleeSavedModificationWitness::ImplicitDefinition { block, instruction } => {
            hasher.byte(1);
            hasher.raw(&block.0.to_le_bytes());
            hasher.raw(&instruction.0.to_le_bytes());
        }
        CalleeSavedModificationWitness::ImplicitClobber { block, instruction } => {
            hasher.byte(2);
            hasher.raw(&block.0.to_le_bytes());
            hasher.raw(&instruction.0.to_le_bytes());
        }
    }
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
    pub homes: RegisterHomeIdentity,            // Register-home plan digest
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity, // Manifest digest
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
/// Later forms share this exact body and the identity its producer sealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedCalleeSavedRequirements {
    pub plan: Arc<AllocatedCalleeSavedRequirementPlan>,
    pub identity: AllocatedCalleeSavedRequirementIdentity,
}

impl AllocatedCalleeSavedRequirements {
    pub fn plan(&self) -> &AllocatedCalleeSavedRequirementPlan {
        &self.plan
    }

    pub const fn identity(&self) -> AllocatedCalleeSavedRequirementIdentity {
        self.identity
    }
}

pub fn allocated_callee_saved_requirement_identity(
    plan: &AllocatedCalleeSavedRequirementPlan,
) -> AllocatedCalleeSavedRequirementIdentity {
    let mut hasher =
        digest_encoder::DomainDigestEncoder::new(b"omega.allocated-callee-saved-requirements.v3");
    hasher.raw(&plan.homes.bytes());
    hasher.raw(&plan.post_allocation_manifest.bytes());
    hasher.raw(&plan.register_environment.bytes());
    hasher.raw(&plan.physical_register_model.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.byte(plan.abi.abi_tag());
    register_model::sha256_register_units(&mut hasher, &plan.callee_saved_units);
    hasher.byte(match plan.policy {
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1 => 0,
    });
    hasher.raw(&plan.budget.encode());
    hasher.raw(&plan.usage.encode());
    register_model::sha256_length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        register_model::sha256_length(&mut hasher, function.modified_units.len());
        for requirement in &function.modified_units {
            hasher.raw(&requirement.unit.0.to_le_bytes());
            register_model::sha256_length(&mut hasher, requirement.witnesses.len());
            for witness in &requirement.witnesses {
                encode_callee_saved_modification_witness_identity(&mut hasher, *witness);
            }
        }
    }
    AllocatedCalleeSavedRequirementIdentity::from_bytes(hasher.finish())
}

pub mod identity {
    pub use crate::{
        AbstractSpillAccessConstraintPlanIdentity, RegisterHomeIdentity, register_home_identity,
    };
}

pub mod preservation {
    pub use crate::{
        AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
        AllocatedCalleeSavedRequirementPolicy, AllocatedCalleeSavedUnitRequirement,
        CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
        encode_callee_saved_modification_witness_identity,
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

#[cfg(test)]
mod tests {
    use super::register_home_identity;
    use crate::{
        AllocatorAvailabilityIdentity, CalleeSavedModificationWitness, FunctionRegisterHomes,
        RegisterHomePlan, VirtualRegisterHome,
    };
    use register_model::{
        RegisterClassId, RegisterViewId, RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
    };
    use selected_instructions::{
        SelectedBlockId, SelectedInstructionId, VirtualRegisterId, test_support::placeholder_plan,
    };
    use semantic_vocabulary::MachineId;
    use sha2::Digest;

    type Mutation = fn(&mut RegisterHomePlan);

    fn plan() -> RegisterHomePlan {
        RegisterHomePlan {
            selected: placeholder_plan(1),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([4; 32]),
            functions: vec![
                FunctionRegisterHomes {
                    machine: MachineId::new(1).unwrap(),
                    assignments: vec![VirtualRegisterHome {
                        virtual_register: VirtualRegisterId(0),
                        class: RegisterClassId(1),
                        view: RegisterViewId(2),
                    }],
                },
                FunctionRegisterHomes {
                    machine: MachineId::new(2).unwrap(),
                    assignments: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn identity_binds_every_home_domain() {
        let baseline = register_home_identity(&plan());
        assert_eq!(baseline, register_home_identity(&plan()));
        let mutations: Vec<Mutation> = vec![
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([6; 32])
            },
            |plan| plan.allocator_availability = AllocatorAvailabilityIdentity::from_bytes([7; 32]),
            |plan| plan.functions[0].machine = MachineId::new(2).unwrap(),
            |plan| plan.functions[0].assignments[0].virtual_register = VirtualRegisterId(1),
            |plan| plan.functions[0].assignments[0].class = RegisterClassId(2),
            |plan| plan.functions[0].assignments[0].view = RegisterViewId(3),
            |plan| plan.functions[0].assignments.clear(),
            |plan| plan.functions.clear(),
            |plan| {
                plan.functions.pop();
            },
            |plan| plan.functions[1].machine = MachineId::new(3).unwrap(),
        ];
        for mutate in mutations {
            let mut changed = plan();
            mutate(&mut changed);
            assert_ne!(baseline, register_home_identity(&changed));
        }
    }

    fn witness_digest(witness: CalleeSavedModificationWitness) -> [u8; 32] {
        let mut hasher = sha2::Sha256::new();
        crate::encode_callee_saved_modification_witness_identity(&mut hasher, witness);
        hasher.finalize().into()
    }

    fn digest_of(bytes: &[u8]) -> [u8; 32] {
        let mut hasher = sha2::Sha256::new();
        hasher.update(bytes);
        hasher.finalize().into()
    }

    #[test]
    fn callee_saved_modification_witness_identity_bytes_are_pinned() {
        let operand_definition =
            |write_semantics| CalleeSavedModificationWitness::OperandDefinition {
                block: SelectedBlockId(0x0403_0201),
                instruction: SelectedInstructionId(0x0807_0605),
                operand: 0x0a09,
                virtual_register: VirtualRegisterId(0x0e0d_0c0b),
                home_view: RegisterViewId(0x100f),
                write_semantics,
            };
        let write_semantics_tags: [(RegisterWriteSemantics, u8); 6] = [
            (RegisterWriteSemantics::ExactView, 0),
            (RegisterWriteSemantics::PreservesUnwritten, 1),
            (RegisterWriteSemantics::ZeroExtendsParent, 2),
            (RegisterWriteSemantics::ZeroExtendsWithinUnit, 3),
            (RegisterWriteSemantics::Discards, 4),
            (RegisterWriteSemantics::InstructionDefined, 5),
        ];
        for (write_semantics, tag) in write_semantics_tags {
            assert_eq!(
                witness_digest(operand_definition(write_semantics)),
                digest_of(&[
                    0, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
                    0x0d, 0x0e, 0x0f, 0x10, tag,
                ]),
                "{write_semantics:?}",
            );
        }
        assert_eq!(
            witness_digest(CalleeSavedModificationWitness::ImplicitDefinition {
                block: SelectedBlockId(0x0403_0201),
                instruction: SelectedInstructionId(0x0807_0605),
            }),
            digest_of(&[1, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
        );
        assert_eq!(
            witness_digest(CalleeSavedModificationWitness::ImplicitClobber {
                block: SelectedBlockId(0x0403_0201),
                instruction: SelectedInstructionId(0x0807_0605),
            }),
            digest_of(&[2, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
        );
    }
}

mod pressure_rematerialization;
pub use pressure_rematerialization::{
    FunctionPressureRematerialization, PressureRematerializationAction,
    PressureRematerializationPlan, PressureRematerializationRewrite,
    pressure_rematerialization_identity,
};

pub mod literal_fold;
