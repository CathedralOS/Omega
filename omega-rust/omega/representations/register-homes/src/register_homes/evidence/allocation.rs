//! Allocation outcome custody and its explicit replay evidence roles.
//!
//! These records do not grant validation or publication authority. The owning
//! transform independently reconstructs and compares them before admission.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterHomeCustodyReceipt {
    pub psi: TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub entry: MachineId,
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub manifest: PrePhysicalOptimizationManifestIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub allocator_availability: crate::AllocatorAvailabilityIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: crate::LivenessIdentity,
    pub ranges: crate::LiveRangeIdentity,
    pub legality: crate::AllocationLegalityIdentity,
    pub homes: RegisterHomeIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub function_count: usize,
    pub structural_unit_function_count: usize,
    pub assignment_count: usize,
}

impl RegisterHomeCustodyReceipt {
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
    pub const fn allocator_availability(self) -> crate::AllocatorAvailabilityIdentity {
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
    pub const fn legality(self) -> crate::AllocationLegalityIdentity {
        self.legality
    }
    pub const fn homes(self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostCopyRegisterHomeCustodyReceipt {
    pub source: SelectedReanalysisCustodyReceipt,
    pub homes: RegisterHomeIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub function_count: usize,
    pub assignment_count: usize,
}

impl PostCopyRegisterHomeCustodyReceipt {
    pub const fn source(self) -> SelectedReanalysisCustodyReceipt {
        self.source
    }
    pub const fn homes(self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostLiteralFoldHomeCustodyReceipt {
    pub source: LiteralFoldCustodyReceipt,
    pub homes: RegisterHomeIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub function_count: usize,
    pub assignment_count: usize,
}

impl PostLiteralFoldHomeCustodyReceipt {
    pub const fn source(&self) -> &LiteralFoldCustodyReceipt {
        &self.source
    }
    pub const fn homes(&self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(&self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(&self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostSelectedLoweringHomeCustodyReceipt {
    pub source: SelectedLoweringOptimizationCustodyReceipt,
    pub homes: RegisterHomeIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub function_count: usize,
    pub assignment_count: usize,
}

impl PostSelectedLoweringHomeCustodyReceipt {
    pub const fn source(&self) -> &SelectedLoweringOptimizationCustodyReceipt {
        &self.source
    }
    pub const fn homes(&self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(&self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(&self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveResidentRematerializationCustodyReceipt {
    pub source: AllocationLegalityCustodyReceipt,
    pub choices: crate::SpillChoiceIdentity,
    pub choice_policy: SpillChoicePolicy,
    pub choice_usage: optimization_core::OptimizationWorkUsage,
    pub classifications: crate::RecoveryClassificationIdentity,
    pub classification_policy: RecoveryClassificationPolicy,
    pub classification_usage: optimization_core::OptimizationWorkUsage,
    pub rematerialization: crate::PressureRematerializationIdentity,
    pub rematerialization_policy: PressureRematerializationPolicy,
    pub rematerialization_usage: optimization_core::OptimizationWorkUsage,
    pub budget: OptimizationWorkBudget,
    pub transformed_selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub liveness: crate::LivenessIdentity,
    pub ranges: crate::LiveRangeIdentity,
    pub legality: crate::AllocationLegalityIdentity,
    pub homes: crate::RegisterHomeIdentity,
    pub manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
    pub function_count: usize,
    pub virtual_register_count: usize,
    pub applied_count: usize,
    pub rewritten_use_count: usize,
    pub assignment_count: usize,
}

impl ActiveResidentRematerializationCustodyReceipt {
    pub const fn source(self) -> AllocationLegalityCustodyReceipt {
        self.source
    }
    pub const fn choices(self) -> crate::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn classifications(self) -> crate::RecoveryClassificationIdentity {
        self.classifications
    }
    pub const fn classification_policy(self) -> RecoveryClassificationPolicy {
        self.classification_policy
    }
    pub const fn classification_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.classification_usage
    }
    pub const fn rematerialization(self) -> crate::PressureRematerializationIdentity {
        self.rematerialization
    }
    pub const fn rematerialization_policy(self) -> PressureRematerializationPolicy {
        self.rematerialization_policy
    }
    pub const fn rematerialization_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.rematerialization_usage
    }
    pub const fn budget(self) -> OptimizationWorkBudget {
        self.budget
    }
    pub const fn transformed_selected(
        self,
    ) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn liveness(self) -> crate::LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> crate::LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> crate::AllocationLegalityIdentity {
        self.legality
    }
    pub const fn homes(self) -> crate::RegisterHomeIdentity {
        self.homes
    }
    pub const fn manifest(self) -> optimization_core::PostAllocationOptimizationManifestIdentity {
        self.manifest
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn applied_count(self) -> usize {
        self.applied_count
    }
    pub const fn rewritten_use_count(self) -> usize {
        self.rewritten_use_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

/// Evidence roles remain distinct; they do not choose the downstream program
/// representation or machine-plan implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationEvidence {
    RegisterHomes(RegisterHomeCustodyReceipt),
    FixedViewCopies(PostCopyRegisterHomeCustodyReceipt),
    LiteralFolds(PostLiteralFoldHomeCustodyReceipt),
    SelectedLowering(PostSelectedLoweringHomeCustodyReceipt),
    ActiveResidentRematerialization(ActiveResidentRematerializationCustodyReceipt),
}
