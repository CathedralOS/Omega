use omega_optimization_core::OptimizationUnitIdentity;
use omega_selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;
use psi_core::FuelScheduleIdentity;

use crate::{ValidatedFixedViewCopies, ValidatedLiteralFold, ValidatedPressureRematerialization};

mod sealed {
    pub trait Sealed {}
}

/// Sealed input boundary for analyses over independently validated selected
/// CFGs. External callers cannot implement this trait for an unchecked plan.
pub trait ValidatedSelectedAnalysis: sealed::Sealed {
    fn selected_plan(&self) -> &SelectedInstructionPlan;
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity;
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity;
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity;
}

impl sealed::Sealed for ValidatedSelectedInstructions {}

impl ValidatedSelectedAnalysis for ValidatedSelectedInstructions {
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
