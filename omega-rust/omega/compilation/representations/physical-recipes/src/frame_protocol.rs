//! Physical function frames and the selected target-specific prologue and epilogue recipes.
use frame_admission::frame_layout::ValidatedTargetFrameLayout;
use machine_code::{TargetFrameLayoutIdentity, TargetFrameProtocolEncodingPolicy};
use post_allocation_machine::PostAllocationMachineProgram;
use std::sync::Arc;
use target::NativeTarget;
pub type PlannedTargetFrameProtocols<Origin> =
    crate::PhysicalRecipe<Origin, FrameProtocolInstructions>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameProtocolInstructions {
    pub frame_layout: TargetFrameLayoutIdentity,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub physical_register_model: register_model::PhysicalRegisterModelIdentity,
    pub target: NativeTarget,
    pub policy: TargetFrameProtocolEncodingPolicy,
    pub functions: Vec<PlannedFrameFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFrameFunction {
    pub machine: semantic_vocabulary::MachineId,
    pub effects: calling_conventions::StateEffectsEvidence,
    pub recipe: FrameRecipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameRecipe {
    X86(target_x86_64::X86_64FrameProtocolPlan),
    Aarch64(target_aarch64::Aarch64FrameProtocolPlan),
}

impl<Origin> PlannedTargetFrameProtocols<Origin> {
    pub fn function_effects(
        &self,
        machine: semantic_vocabulary::MachineId,
    ) -> Option<&calling_conventions::StateEffectsEvidence> {
        self.instructions
            .functions
            .iter()
            .find(|function| function.machine == machine)
            .map(|function| &function.effects)
    }
}

impl<Origin> PlannedTargetFrameProtocols<Origin> {
    pub fn frame_layout(&self) -> TargetFrameLayoutIdentity {
        self.instructions.frame_layout
    }
    pub fn register_environment(&self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.instructions.register_environment
    }
    pub fn physical_register_model(&self) -> register_model::PhysicalRegisterModelIdentity {
        self.instructions.physical_register_model
    }
    pub fn target(&self) -> NativeTarget {
        self.instructions.target
    }
    pub fn policy(&self) -> TargetFrameProtocolEncodingPolicy {
        self.instructions.policy
    }
    pub fn functions(&self) -> &[PlannedFrameFunction] {
        &self.instructions.functions
    }
}
impl PlannedFrameFunction {
    pub fn machine(&self) -> semantic_vocabulary::MachineId {
        self.machine
    }
    pub fn recipe(&self) -> &FrameRecipe {
        &self.recipe
    }
}
/// Complete physical instructions and stack geometry before any frame bytes
/// are encoded. Resource binding can inspect this form without emitting code.
#[derive(Debug)]
pub struct PhysicalFrameProgram<Origin> {
    pub instructions: PostAllocationMachineProgram,
    pub storage: Arc<machine_code::NonAuthoritativeCalleeSaveStoragePlan>,
    pub frame: ValidatedTargetFrameLayout,
    pub protocols: PlannedTargetFrameProtocols<Origin>,
}
