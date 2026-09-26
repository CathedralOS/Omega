//! Current inputs to fragment emission, independent of producer-stage history.

use std::sync::Arc;

use register_homes_to_post_allocation_machine::PostAllocationMachinePlan;
use selected_instructions_to_selected_instructions::register_homes::RegisterHomePlan;
use target_operations_to_selected_instructions::{
    PreAllocationMachineEffectPlan, SelectedInstructionPlan,
};

use super::ResolvedMachineLayout;
use crate::machine_code::SelectedFormEncoding;

/// Shared original artifacts, not copied snapshots or admission tokens.
/// Consumers must independently validate their exact joins before publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMachineProgram {
    pub selected: Arc<SelectedInstructionPlan>,
    pub homes: Arc<RegisterHomePlan>,
    pub effects: Arc<PreAllocationMachineEffectPlan>,
    pub machine: Arc<PostAllocationMachinePlan>,
    pub encoding: Arc<SelectedFormEncoding>,
    pub layout: Arc<ResolvedMachineLayout>,
    pub frame: Arc<crate::machine_code::TargetFrameLayoutPlan>,
    pub protocol: Arc<crate::machine_code::TargetFrameProtocolEncodingPlan>,
}
