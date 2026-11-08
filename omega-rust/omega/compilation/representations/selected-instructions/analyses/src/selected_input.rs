use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    CurrentSelectedProgram, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedProgram,
};
use semantic_vocabulary::WorkScheduleIdentity;

/// The selected program an analysis reads: its plan and the identities its
/// receipt carries. Inputs are trusted same-compile plans; the stages that
/// produce them seal which of their outputs may be analyzed
/// (`ValidatedSelectedAnalysis` in Omega 04), so this crate does not
/// re-validate a plan it is handed.
pub trait SelectedProgramView {
    fn selected_plan(&self) -> &SelectedInstructionPlan;
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan>;
    fn selected_identity(&self) -> SelectedInstructionPlanIdentity;
    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity;
    fn work_schedule_identity(&self) -> WorkScheduleIdentity;

    /// The plan and its identities as the stages after allocation read them.
    fn current_program(&self) -> CurrentSelectedProgram<'_> {
        CurrentSelectedProgram {
            plan: self.selected_plan(),
            identity: self.selected_identity(),
            optimization_unit: self.optimization_unit_identity(),
            work_schedule: self.work_schedule_identity(),
        }
    }
}

/// Borrow a current program view without exposing which producer or rewrite
/// established it.
#[derive(Clone, Copy)]
pub struct SelectedProgramRef<'program> {
    program: &'program dyn SelectedProgramView,
}

impl<'program> SelectedProgramRef<'program> {
    pub fn plan(self) -> &'program SelectedInstructionPlan {
        self.program.selected_plan()
    }

    pub fn new(program: &'program impl SelectedProgramView) -> Self {
        Self { program }
    }
}

impl SelectedProgramView for SelectedProgramRef<'_> {
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

    fn work_schedule_identity(&self) -> WorkScheduleIdentity {
        self.program.work_schedule_identity()
    }
}

/// The program instruction selection produced.
impl SelectedProgramView for SelectedProgram {
    fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        self.shared_plan()
    }

    fn selected_plan(&self) -> &SelectedInstructionPlan {
        self.plan()
    }

    fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
        self.identity()
    }

    fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
        self.optimization_unit()
    }

    fn work_schedule_identity(&self) -> WorkScheduleIdentity {
        self.work_schedule()
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
    work_schedule: WorkScheduleIdentity,
}

impl OwnedSelectedProgram {
    pub fn retain(source: &impl SelectedProgramView) -> Self {
        Self {
            plan: source.shared_selected_plan(),
            selected: source.selected_identity(),
            optimization_unit: source.optimization_unit_identity(),
            work_schedule: source.work_schedule_identity(),
        }
    }
}

impl SelectedProgramView for OwnedSelectedProgram {
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
    fn work_schedule_identity(&self) -> WorkScheduleIdentity {
        self.work_schedule
    }
}
