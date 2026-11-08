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
  `FixedPrecoloredSourceSegmentHome`), framed on the wire under `OMGRAH\0\0`
  version 7.
- `AllocatedCalleeSavedRequirementPlan` records allocation-visible ABI
  preservation requirements and modification witnesses
  (`CalleeSavedModificationWitness`) without choosing stack frames or save/restore
  instructions.
- `recovery` owns pressure-recovery classifications, spill choices, and
  authenticated `FixedViewCopyPlan` rewrite artifacts (`OMGFCV\0\0` version 98).
- `post_allocation_manifest` owns the structured report
  (`PostAllocationOptimizationManifest`, `OMGPAO\0\0` version 16) at the first
  independently validated physical-home boundary.
*/

/*
Why we separate physical-home data from allocation and validation authority:

A register-home plan or recovery artifact on the wire is a candidate record,
not proof that interference, fixed-view constraints, or live-range preservation
succeeded. Keeping the data tables, canonical SHA-256 identities, and wire
envelopes in `register-homes` lets the allocation pipeline, replay harnesses,
and independent validators exchange and authenticate artifacts without coupling
data custody to the graph-coloring or spill-recovery algorithms.
*/

/*
Provenance and version boundaries:

Every artifact envelope in this crate verifies its magic header, exact wire
version, and 32-byte domain-separated SHA-256 identity before returning a
decoded record, and rejects trailing bytes or out-of-vocabulary tags closed.
*/

