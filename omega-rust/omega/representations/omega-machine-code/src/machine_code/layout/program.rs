//! Current inputs to fragment emission, independent of producer-stage history.

use std::sync::Arc;

use omega_physical_instructions::PostAllocationMachinePlan;
use omega_register_homes::RegisterHomePlan;
use omega_selected_instructions::{PreAllocationMachineEffectPlan, SelectedInstructionPlan};

use super::ResolvedMachineLayout;

/// Shared original artifacts, not copied snapshots or admission tokens.
/// Consumers must independently validate their exact joins before publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMachineProgram {
    pub selected: Arc<SelectedInstructionPlan>,
    pub homes: Arc<RegisterHomePlan>,
    pub effects: Arc<PreAllocationMachineEffectPlan>,
    pub machine: Arc<PostAllocationMachinePlan>,
    pub layout: Arc<ResolvedMachineLayout>,
}
