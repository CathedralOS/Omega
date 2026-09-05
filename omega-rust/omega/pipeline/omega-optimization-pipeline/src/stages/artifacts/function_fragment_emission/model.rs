use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, Optimization,
    OptimizationSelectionIdentity, PostAllocationOptimizationManifestIdentity,
};
use omega_target::NativeTarget;
use psi_core::FuelScheduleIdentity;
use psi_terminal::TerminalPsiIdentity;

use crate::{SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity};

use super::source::StagedOptimizedFunctionFragmentEmissionSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionSourceKind {
    X86Rel8V1,
    SelectedLoweringV1,
    PostAllocationMachineOptimizationV1 { optimization: Optimization },
    AllocationRecoveryV1,
    UnitBaselineV1,
    StructuralUnitV1,
    CanonicalFixedFrameBodyV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionStage {
    ValidatedRelocationFreeFunctionFragmentsV1,
    ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentEmissionStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instruction_spans: u64,
    pub zero_byte_instruction_spans: u64,
    pub bytes: u64,
    pub resolved_conditional_branches: u64,
    pub logical_fuel_settlements: u64,
    pub structural_unit_functions: u64,
    pub structural_unit_blocks: u64,
    pub structural_unit_instruction_spans: u64,
    pub structural_unit_bytes: u64,
    pub unresolved_internal_machine_fixups: u64,
    pub structural_logical_fuel_settlements: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionManifest {
    pub identity: FunctionFragmentEmissionManifestIdentity,
    pub stage: FunctionFragmentEmissionStage,
    pub source_kind: FunctionFragmentEmissionSourceKind,
    pub source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: omega_physical_instructions::PostAllocationMachineIdentity,
    pub final_pre_layout: SelectedFormEncodingIdentity,
    pub final_resolved_layout: crate::ResolvedSelectedFormLayoutIdentity,
    pub whole_function_exit_contract: WholeFunctionExitContractIdentity,
    pub fragments: FunctionFragmentEmissionIdentity,
    pub target: NativeTarget,
    pub statistics: FunctionFragmentEmissionStatistics,
    pub section_placement: FunctionFragmentEmissionUnavailableData,
    pub symbols: FunctionFragmentEmissionUnavailableData,
    pub object_relocations: FunctionFragmentEmissionUnavailableData,
    pub executable_image: FunctionFragmentEmissionUnavailableData,
    pub installation: FunctionFragmentEmissionUnavailableData,
    pub publication: FunctionFragmentEmissionUnavailableData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentEmissionManifest {
    pub(super) record: FunctionFragmentEmissionManifest,
}

impl ValidatedFunctionFragmentEmissionManifest {
    pub const fn record(&self) -> &FunctionFragmentEmissionManifest {
        &self.record
    }
}

#[derive(Debug)]
pub struct StagedOptimizedFunctionFragmentEmission {
    pub(super) source: StagedOptimizedFunctionFragmentEmissionSource,
    pub(super) fragments: FunctionFragmentEmissionPlan,
    pub(super) manifest: ValidatedFunctionFragmentEmissionManifest,
    pub(super) custody: StagedFunctionFragmentEmissionCustodyReceipt,
}

impl StagedOptimizedFunctionFragmentEmission {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmissionSource {
        &self.source
    }
    pub const fn fragments(&self) -> &FunctionFragmentEmissionPlan {
        &self.fragments
    }
    pub const fn manifest(&self) -> &ValidatedFunctionFragmentEmissionManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> StagedFunctionFragmentEmissionCustodyReceipt {
        self.custody
    }

    pub fn verified_input(
        &self,
    ) -> &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput {
        self.source.verified_input()
    }

    pub fn provider_installation(
        &self,
    ) -> Option<&omega_psi_to_abstract_operations::AdmittedProviderInstallation> {
        self.source.provider_installation()
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        self.source.function_relative_manifest()
    }

    pub fn post_allocation_manifest(
        &self,
    ) -> &omega_regalloc::ValidatedPostAllocationOptimizationManifest {
        self.source.post_allocation_manifest()
    }

    pub fn pre_physical_manifest(
        &self,
    ) -> &omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest {
        self.source.pre_physical_manifest()
    }

    #[cfg(test)]
    pub(crate) fn fragments_mut(&mut self) -> &mut FunctionFragmentEmissionPlan {
        &mut self.fragments
    }

    #[cfg(test)]
    pub(crate) fn manifest_record_mut(&mut self) -> &mut FunctionFragmentEmissionManifest {
        &mut self.manifest.record
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_source_realization_for_test(&mut self) {
        self.custody.source_realization =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                b"corrupt function-fragment source realization",
            );
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_fragments_for_test(&mut self) {
        self.custody.fragments = FunctionFragmentEmissionIdentity::from_canonical_bytes(
            b"corrupt function-fragment emission",
        );
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_manifest_for_test(&mut self) {
        self.custody.manifest = FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(
            b"corrupt function-fragment manifest",
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedFunctionFragmentEmissionCustodyReceipt {
    pub(super) source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub(super) fragments: FunctionFragmentEmissionIdentity,
    pub(super) manifest: FunctionFragmentEmissionManifestIdentity,
}

impl StagedFunctionFragmentEmissionCustodyReceipt {
    pub const fn source_realization(
        self,
    ) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.source_realization
    }
    pub const fn fragments(self) -> FunctionFragmentEmissionIdentity {
        self.fragments
    }
    pub const fn manifest(self) -> FunctionFragmentEmissionManifestIdentity {
        self.manifest
    }
}
