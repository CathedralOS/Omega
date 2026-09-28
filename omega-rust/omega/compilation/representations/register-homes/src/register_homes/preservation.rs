use crate::RegisterHomeIdentity;
use optimization_core::{
    OptimizationWorkBudget, OptimizationWorkUsage, PostAllocationOptimizationManifestIdentity,
};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::MachineId;
use sha2::{Digest, Sha256};
use target::NativeTarget;

use register_model::FrameAbiPreservationConvention;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocatedCalleeSavedRequirementIdentity([u8; 32]);

impl AllocatedCalleeSavedRequirementIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocatedCalleeSavedRequirementPolicy {
    AllocatedSelectedWritesIntersectAbiPreservationV1,
}

/// One exact selected write that may modify the containing ABI-preserved unit.
/// A home without a selected definition never creates a witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalleeSavedModificationWitness {
    OperandDefinition {
        block: SelectedBlockId,
        instruction: SelectedInstructionId,
        operand: u16,
        virtual_register: VirtualRegisterId,
        home_view: RegisterViewId,
        write_semantics: RegisterWriteSemantics,
    },
    ImplicitDefinition {
        block: SelectedBlockId,
        instruction: SelectedInstructionId,
    },
    ImplicitClobber {
        block: SelectedBlockId,
        instruction: SelectedInstructionId,
    },
}

/// The canonical identity encoding of [`CalleeSavedModificationWitness`].
///
/// The variant tags written here — `0` for `OperandDefinition`, `1` for
/// `ImplicitDefinition`, `2` for `ImplicitClobber` — the little-endian field
/// order after each tag, and the write-semantics tag below *are* the identity:
/// the allocated callee-saved requirement identity and the non-authoritative
/// callee-save storage identity derived from it hash exactly this byte stream.
/// Changing a tag or reordering a field changes both artifacts and every
/// replay record that quotes them.
///
/// Producers must call this function rather than repeat the table. A second
/// copy is how a producer and the downstream stage that must agree with it
/// drift apart silently: both keep compiling, both keep hashing, and only a
/// rejected identity ever reveals the divergence.
pub fn encode_callee_saved_modification_witness_identity(
    hasher: &mut Sha256,
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
            hasher.update([0]);
            hasher.update(block.0.to_le_bytes());
            hasher.update(instruction.0.to_le_bytes());
            hasher.update(operand.to_le_bytes());
            hasher.update(virtual_register.0.to_le_bytes());
            hasher.update(home_view.0.to_le_bytes());
            hasher.update([write_semantics_identity_tag(write_semantics)]);
        }
        CalleeSavedModificationWitness::ImplicitDefinition { block, instruction } => {
            hasher.update([1]);
            hasher.update(block.0.to_le_bytes());
            hasher.update(instruction.0.to_le_bytes());
        }
        CalleeSavedModificationWitness::ImplicitClobber { block, instruction } => {
            hasher.update([2]);
            hasher.update(block.0.to_le_bytes());
            hasher.update(instruction.0.to_le_bytes());
        }
    }
}

/// The write-semantics byte embedded in a witness identity. It is part of the
/// encoding above and shares its stability contract.
fn write_semantics_identity_tag(value: RegisterWriteSemantics) -> u8 {
    match value {
        RegisterWriteSemantics::ExactView => 0,
        RegisterWriteSemantics::PreservesUnwritten => 1,
        RegisterWriteSemantics::ZeroExtendsParent => 2,
        RegisterWriteSemantics::ZeroExtendsWithinUnit => 3,
        RegisterWriteSemantics::Discards => 4,
        RegisterWriteSemantics::InstructionDefined => 5,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedCalleeSavedUnitRequirement {
    pub unit: RegisterUnitId,
    /// Selected traversal order, with operand definitions before implicit
    /// definitions and clobbers at each instruction.
    pub witnesses: Vec<CalleeSavedModificationWitness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAllocatedCalleeSavedRequirements {
    pub machine: MachineId,
    pub modified_units: Vec<AllocatedCalleeSavedUnitRequirement>,
}

/// Allocation-visible requirements only. This plan chooses no save/restore
/// operation, stack slot, frame coordinate, unwind row, fault behavior,
/// encoding, emission, or publication artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedCalleeSavedRequirementPlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub homes: RegisterHomeIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub target: NativeTarget,
    pub abi: FrameAbiPreservationConvention,
    pub callee_saved_units: Vec<RegisterUnitId>,
    pub policy: AllocatedCalleeSavedRequirementPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionAllocatedCalleeSavedRequirements>,
}