pub(crate) use optimization_core::digest_encoder;

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
macro_rules! decode_error {
    ($error:ty, $description:literal) => {
        $crate::decode_error_display!($error, $description);

        optimization_core::truncated_from_adapter!($error);

        optimization_core::take_error_from_adapter!($error);
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
    #[cfg(test)]
    pub(crate) use optimization_core::codec_bytes::patch_encoded;
    pub(crate) use optimization_core::codec_bytes::push_encoded;

    pub(crate) fn length(bytes: &mut Vec<u8>, value: usize, artifact: &'static str) {
        bytes.extend_from_slice(
            &u64::try_from(value)
                .unwrap_or_else(|_| panic!("{artifact} length fits u64"))
                .to_le_bytes(),
        );
    }

    pub(crate) fn encode_target(bytes: &mut Vec<u8>, target: target::NativeTarget) {
        bytes.push(match target.architecture {
            target::Architecture::X86_64 => 0,
            target::Architecture::Aarch64 => 1,
        });
        bytes.push(match target.object_format {
            target::ObjectFormat::Elf => 0,
            target::ObjectFormat::MachO => 1,
            target::ObjectFormat::Coff => 2,
        });
        bytes.extend_from_slice(
            &u64::try_from(target.pointer_size)
                .expect("terminal target pointer size fits u64")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(
            &u64::try_from(target.pointer_alignment)
                .expect("terminal target pointer alignment fits u64")
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

    pub(crate) struct Envelope {
        magic: &'static [u8; 8], // 8-byte wire magic header
        version: u32,            // Exact wire format version
    }

    impl Envelope {
        pub(crate) const fn new(magic: &'static [u8; 8], version: u32) -> Self {
            Self { magic, version }
        }

        pub(crate) fn frame(&self, identity: [u8; 32], content: &[u8]) -> Vec<u8> {
            let mut encoded =
                Vec::with_capacity(self.magic.len() + 4 + identity.len() + content.len());
            encoded.extend_from_slice(self.magic);
            encoded.extend_from_slice(&self.version.to_le_bytes());
            encoded.extend_from_slice(&identity);
            encoded.extend_from_slice(content);
            encoded
        }

        pub(crate) fn open<'a, E>(
            &self,
            encoded: &'a [u8],
            wrong_magic: impl FnOnce() -> E,
            unsupported_version: impl FnOnce(u32) -> E,
        ) -> Result<(crate::cursor::Cursor<'a>, [u8; 32]), E>
        where
            E: From<crate::cursor::Truncated> + From<crate::cursor::TakeError>,
        {
            let mut cursor = crate::cursor::Cursor::new(encoded);
            if cursor.take(self.magic.len())? != self.magic.as_slice() {
                return Err(wrong_magic());
            }
            let version = cursor.u32()?;
            if version != self.version {
                return Err(unsupported_version(version));
            }
            let identity = cursor.array()?;
            Ok((cursor, identity))
        }
    }

    pub(crate) fn decode_list<T, E>(
        cursor: &mut crate::cursor::Cursor<'_>,
        mut decode: impl FnMut(&mut crate::cursor::Cursor<'_>) -> Result<T, E>,
    ) -> Result<Vec<T>, E>
    where
        E: From<crate::cursor::Truncated> + From<crate::cursor::TakeError>,
    {
        let count = cursor.length()?;
        let mut values = Vec::with_capacity(count.min(cursor.remaining()));
        for _ in 0..count {
            values.push(decode(cursor)?);
        }
        Ok(values)
    }

    pub(crate) fn domain_identity(domain: &[u8], content: &[u8]) -> [u8; 32] {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
        digest.raw(content);
        digest.finish()
    }
}

pub(crate) mod cursor {
    pub(crate) use optimization_core::cursor::{Cursor, TakeError, Truncated};

    pub(crate) fn optional<T, E>(
        cursor: &mut Cursor<'_>,
        read: impl FnOnce(&mut Cursor<'_>) -> Result<T, E>,
        unknown_tag: impl FnOnce(u8) -> E,
    ) -> Result<Option<T>, E>
    where
        E: From<Truncated>,
    {
        match cursor.byte()? {
            0 => Ok(None),
            1 => Ok(Some(read(cursor)?)),
            tag => Err(unknown_tag(tag)),
        }
    }
}

pub mod constraints;
pub mod post_allocation_manifest;
pub mod recovery;

pub use constraints::{
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityIdentity,
    AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, EntryFixedViewTransition,
    FixedPrecoloredInterval, FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredIntervalPolicy, FixedPrecoloredRegisterSplitRequirements,
    FixedPrecoloredSourceFragmentRequirements, FixedPrecoloredSourceSegment,
    FixedPrecoloredSourceSegmentId, FixedPrecoloredSourceSegmentOpening,
    FixedPrecoloredSplitRequirementPlan, FixedPrecoloredSplitRequirementPlanIdentity,
    FixedPrecoloredSplitRequirementPolicy, FunctionAllocationLegality,
    FunctionFixedPrecoloredIntervals, FunctionFixedPrecoloredSplitRequirements,
    RegisterClassAvailability, VirtualEarlyClobberPointLegality, VirtualLegalitySpan,
    VirtualRegisterAllocationLegality, allocation_legality, allocation_legality_function_digest,
    allocation_legality_identity_from_digests, allocator_availability,
    allocator_availability_identity, fixed_precolored_interval_plan_identity,
    fixed_precolored_intervals, fixed_precolored_split_requirement_plan_identity,
    fixed_precolored_split_requirements,
};
pub use post_allocation_manifest::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
    PostAllocationUnavailableData,
};
pub use recovery::{
    FixedViewCopy, FixedViewCopyDecodeError, FixedViewCopyDestination, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, FunctionRecoveryClassification,
    FunctionSpillChoices, NoAdmittedRecoveryReason, PressureContender,
    PressureRecoveryClassification, PressureResident, RecoveryClassification,
    RecoveryClassificationDecodeError, RecoveryClassificationIdentity, RecoveryClassificationPlan,
    RecoveryClassificationPolicy, RecoveryFutureUse, RecoveryVictimRole, SpillChoice,
    SpillChoiceDecodeError, SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy,
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
    CurrentSelectedProgram, LiveRangeIdentity, LiveRangePlan, PreAllocationMachineEffectPlan,
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualFixedConstraintSite, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;

use crate::codec_bytes::{Envelope, domain_identity, length, push_encoded};

const REGISTER_HOME_ENVELOPE: Envelope = Envelope::new(b"OMGRAH\0\0", 7);

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
    pub selected: CurrentSelectedProgram<'program>, // Current selected program
    pub effects: &'program Arc<PreAllocationMachineEffectPlan>, // Pre-allocation machine effects
    pub ranges: &'program LiveRangePlan,            // Live-range plan
    pub ranges_identity: LiveRangeIdentity,         // Live-range plan identity
    pub legality: &'program AllocationLegalityPlan, // Allocation-legality plan
    pub legality_identity: AllocationLegalityIdentity, // Allocation-legality identity
    pub homes: &'program RegisterHomePlan,          // Register-home plan
    pub homes_identity: RegisterHomeIdentity,       // Register-home plan identity
    pub manifest: &'program PostAllocationOptimizationManifest, // Post-allocation manifest
}

identity_newtype! { pub struct RegisterHomeIdentity(pub(crate) [u8; 32]); }
identity_newtype! { pub struct AbstractSpillAccessConstraintPlanIdentity([u8; 32]); }

pub fn register_home_identity(plan: &RegisterHomePlan) -> RegisterHomeIdentity {
    RegisterHomeIdentity(domain_identity(
        b"omega.terminal-register-homes.v7\0",
        &encode_terminal_register_home_content(plan),
    ))
}

pub(crate) fn encode_terminal_register_home_content(plan: &RegisterHomePlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
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
    pub legality: AllocationLegalityIdentity, // Allocation-legality digest
    pub ranges: LiveRangeIdentity,            // Live-range digest
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

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_register_home_content(self);
        let identity = register_home_identity(self);
        REGISTER_HOME_ENVELOPE.frame(identity.bytes(), &content)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, RegisterHomeDecodeError> {
        let (mut cursor, identity) = REGISTER_HOME_ENVELOPE.open(
            encoded,
            || RegisterHomeDecodeError::WrongMagic,
            RegisterHomeDecodeError::UnsupportedVersion,
        )?;
        let identity = RegisterHomeIdentity::from_bytes(identity);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let functions =
            crate::codec_bytes::decode_list::<_, RegisterHomeDecodeError>(&mut cursor, |cursor| {
                let raw_machine = cursor.u64()?;
                let machine = MachineId::new(raw_machine)
                    .ok_or(RegisterHomeDecodeError::InvalidMachineId(raw_machine))?;
                let assignments = crate::codec_bytes::decode_list::<_, RegisterHomeDecodeError>(
                    &mut *cursor,
                    |cursor| {
                        Ok(VirtualRegisterHome {
                            virtual_register: VirtualRegisterId(cursor.u32()?),
                            class: RegisterClassId(cursor.u16()?),
                            view: RegisterViewId(cursor.u16()?),
                        })
                    },
                )?;
                Ok(FunctionRegisterHomes {
                    machine,
                    assignments,
                })
            })?;
        if cursor.remaining() != 0 {
            return Err(RegisterHomeDecodeError::TrailingBytes);
        }
        let plan = Self {
            legality,
            ranges,
            register_environment,
            allocator_availability,
            functions,
        };
        if register_home_identity(&plan) != identity {
            return Err(RegisterHomeDecodeError::IdentityMismatch);
        }
        Ok(plan)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterHomeDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidMachineId(u64),
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

decode_error! { RegisterHomeDecodeError, "invalid Terminal register-home encoding" }

identity_newtype! { pub struct FixedPrecoloredSegmentHomePlanIdentity(pub(crate) [u8; 32]); }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredSegmentHomePolicy {
    MostConstrainedLowestCompatibleViewV1, // Place most constrained domain into preferred viable view
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedPrecoloredHomeDomainId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSegmentHomePlan {
    pub split_requirements: crate::FixedPrecoloredSplitRequirementPlanIdentity, // Split requirements digest
    pub fixed_intervals: crate::FixedPrecoloredIntervalPlanIdentity, // Fixed intervals digest
    pub ranges: LiveRangeIdentity,                                   // Live-range digest
    pub legality: AllocationLegalityIdentity,                        // Allocation-legality digest
    pub register_environment: TargetRegisterEnvironmentIdentity,     // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity, // Allocator availability digest
    pub optimization_unit: OptimizationUnitIdentity,           // Optimization unit digest
    pub work_schedule: WorkScheduleIdentity,                   // Work schedule identity
    pub target: NativeTarget,                                  // Target architecture and format
    pub policy: FixedPrecoloredSegmentHomePolicy,              // Segment-home selection policy
    pub budget: OptimizationWorkBudget,                        // Optimization work budget
    pub usage: OptimizationWorkUsage,                          // Optimization work usage
    pub functions: Vec<FunctionFixedPrecoloredSegmentHomes>,   // Per-function segment homes
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

pub fn fixed_precolored_segment_home_plan_identity(
    plan: &FixedPrecoloredSegmentHomePlan,
) -> FixedPrecoloredSegmentHomePlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-segment-homes.v2\0");
    bytes.extend_from_slice(&plan.split_requirements.bytes());
    bytes.extend_from_slice(&plan.fixed_intervals.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    crate::codec_bytes::encode_target(&mut bytes, plan.target);
    bytes.push(match plan.policy {
        FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_segment_home_family(&mut bytes, &plan.functions);
    FixedPrecoloredSegmentHomePlanIdentity(crate::digest_encoder::content_identity(&bytes))
}

fn encode_segment_home_family(
    bytes: &mut Vec<u8>,
    functions: &[FunctionFixedPrecoloredSegmentHomes],
) {
    length(bytes, functions.len(), "fixed/precolored segment-home");
    for function in functions {
        crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
        length(
            bytes,
            function.assignments.len(),
            "fixed/precolored segment-home",
        );
        for assignment in &function.assignments {
            crate::codec_bytes::push_encoded(bytes, assignment.virtual_register.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, assignment.class.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, assignment.source_segment.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, assignment.allocation_domain.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, assignment.view.0.to_le_bytes());
        }
    }
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
    pub selected: SelectedInstructionPlanIdentity, // Selected instruction plan digest
    pub homes: RegisterHomeIdentity,               // Register-home plan digest
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity, // Manifest digest
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub physical_register_model: PhysicalRegisterModelIdentity, // Physical register model digest
    pub target: NativeTarget,                      // Target architecture and format
    pub abi: FrameAbiPreservationConvention,       // Frame ABI preservation convention
    pub callee_saved_units: Vec<RegisterUnitId>,   // ABI callee-saved register units
    pub policy: AllocatedCalleeSavedRequirementPolicy, // Requirement derivation policy
    pub budget: OptimizationWorkBudget,            // Optimization work budget
    pub usage: OptimizationWorkUsage,              // Optimization work usage
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

pub mod codec {
    pub use crate::RegisterHomeDecodeError;
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
        FixedPrecoloredSegmentHomePlanIdentity, FixedPrecoloredSegmentHomePolicy,
        FixedPrecoloredSourceSegmentHome, FunctionFixedPrecoloredSegmentHomes,
        fixed_precolored_segment_home_plan_identity,
    };
}

pub mod storage {
    pub use crate::{
        FixedPrecoloredHomeDomainId, FixedPrecoloredSegmentHomePlan,
        FixedPrecoloredSegmentHomePlanIdentity, FixedPrecoloredSegmentHomePolicy,
        FixedPrecoloredSourceSegmentHome, FunctionFixedPrecoloredSegmentHomes,
        FunctionRegisterHomes, RegisterHomePlan, VirtualRegisterHome,
        fixed_precolored_segment_home_plan_identity, fixed_precolored_segment_homes,
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
        AllocationLegalityIdentity, AllocatorAvailabilityIdentity, CalleeSavedModificationWitness,
        FunctionRegisterHomes, RegisterHomeDecodeError, RegisterHomePlan, VirtualRegisterHome,
    };
    use register_model::{
        RegisterClassId, RegisterViewId, RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
    };
    use selected_instructions::{
        LiveRangeIdentity, SelectedBlockId, SelectedInstructionId, VirtualRegisterId,
    };
    use semantic_vocabulary::MachineId;
    use sha2::Digest;

    type Mutation = fn(&mut RegisterHomePlan);

    fn plan() -> RegisterHomePlan {
        RegisterHomePlan {
            legality: AllocationLegalityIdentity::from_bytes([1; 32]),
            ranges: LiveRangeIdentity::from_bytes([2; 32]),
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
    fn retired_split_roster_wire_rejects_before_identity_comparison() {
        let identity = [
            0xe0, 0x14, 0x35, 0x59, 0x35, 0x00, 0xd5, 0x03, 0x6e, 0x3e, 0xad, 0x06, 0xff, 0xc8,
            0xc9, 0xcb, 0x96, 0xb1, 0x1b, 0x73, 0x9f, 0x35, 0x01, 0xc2, 0xd2, 0x65, 0xcd, 0x4e,
            0x8b, 0x55, 0x9a, 0xba,
        ];
        let mut expected = b"OMGRAH\0\0\x06\0\0\0".to_vec();
        expected.extend_from_slice(&identity);
        for value in [1, 2, 3, 4] {
            expected.extend_from_slice(&[value; 32]);
        }
        for value in [1_u64, 1, 1] {
            crate::codec_bytes::push_encoded(&mut expected, value.to_le_bytes());
        }
        expected.extend_from_slice(&[0, 0, 0, 0, 1, 0, 2, 0]);
        for value in [1_u64, 2, 0] {
            crate::codec_bytes::push_encoded(&mut expected, value.to_le_bytes());
        }
        assert_eq!(expected.len(), 228);
        assert_eq!(
            RegisterHomePlan::decode(&expected),
            Err(RegisterHomeDecodeError::UnsupportedVersion(6))
        );
    }

    #[test]
    fn identity_binds_every_home_domain() {
        let baseline = register_home_identity(&plan());
        assert_eq!(baseline, register_home_identity(&plan()));
        let mutations: Vec<Mutation> = vec![
            |plan| plan.legality = AllocationLegalityIdentity::from_bytes([4; 32]),
            |plan| plan.ranges = LiveRangeIdentity::from_bytes([5; 32]),
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

    #[test]
    fn canonical_home_codec_rejects_framing_and_identity_corruption() {
        let plan = plan();
        let encoded = plan.encode();
        assert_eq!(RegisterHomePlan::decode(&encoded), Ok(plan));

        let mut identity_tamper = encoded.clone();
        identity_tamper[12] ^= 1;
        assert_eq!(
            RegisterHomePlan::decode(&identity_tamper),
            Err(RegisterHomeDecodeError::IdentityMismatch)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            RegisterHomePlan::decode(&trailing),
            Err(RegisterHomeDecodeError::TrailingBytes)
        );
        assert_eq!(
            RegisterHomePlan::decode(&encoded[..encoded.len() - 1]),
            Err(RegisterHomeDecodeError::Truncated)
        );
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            RegisterHomePlan::decode(&wrong_magic),
            Err(RegisterHomeDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        crate::codec_bytes::patch_encoded(&mut wrong_version, 8, 4_u32.to_le_bytes());
        assert_eq!(
            RegisterHomePlan::decode(&wrong_version),
            Err(RegisterHomeDecodeError::UnsupportedVersion(4))
        );
        let mut invalid_machine = encoded;
        let machine_offset = 8 + 4 + 32 + (4 * 32) + 8;
        crate::codec_bytes::patch_encoded(
            &mut invalid_machine,
            machine_offset,
            0_u64.to_le_bytes(),
        );
        assert_eq!(
            RegisterHomePlan::decode(&invalid_machine),
            Err(RegisterHomeDecodeError::InvalidMachineId(0))
        );
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
