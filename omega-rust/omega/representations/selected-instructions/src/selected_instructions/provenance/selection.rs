//! Plain selection evidence. Independent replay in the transform grants admission.
use crate::SelectedInstructionPlanIdentity;
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionCustodyReceipt {
    pub psi: TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub entry: MachineId,
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub manifest: PrePhysicalOptimizationManifestIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub legalized: legalized_operations::LegalizedOperationPlanIdentity,
    pub legalization_validator: OptimizationValidatorIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub function_count: usize,
}

impl SelectionCustodyReceipt {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(self) -> target::NativeTarget {
        self.target
    }

    pub const fn entry(self) -> MachineId {
        self.entry
    }

    pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
        self.optimization
    }

    pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }

    pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
        self.manifest
    }

    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn legalized(self) -> legalized_operations::LegalizedOperationPlanIdentity {
        self.legalized
    }

    pub const fn legalization_validator(self) -> OptimizationValidatorIdentity {
        self.legalization_validator
    }

    pub const fn register_environment(self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }
}
