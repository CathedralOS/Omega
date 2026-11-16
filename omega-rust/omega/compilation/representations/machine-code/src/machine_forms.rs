/*
Stages 07, 08 and 09 each read one form: the one the stage before them
produced. `FramedMachineProgram` is the form stage 07 reads. The frame step
produces it once the frame layout is known: the post-allocation machine plan,
the frame layout computed for that plan, and the selected program and physical
register model the plan was allocated against. Stage 07 reads the selected
instructions and frame slots beside the machine plan, so they travel in the
form rather than arriving as separate arguments.

Each later stage's output keeps the form it was computed from, so stage 08's
input reaches the framed machine through stage 07's encoding, and stage 09's
reaches both through stage 08's layout. The traits below are how a stage reads
its input without naming the stage that produced it: stage 07's output is
defined in stage 07, which stage 08 may not depend on. Their implementations
return what an earlier stage already computed; none of them derives anything.
*/

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use post_allocation_machine::PostAllocationMachinePlan;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    CurrentSelectedProgram, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::WorkScheduleIdentity;

use crate::{ResolvedMachineLayout, SelectedFormEncoding, TargetFrameLayoutPlan};

/// The post-allocation machine plan with its frame layout, and the selected
/// program and register model it was allocated against. Stage 07 checks that
/// these agree before it encodes anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramedMachineProgram {
    pub selected: Arc<SelectedInstructionPlan>,
    pub selected_identity: SelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub machine: Arc<PostAllocationMachinePlan>,
    pub physical: ValidatedPhysicalRegisterModel,
    pub frame: Arc<TargetFrameLayoutPlan>,
}

impl FramedMachineProgram {
    pub fn selected_program(&self) -> CurrentSelectedProgram<'_> {
        CurrentSelectedProgram {
            plan: &self.selected,
            identity: self.selected_identity,
            optimization_unit: self.optimization_unit,
            work_schedule: self.work_schedule,
        }
    }
}

/// A form that carries the framed machine program it was computed from.
pub trait FramedMachineSource {
    fn framed_machine(&self) -> &FramedMachineProgram;
}

impl FramedMachineSource for FramedMachineProgram {
    fn framed_machine(&self) -> &FramedMachineProgram {
        self
    }
}

/// Stage 07's output form: the layout-independent encoding and the framed
/// machine program it encodes.
pub trait EncodedMachineSource: FramedMachineSource {
    fn encoding(&self) -> &SelectedFormEncoding;
}

/// Stage 08's output form: the resolved layout and the encoding it lays out.
pub trait ResolvedLayoutSource: EncodedMachineSource {
    fn resolved_layout(&self) -> &Arc<ResolvedMachineLayout>;
}
