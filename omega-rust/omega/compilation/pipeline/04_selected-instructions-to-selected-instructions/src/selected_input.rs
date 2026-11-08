//! Which selected programs this stage lets the shared analyses read.
//!
//! The analyses package reads any [`SelectedProgramView`]; this stage seals
//! the programs it hands them to instruction selection's output and the
//! validated results of its own rewrites, so an unchecked plan built here
//! cannot reach an analysis.

use crate::{
    ValidatedAddressFold, ValidatedConstantBoolean, ValidatedConstantBranch,
    ValidatedCopiedCallOperand, ValidatedCopyRemoval, ValidatedDeadCompare,
    ValidatedEquivalentCompare, ValidatedLiteralFold, ValidatedPreAllocationTransformation,
    ValidatedRedundantCompare, ValidatedRedundantExtension, ValidatedStoredLoadForwarding,
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

/// A selected program this stage produced or validated. External callers
/// cannot implement this trait for an unchecked plan.
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

validated_selected_analysis!(ValidatedAddressFold);

validated_selected_analysis!(ValidatedConstantBoolean);

validated_selected_analysis!(ValidatedConstantBranch);

validated_selected_analysis!(ValidatedCopyRemoval);

validated_selected_analysis!(ValidatedRedundantCompare);

validated_selected_analysis!(ValidatedRedundantExtension);

impl sealed::Sealed for ValidatedPreAllocationTransformation {}

impl ValidatedSelectedAnalysis for ValidatedPreAllocationTransformation {}

/// The pre-allocation joint fixed point's current program: whichever exact
/// family committed the last step, its transformed plan is the next sweep's
/// discovery source.
impl SelectedProgramView for ValidatedPreAllocationTransformation {
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
    fn work_schedule_identity(&self) -> WorkScheduleIdentity {
        self.work_schedule()
    }
}

validated_selected_analysis!(ValidatedStoredLoadForwarding);

validated_selected_analysis!(ValidatedDeadCompare);

validated_selected_analysis!(ValidatedEquivalentCompare);

validated_selected_analysis!(ValidatedLiteralFold);

validated_selected_analysis!(ValidatedCopiedCallOperand);
