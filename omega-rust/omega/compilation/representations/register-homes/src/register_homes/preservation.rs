use crate::RegisterHomeIdentity;
use optimization_core::{
    OptimizationWorkBudget, OptimizationWorkUsage, PostAllocationOptimizationManifestIdentity,
};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity, write_semantics_tag,
};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

use register_model::FrameAbiPreservationConvention;

crate::identity_newtype! { pub struct AllocatedCalleeSavedRequirementIdentity([u8; 32]); }

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
pub fn encode_callee_saved_modification_witness_identity<
    H: register_model::Sha256RegisterDigest + ?Sized,
>(
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

/// The write-semantics byte embedded in a witness identity. It is part of the
/// encoding above and shares its stability contract.

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

impl AllocatedCalleeSavedRequirementPlan {
    /// Every function's allocated unit requirements, in function order.
    pub fn modified_units(
        &self,
    ) -> impl Iterator<Item = &AllocatedCalleeSavedUnitRequirement> + '_ {
        self.functions
            .iter()
            .flat_map(|function| &function.modified_units)
    }
}
