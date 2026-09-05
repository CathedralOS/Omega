use omega_machine_code::ResolvedMachineProgram;

use super::current::CurrentFunctionFragmentInput;
use super::replay::FunctionFragmentReplayInputs;
use super::{FunctionFragmentEmissionError, FunctionFragmentEmissionSourceKind};

/// Current program and admission facts are independent of replay history.
/// Only independent replay consumes the earlier producer-stage objects.
#[derive(Debug)]
pub struct StagedOptimizedFunctionFragmentEmissionSource {
    current: CurrentFunctionFragmentInput,
    replay: FunctionFragmentReplayInputs,
}

impl From<FunctionFragmentReplayInputs> for StagedOptimizedFunctionFragmentEmissionSource {
    fn from(replay: FunctionFragmentReplayInputs) -> Self {
        Self {
            current: CurrentFunctionFragmentInput::retain(&replay),
            replay,
        }
    }
}

impl StagedOptimizedFunctionFragmentEmissionSource {
    pub fn program(&self) -> &ResolvedMachineProgram {
        &self.current.program
    }
    pub fn machine(
        &self,
    ) -> &omega_register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan
    {
        &self.current.machine
    }
    pub fn resolved_layout(
        &self,
    ) -> &omega_selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout
    {
        &self.current.layout
    }
    pub fn selected_plan(&self) -> &omega_selected_instructions::SelectedInstructionPlan {
        &self.current.program.selected
    }
    pub fn register_homes(
        &self,
    ) -> &omega_selected_instructions_to_register_homes::ValidatedRegisterHomes {
        &self.current.homes
    }
    pub fn register_environment(
        &self,
    ) -> &omega_target_to_register_environment::ValidatedTargetRegisterEnvironment {
        &self.current.environment
    }
    pub fn encoding(
        &self,
    ) -> &omega_post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding
    {
        &self.current.encoding
    }
    pub fn frame_protocol(
        &self,
    ) -> Option<&omega_frame_layout_to_frame_protocol::ValidatedTargetFrameProtocolEncoding> {
        self.current.frame_protocol.as_ref()
    }
    pub fn frame_layout(
        &self,
    ) -> Option<&omega_post_allocation_machine_to_frame_layout::ValidatedTargetFrameLayout> {
        self.current.frame_layout.as_ref()
    }
    pub const fn exit_contract(&self) -> &crate::ValidatedWholeFunctionExitContract {
        &self.current.exit
    }
    pub const fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.current.manifest
    }
    pub fn post_allocation_manifest(
        &self,
    ) -> &omega_selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest
    {
        &self.current.post_allocation_manifest
    }
    pub fn optimized_target(
        &self,
    ) -> &omega_abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations {
        &self.current.target_input
    }
    pub fn pre_physical_manifest(
        &self,
    ) -> &omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest {
        self.optimized_target().optimized().pre_physical_manifest()
    }
    pub fn verified_input(
        &self,
    ) -> &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput {
        self.optimized_target().optimized().verified_input()
    }
    pub fn provider_installation(
        &self,
    ) -> Option<&omega_psi_to_abstract_operations::AdmittedProviderInstallation> {
        self.optimized_target().provider_installation()
    }
    pub const fn source_kind(&self) -> FunctionFragmentEmissionSourceKind {
        self.current.source_kind
    }

    /// Rule-specific historical evidence, not a current-program accessor.
    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<&omega_post_allocation_machine_to_optimized_machine::StagedOptimizedPostAllocationMachineOptimization>{
        self.replay.post_allocation_machine_optimization()
    }
    pub(crate) fn replay(&self) -> &FunctionFragmentReplayInputs {
        &self.replay
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn replay_for_test(&self) -> &FunctionFragmentReplayInputs {
        &self.replay
    }
    pub(super) fn validate_current(&self) -> Result<(), FunctionFragmentEmissionError> {
        self.current.validate_against(&self.replay)
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn replay_mut(&mut self) -> &mut FunctionFragmentReplayInputs {
        &mut self.replay
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn into_replay_for_test(self) -> FunctionFragmentReplayInputs {
        self.replay
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn program_mut(&mut self) -> &mut ResolvedMachineProgram {
        &mut self.current.program
    }
}

impl From<crate::StagedFunctionRelativeLayoutOptimizationRealization>
    for StagedOptimizedFunctionFragmentEmissionSource
{
    fn from(realization: crate::StagedFunctionRelativeLayoutOptimizationRealization) -> Self {
        FunctionFragmentReplayInputs::X86Rel8Direct(Box::new(realization)).into()
    }
}

impl From<crate::StagedSelectedLoweringFunctionRelativeRealization>
    for StagedOptimizedFunctionFragmentEmissionSource
{
    fn from(realization: crate::StagedSelectedLoweringFunctionRelativeRealization) -> Self {
        FunctionFragmentReplayInputs::SelectedLowering(Box::new(realization)).into()
    }
}

impl From<crate::StagedPostAllocationMachineFunctionRelativeRealization>
    for StagedOptimizedFunctionFragmentEmissionSource
{
    fn from(realization: crate::StagedPostAllocationMachineFunctionRelativeRealization) -> Self {
        FunctionFragmentReplayInputs::PostAllocationMachine(Box::new(realization)).into()
    }
}

impl From<crate::StagedAllocationRecoveryFunctionRelativeRealization>
    for StagedOptimizedFunctionFragmentEmissionSource
{
    fn from(realization: crate::StagedAllocationRecoveryFunctionRelativeRealization) -> Self {
        FunctionFragmentReplayInputs::AllocationRecovery(Box::new(realization)).into()
    }
}

impl From<crate::StagedOptimizedUnitFunctionRelativeRealization>
    for StagedOptimizedFunctionFragmentEmissionSource
{
    fn from(realization: crate::StagedOptimizedUnitFunctionRelativeRealization) -> Self {
        FunctionFragmentReplayInputs::UnitBaseline(Box::new(realization)).into()
    }
}

impl From<crate::StagedOptimizedStructuralUnitFunctionRelativeRealization>
    for StagedOptimizedFunctionFragmentEmissionSource
{
    fn from(realization: crate::StagedOptimizedStructuralUnitFunctionRelativeRealization) -> Self {
        FunctionFragmentReplayInputs::StructuralUnit(Box::new(realization)).into()
    }
}

impl From<crate::StagedFixedFrameFunctionRelativeRealization>
    for StagedOptimizedFunctionFragmentEmissionSource
{
    fn from(realization: crate::StagedFixedFrameFunctionRelativeRealization) -> Self {
        FunctionFragmentReplayInputs::FixedFrame(Box::new(realization)).into()
    }
}
