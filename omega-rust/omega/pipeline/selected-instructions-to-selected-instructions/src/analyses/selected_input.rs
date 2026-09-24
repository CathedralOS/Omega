use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::rewrites::unexecuted::peepholes::{
    ValidatedConditionMaterialization, ValidatedCopiedCallOperand, ValidatedProjectedAccess,
    ValidatedTerminatorPair,
};
use crate::rewrites::unexecuted::{
    ValidatedAddressFold, ValidatedArmRelocation, ValidatedBoundaryBoolean,
    ValidatedBoundaryBranch, ValidatedCommutingRelocation, ValidatedConfluenceRelocation,
    ValidatedConstantBoolean, ValidatedConstantBranch, ValidatedDeadCompare,
    ValidatedDeadStoreElimination, ValidatedEquivalentCompare, ValidatedInflowRelocation,
    ValidatedInterchange, ValidatedMemberRunRelocation, ValidatedRedundantCompare,
    ValidatedScheduledRelocation, ValidatedStoreMutationMotion, ValidatedStoredLoadForwarding,
};
use crate::{
    ValidatedCopyRemoval, ValidatedFixedViewCopies, ValidatedLiteralFold,
    ValidatedPreAllocationTransformation, ValidatedPressureRematerialization,
    ValidatedRedundantExtension, ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
};

mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for ValidatedAddressFold {}

impl ValidatedSelectedAnalysis for ValidatedAddressFold {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedArmRelocation {}

impl ValidatedSelectedAnalysis for ValidatedArmRelocation {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedBoundaryBoolean {}

impl ValidatedSelectedAnalysis for ValidatedBoundaryBoolean {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedBoundaryBranch {}

impl ValidatedSelectedAnalysis for ValidatedBoundaryBranch {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedConstantBoolean {}

impl ValidatedSelectedAnalysis for ValidatedConstantBoolean {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedConstantBranch {}

impl ValidatedSelectedAnalysis for ValidatedConstantBranch {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedInterchange {}

impl ValidatedSelectedAnalysis for ValidatedInterchange {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedCommutingRelocation {}

impl ValidatedSelectedAnalysis for ValidatedCommutingRelocation {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedConfluenceRelocation {}

impl ValidatedSelectedAnalysis for ValidatedConfluenceRelocation {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedCopyRemoval {}

impl ValidatedSelectedAnalysis for ValidatedCopyRemoval {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedRedundantCompare {}

impl ValidatedSelectedAnalysis for ValidatedRedundantCompare {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedRedundantExtension {}

impl ValidatedSelectedAnalysis for ValidatedRedundantExtension {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedPreAllocationTransformation {}

/// The pre-allocation joint fixed point's current program: whichever exact
/// family committed the last step, its transformed plan is the next sweep's
/// discovery source.
impl ValidatedSelectedAnalysis for ValidatedPreAllocationTransformation {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedStoredLoadForwarding {}

impl ValidatedSelectedAnalysis for ValidatedStoredLoadForwarding {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedStoreMutationMotion {}

impl ValidatedSelectedAnalysis for ValidatedStoreMutationMotion {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedDeadCompare {}

impl ValidatedSelectedAnalysis for ValidatedDeadCompare {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedDeadStoreElimination {}

impl ValidatedSelectedAnalysis for ValidatedDeadStoreElimination {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedEquivalentCompare {}

impl ValidatedSelectedAnalysis for ValidatedEquivalentCompare {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedInflowRelocation {}

impl ValidatedSelectedAnalysis for ValidatedInflowRelocation {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedMemberRunRelocation {}

impl ValidatedSelectedAnalysis for ValidatedMemberRunRelocation {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedScheduledRelocation {}

impl ValidatedSelectedAnalysis for ValidatedScheduledRelocation {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedRuntimeSpill {}

impl ValidatedSelectedAnalysis for ValidatedRuntimeSpill {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedRuntimeRematerialization {}

impl ValidatedSelectedAnalysis for ValidatedRuntimeRematerialization {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

/// Sealed input boundary for analyses over independently validated selected
/// CFGs. External callers cannot implement this trait for an unchecked plan.
pub trait ValidatedSelectedAnalysis: sealed::Sealed {
    fn selected_plan(&self) -> &SelectedInstructionPlan;
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan>;
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity;
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity;
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity;
}

/// Borrow the current validated program without exposing which producer or
/// rewrite established it. This does not admit an unchecked selected plan.
#[derive(Clone, Copy)]
pub struct SelectedProgramRef<'program> {
    program: &'program dyn ValidatedSelectedAnalysis,
}

impl<'program> SelectedProgramRef<'program> {
    pub fn plan(self) -> &'program SelectedInstructionPlan {
        self.program.selected_plan()
    }

    pub fn new(program: &'program impl ValidatedSelectedAnalysis) -> Self {
        Self { program }
    }
}

impl sealed::Sealed for SelectedProgramRef<'_> {}

impl ValidatedSelectedAnalysis for SelectedProgramRef<'_> {
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.program.shared_selected_plan()
    }

    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.program.selected_plan()
    }

    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.program.selected_identity()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.program.optimization_unit_identity()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.program.fuel_schedule_identity()
    }
}

impl sealed::Sealed for ValidatedSelectedInstructions {}

impl ValidatedSelectedAnalysis for ValidatedSelectedInstructions {
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_plan()
    }

    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.plan()
    }

    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().identity()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedFixedViewCopies {}

impl ValidatedSelectedAnalysis for ValidatedFixedViewCopies {
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        std::sync::Arc::clone(&self.plan().transformed)
    }

    fn selected_plan(&self) -> &SelectedInstructionPlan {
        &self.plan().transformed
    }

    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedLiteralFold {}

impl ValidatedSelectedAnalysis for ValidatedLiteralFold {
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }

    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }

    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedPressureRematerialization {}

impl ValidatedSelectedAnalysis for ValidatedPressureRematerialization {
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }

    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }

    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedTerminatorPair {}

impl ValidatedSelectedAnalysis for ValidatedTerminatorPair {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedProjectedAccess {}

impl ValidatedSelectedAnalysis for ValidatedProjectedAccess {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedCopiedCallOperand {}

impl ValidatedSelectedAnalysis for ValidatedCopiedCallOperand {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedConditionMaterialization {}

impl ValidatedSelectedAnalysis for ValidatedConditionMaterialization {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.transformed()
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_transformed()
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

/// An admitted current selected program, independent of its producing stage.
/// Construction only accepts the sealed analysis boundary; a raw plan or digest
/// cannot mint this token. Replay evidence remains with the enclosing product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSelectedProgram {
    plan: std::sync::Arc<SelectedInstructionPlan>,
    selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl OwnedSelectedProgram {
    pub fn retain(source: &impl ValidatedSelectedAnalysis) -> Self {
        Self {
            plan: source.shared_selected_plan(),
            selected: source.selected_identity(),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        }
    }
}

impl sealed::Sealed for OwnedSelectedProgram {}

impl ValidatedSelectedAnalysis for OwnedSelectedProgram {
    fn selected_plan(&self) -> &SelectedInstructionPlan {
        &self.plan
    }
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        std::sync::Arc::clone(&self.plan)
    }
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
}
