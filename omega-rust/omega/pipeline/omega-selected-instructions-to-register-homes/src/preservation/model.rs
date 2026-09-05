use crate::RegisterHomeIdentity;
use omega_optimization_core::{
    OptimizationWorkBudget, OptimizationWorkUsage, PostAllocationOptimizationManifestIdentity,
};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_target::NativeTarget;
use psi_core::MachineId;

use omega_target_to_register_environment::FrameAbiPreservationConvention;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocatedCalleeSavedFunctionKind {
    Ordinary,
    StructuralUnit,
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
    pub kind: AllocatedCalleeSavedFunctionKind,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatedCalleeSavedRequirementReceipt {
    pub(in crate::preservation) identity: AllocatedCalleeSavedRequirementIdentity,
    pub(in crate::preservation) selected: SelectedInstructionPlanIdentity,
    pub(in crate::preservation) homes: RegisterHomeIdentity,
    pub(in crate::preservation) post_allocation_manifest:
        PostAllocationOptimizationManifestIdentity,
    pub(in crate::preservation) register_environment: TargetRegisterEnvironmentIdentity,
    pub(in crate::preservation) physical_register_model: PhysicalRegisterModelIdentity,
    pub(in crate::preservation) target: NativeTarget,
    pub(in crate::preservation) abi: FrameAbiPreservationConvention,
    pub(in crate::preservation) policy: AllocatedCalleeSavedRequirementPolicy,
    pub(in crate::preservation) usage: OptimizationWorkUsage,
    pub(in crate::preservation) function_count: usize,
    pub(in crate::preservation) modified_function_count: usize,
    pub(in crate::preservation) modified_unit_count: usize,
    pub(in crate::preservation) witness_count: usize,
}

impl AllocatedCalleeSavedRequirementReceipt {
    pub const fn identity(self) -> AllocatedCalleeSavedRequirementIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn homes(self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn physical_register_model(self) -> PhysicalRegisterModelIdentity {
        self.physical_register_model
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
    pub const fn abi(self) -> FrameAbiPreservationConvention {
        self.abi
    }
    pub const fn policy(self) -> AllocatedCalleeSavedRequirementPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn modified_function_count(self) -> usize {
        self.modified_function_count
    }
    pub const fn modified_unit_count(self) -> usize {
        self.modified_unit_count
    }
    pub const fn witness_count(self) -> usize {
        self.witness_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocatedCalleeSavedRequirements {
    pub(in crate::preservation) plan: AllocatedCalleeSavedRequirementPlan,
    pub(in crate::preservation) receipt: AllocatedCalleeSavedRequirementReceipt,
}

impl ValidatedAllocatedCalleeSavedRequirements {
    pub const fn plan(&self) -> &AllocatedCalleeSavedRequirementPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> AllocatedCalleeSavedRequirementReceipt {
        self.receipt
    }
}
