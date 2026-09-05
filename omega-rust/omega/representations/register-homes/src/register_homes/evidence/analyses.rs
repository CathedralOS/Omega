//! Input identities and counts reported by allocation analyses.
//!
//! These records do not grant validation or publication authority. The owning
//! transform independently reconstructs and compares them before admission.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationLegalityCustodyReceipt {
    pub psi: TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub entry: MachineId,
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub manifest: PrePhysicalOptimizationManifestIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: crate::LivenessIdentity,
    pub ranges: crate::LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub function_count: usize,
    pub structural_unit_function_count: usize,
    pub virtual_register_count: usize,
    pub point_count: usize,
    pub candidate_count: usize,
    pub entry_transition_count: usize,
}

impl AllocationLegalityCustodyReceipt {
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
    pub const fn register_environment(self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> crate::LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> crate::LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn point_count(self) -> usize {
        self.point_count
    }
    pub const fn candidate_count(self) -> usize {
        self.candidate_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivenessCustodyReceipt {
    pub psi: TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub entry: MachineId,
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub manifest: PrePhysicalOptimizationManifestIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: LivenessIdentity,
    pub function_count: usize,
    pub structural_unit_function_count: usize,
    pub block_count: usize,
    pub virtual_register_count: usize,
    pub instruction_count: usize,
    pub successor_count: usize,
}

impl LivenessCustodyReceipt {
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

    pub const fn register_environment(self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn liveness(self) -> LivenessIdentity {
        self.liveness
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }

    pub const fn block_count(self) -> usize {
        self.block_count
    }

    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }

    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }

    pub const fn successor_count(self) -> usize {
        self.successor_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeCustodyReceipt {
    pub psi: TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub entry: MachineId,
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub manifest: PrePhysicalOptimizationManifestIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: crate::LivenessIdentity,
    pub ranges: LiveRangeIdentity,
    pub function_count: usize,
    pub structural_unit_function_count: usize,
    pub block_count: usize,
    pub virtual_register_count: usize,
    pub virtual_occurrence_count: usize,
    pub fixed_constraint_count: usize,
    pub virtual_fragment_count: usize,
    pub architectural_unit_count: usize,
    pub architectural_action_count: usize,
    pub architectural_fragment_count: usize,
    pub virtual_edge_connector_count: usize,
    pub architectural_edge_connector_count: usize,
    pub interference_count: usize,
}

impl LiveRangeCustodyReceipt {
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

    pub const fn register_environment(self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn liveness(self) -> crate::LivenessIdentity {
        self.liveness
    }

    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }

    pub const fn block_count(self) -> usize {
        self.block_count
    }

    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }

    pub const fn virtual_fragment_count(self) -> usize {
        self.virtual_fragment_count
    }

    pub const fn virtual_occurrence_count(self) -> usize {
        self.virtual_occurrence_count
    }

    pub const fn fixed_constraint_count(self) -> usize {
        self.fixed_constraint_count
    }

    pub const fn architectural_unit_count(self) -> usize {
        self.architectural_unit_count
    }

    pub const fn architectural_fragment_count(self) -> usize {
        self.architectural_fragment_count
    }

    pub const fn architectural_action_count(self) -> usize {
        self.architectural_action_count
    }

    pub const fn virtual_edge_connector_count(self) -> usize {
        self.virtual_edge_connector_count
    }

    pub const fn architectural_edge_connector_count(self) -> usize {
        self.architectural_edge_connector_count
    }

    pub const fn interference_count(self) -> usize {
        self.interference_count
    }
}
