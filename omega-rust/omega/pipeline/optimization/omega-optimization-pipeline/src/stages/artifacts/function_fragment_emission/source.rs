use omega_regalloc::ValidatedSelectedAnalysis;

use crate::{
    StagedAllocationRecoveryFunctionRelativeRealization,
    StagedFixedFrameFunctionRelativeRealization,
    StagedFunctionRelativeLayoutOptimizationRealization,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    StagedOptimizedUnitFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeSource,
    StagedSelectedLoweringFunctionRelativeRealization,
};

#[derive(Debug)]
pub enum StagedOptimizedFunctionFragmentEmissionSource {
    X86Rel8Direct(Box<StagedFunctionRelativeLayoutOptimizationRealization>),
    SelectedLowering(Box<StagedSelectedLoweringFunctionRelativeRealization>),
    PostAllocationMachine(Box<StagedPostAllocationMachineFunctionRelativeRealization>),
    AllocationRecovery(Box<StagedAllocationRecoveryFunctionRelativeRealization>),
    UnitBaseline(Box<StagedOptimizedUnitFunctionRelativeRealization>),
    StructuralUnit(Box<StagedOptimizedStructuralUnitFunctionRelativeRealization>),
    FixedFrame(Box<StagedFixedFrameFunctionRelativeRealization>),
}

impl StagedOptimizedFunctionFragmentEmissionSource {
    pub const fn fixed_frame_realization(
        &self,
    ) -> Option<&StagedFixedFrameFunctionRelativeRealization> {
        match self {
            Self::FixedFrame(realization) => Some(realization),
            _ => None,
        }
    }

    pub fn selected_plan(&self) -> &omega_selected_instructions::SelectedInstructionPlan {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .selected_plan(),
            Self::SelectedLowering(realization) => selected_after_lowering(realization.homes()),
            Self::PostAllocationMachine(realization) => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => homes
                    .legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .selected()
                    .selected_plan(),
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    selected_after_lowering(homes)
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(
                    source,
                ) => source.selected_plan(),
            },
            Self::AllocationRecovery(realization) => realization.source().selected_plan(),
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .selected_plan(),
            Self::StructuralUnit(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .selected_plan(),
            Self::FixedFrame(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .selected_plan(),
        }
    }

    pub const fn register_homes(&self) -> &omega_regalloc::ValidatedRegisterHomes {
        match self {
            Self::X86Rel8Direct(realization) => realization.homes().homes(),
            Self::SelectedLowering(realization) => realization.homes().homes(),
            Self::PostAllocationMachine(realization) => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => homes.homes(),
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    homes.homes()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(
                    source,
                ) => source.homes(),
            },
            Self::AllocationRecovery(realization) => realization.source().homes(),
            Self::UnitBaseline(realization) => realization.homes().homes(),
            Self::StructuralUnit(realization) => realization.homes().homes(),
            Self::FixedFrame(realization) => realization.homes().homes(),
        }
    }

    pub fn register_environment(&self) -> &crate::ValidatedTargetRegisterEnvironment {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::SelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::PostAllocationMachine(realization) => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => homes
                    .legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .register_environment(),
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    homes
                        .selected_lowering_run()
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .register_environment()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(
                    source,
                ) => source.register_environment(),
            },
            Self::AllocationRecovery(realization) => realization.source().register_environment(),
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::StructuralUnit(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::FixedFrame(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
        }
    }

    pub const fn exit_contract(&self) -> &crate::ValidatedWholeFunctionExitContract {
        match self {
            Self::X86Rel8Direct(realization) => realization.exit_contract(),
            Self::SelectedLowering(realization) => realization.exit_contract(),
            Self::PostAllocationMachine(realization) => realization.exit_contract(),
            Self::AllocationRecovery(realization) => realization.exit_contract(),
            Self::UnitBaseline(realization) => realization.exit_contract(),
            Self::StructuralUnit(realization) => realization.exit_contract(),
            Self::FixedFrame(realization) => realization.exit_contract(),
        }
    }
    pub fn pre_physical_manifest(
        &self,
    ) -> &omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::SelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::PostAllocationMachine(realization) => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => homes
                    .legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .optimized_target()
                    .optimized()
                    .pre_physical_manifest(),
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    homes
                        .selected_lowering_run()
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .optimized_target()
                        .optimized()
                        .pre_physical_manifest()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(
                    source,
                ) => source
                    .optimized_target()
                    .optimized()
                    .pre_physical_manifest(),
            },
            Self::AllocationRecovery(realization) => realization
                .source()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::StructuralUnit(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::FixedFrame(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
        }
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization.manifest(),
            Self::SelectedLowering(realization) => realization.manifest(),
            Self::PostAllocationMachine(realization) => realization.manifest(),
            Self::AllocationRecovery(realization) => realization.manifest(),
            Self::UnitBaseline(realization) => realization.manifest(),
            Self::StructuralUnit(realization) => realization.manifest(),
            Self::FixedFrame(realization) => realization.manifest(),
        }
    }

    pub const fn post_allocation_manifest(
        &self,
    ) -> &omega_regalloc::ValidatedPostAllocationOptimizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization.homes().post_allocation_manifest(),
            Self::SelectedLowering(realization) => realization.homes().post_allocation_manifest(),
            Self::PostAllocationMachine(realization) => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => {
                    homes.post_allocation_manifest()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    homes.post_allocation_manifest()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(
                    source,
                ) => source.post_allocation_manifest(),
            },
            Self::AllocationRecovery(realization) => {
                realization.source().post_allocation_manifest()
            }
            Self::UnitBaseline(realization) => realization.homes().post_allocation_manifest(),
            Self::StructuralUnit(realization) => realization.homes().post_allocation_manifest(),
            Self::FixedFrame(realization) => realization.homes().post_allocation_manifest(),
        }
    }

    /// Borrow the exact optimized-target carrier retained through every
    /// admitted realization route.
    pub fn optimized_target(&self) -> &crate::ValidatedOptimizedTargetOperations {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target(),
            Self::SelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target(),
            Self::PostAllocationMachine(realization) => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => homes
                    .legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .optimized_target(),
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    homes
                        .selected_lowering_run()
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .optimized_target()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(
                    source,
                ) => source.optimized_target(),
            },
            Self::AllocationRecovery(realization) => realization.source().optimized_target(),
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target(),
            Self::StructuralUnit(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target(),
            Self::FixedFrame(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target(),
        }
    }

    /// Borrow the exact verifier-owned input retained through every admitted
    /// realization route. This accessor does not detach semantic or proof
    /// context from staged custody.
    pub fn verified_input(
        &self,
    ) -> &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput {
        self.optimized_target().optimized().verified_input()
    }

    /// Borrow the opaque checked-provider installation, when one authorized
    /// the installed calls retained by this realization.
    pub fn provider_installation(
        &self,
    ) -> Option<&omega_psi_to_abstract_operations::AdmittedProviderInstallation> {
        self.optimized_target().provider_installation()
    }
}

fn selected_after_lowering(
    homes: &crate::StagedOptimizedRegisterHomesAfterSelectedLowering,
) -> &omega_selected_instructions::SelectedInstructionPlan {
    let run = homes.selected_lowering_run();
    match run.steps().last() {
        Some(step) => step.fold().selected_plan(),
        None => run
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .selected()
            .selected_plan(),
    }
}
