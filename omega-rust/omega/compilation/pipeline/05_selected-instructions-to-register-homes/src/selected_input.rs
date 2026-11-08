//! Which selected programs this stage lets the shared analyses read.
//!
//! The analyses package reads any [`SelectedProgramView`]; register
//! allocation seals the programs it hands them: instruction selection's
//! program, the selected-optimization results it receives, and the validated
//! results of its own recovery rewrites, so an unchecked plan built here
//! cannot reach an analysis.

use crate::{
    ValidatedFixedOccurrences, ValidatedFixedViewCopies, ValidatedPressureRematerialization,
    ValidatedRuntimeRematerialization, ValidatedRuntimeSpill, ValidatedRuntimeSplit,
};
use optimization_core::OptimizationUnitIdentity;
use selected_instruction_analyses::{
    OwnedSelectedProgram, SelectedProgramRef, SelectedProgramView,
};
use selected_instructions::{
    SelectedInstructionPlan, SelectedInstructionPlanIdentity, SelectedProgram,
};
use semantic_vocabulary::WorkScheduleIdentity;

mod sealed {
    pub trait Sealed {}
}

/// A selected program this stage produced, validated or received from
/// selected optimization. External callers cannot implement this trait for
/// an unchecked plan.
pub trait ValidatedSelectedAnalysis: sealed::Sealed + SelectedProgramView {}

impl sealed::Sealed for SelectedProgram {}
impl ValidatedSelectedAnalysis for SelectedProgram {}
impl sealed::Sealed for OwnedSelectedProgram {}
impl ValidatedSelectedAnalysis for OwnedSelectedProgram {}
impl sealed::Sealed for SelectedProgramRef<'_> {}
impl ValidatedSelectedAnalysis for SelectedProgramRef<'_> {}

/// The analysis view every validated rewrite result answers identically: the transformed plan, its shared handle, and the receipt's
/// three identities. One declarative row per validated type expands both
/// impls.
macro_rules! validated_selected_analysis {
    ($name:ident) => {
        impl sealed::Sealed for $name {}

        impl ValidatedSelectedAnalysis for $name {}

        impl SelectedProgramView for $name {
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
            fn work_schedule_identity(&self) -> WorkScheduleIdentity {
                self.receipt().work_schedule()
            }
        }
    };
}

validated_selected_analysis!(ValidatedRuntimeSpill);

validated_selected_analysis!(ValidatedRuntimeSplit);

validated_selected_analysis!(ValidatedRuntimeRematerialization);

validated_selected_analysis!(ValidatedPressureRematerialization);

validated_selected_analysis!(ValidatedFixedOccurrences);

impl sealed::Sealed for ValidatedFixedViewCopies {}

impl ValidatedSelectedAnalysis for ValidatedFixedViewCopies {}

impl SelectedProgramView for ValidatedFixedViewCopies {
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

    fn work_schedule_identity(&self) -> WorkScheduleIdentity {
        self.receipt().work_schedule()
    }
}
