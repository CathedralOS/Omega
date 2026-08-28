use omega_optimization_core::OptimizationUnitIdentity;
use omega_terminal_selected_instructions::{
    TerminalSelectedInstructionPlan, TerminalSelectedInstructionPlanIdentity,
};
use omega_terminal_target_operations_to_selected_instructions::ValidatedTerminalSelectedInstructions;
use psi_core::FuelScheduleIdentity;

use crate::{
    ValidatedTerminalFixedViewCopies, ValidatedTerminalLiteralFold,
    ValidatedTerminalPressureRematerialization,
};

mod sealed {
    pub trait Sealed {}
}

/// Sealed input boundary for analyses over independently validated selected
/// CFGs. External callers cannot implement this trait for an unchecked plan.
pub trait ValidatedTerminalSelectedAnalysis: sealed::Sealed {
    fn selected_plan(&self) -> &TerminalSelectedInstructionPlan;
    fn selected_identity(&self) -> TerminalSelectedInstructionPlanIdentity;
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity;
    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity;
}

impl sealed::Sealed for ValidatedTerminalSelectedInstructions {}

impl ValidatedTerminalSelectedAnalysis for ValidatedTerminalSelectedInstructions {
    fn selected_plan(&self) -> &TerminalSelectedInstructionPlan {
        self.plan()
    }

    fn selected_identity(&self) -> TerminalSelectedInstructionPlanIdentity {
        self.receipt().identity()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedTerminalFixedViewCopies {}

impl ValidatedTerminalSelectedAnalysis for ValidatedTerminalFixedViewCopies {
    fn selected_plan(&self) -> &TerminalSelectedInstructionPlan {
        &self.plan().transformed
    }

    fn selected_identity(&self) -> TerminalSelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedTerminalLiteralFold {}

impl ValidatedTerminalSelectedAnalysis for ValidatedTerminalLiteralFold {
    fn selected_plan(&self) -> &TerminalSelectedInstructionPlan {
        self.transformed()
    }

    fn selected_identity(&self) -> TerminalSelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}

impl sealed::Sealed for ValidatedTerminalPressureRematerialization {}

impl ValidatedTerminalSelectedAnalysis for ValidatedTerminalPressureRematerialization {
    fn selected_plan(&self) -> &TerminalSelectedInstructionPlan {
        self.transformed()
    }

    fn selected_identity(&self) -> TerminalSelectedInstructionPlanIdentity {
        self.receipt().transformed_selected()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.receipt().optimization_unit()
    }

    fn fuel_schedule_identity(&self) -> FuelScheduleIdentity {
        self.receipt().fuel_schedule()
    }
}
