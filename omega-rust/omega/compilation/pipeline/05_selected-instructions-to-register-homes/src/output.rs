/*
We project the staged result of whichever allocation procedure ran
(`StagedAllocation`) into one `AllocationOutput` borrow and admit it as
`RetainedAllocation`. The handoff retains the producer's manifest and decisions without replaying
them. Explicit validation checks the recovery selection and manifest history;
current allocation facts are captured once for `Omega 06`.
*/

macro_rules! allocation_output {
    ($self:expr, $placed:expr, $selected:expr, $history:expr, $evidence:expr) => {
        AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: $placed,
                homes: $self.homes().plan(),
            },
            selected: $selected,
            liveness: $self.liveness(),
            ranges: $self.ranges(),
            legality: $self.legality(),
            homes: $self.homes(),
            manifest: $self.post_allocation_manifest(),
            environment: $self.register_environment(),
            selections: $self.selections(),
            budget: $self.budget_per_pass(),
            history: $history,
            evidence: $evidence,
        }
    };
}

pub(crate) mod baseline {
    use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
    use crate::StagedOptimizedRegisterHomes;

    impl ProjectAllocation for StagedOptimizedRegisterHomes {
        fn project_allocation(&self) -> AllocationOutput<'_> {
            allocation_output!(
                self,
                self.selected().plan(),
                self.selected(),
                self.history(),
                AllocationEvidence::RegisterHomes(self.custody().to_owned())
            )
        }
    }
}
pub(crate) mod current {
    use std::sync::Arc;

    use crate::{AssignedRegisterHomes, ValidatedPostAllocationOptimizationManifest};
    use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
    use register_environment::ValidatedTargetRegisterEnvironment;
    use register_homes::{AllocatedProgram, AllocatedProgramFacts};
    use selected_instruction_analyses::SelectedProgramView;
    use selected_instruction_analyses::{
        MachineEffectStageError, OwnedSelectedProgram, SelectedOptimizationRecord,
        SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
        analyze_machine_effects,
    };
    use selected_instructions::PreAllocationMachineEffectPlan;

    use super::{AllocationEvidence, AllocationOutput};

    #[derive(Debug)]
    pub(crate) struct CurrentAllocation {
        pub(crate) program: AllocatedProgram,
        selected: OwnedSelectedProgram,
        liveness: ValidatedLiveness,
        ranges: ValidatedLiveRanges,
        legality: ValidatedAllocationLegality,
        homes: AssignedRegisterHomes,
        manifest: ValidatedPostAllocationOptimizationManifest,
        environment: ValidatedTargetRegisterEnvironment,
        effects: Arc<PreAllocationMachineEffectPlan>,
        evidence: AllocationEvidence,
        selections: OptimizationSelections,
        budget: OptimizationWorkBudget,
        history: SelectedOptimizationRecord,
        publication: Arc<register_homes::AllocationPublicationFacts>,
    }

    impl CurrentAllocation {
        pub(crate) fn capture(
            source: &AllocationOutput<'_>,
            publication: Arc<register_homes::AllocationPublicationFacts>,
        ) -> Result<Self, MachineEffectStageError> {
            let effects =
                analyze_machine_effects(source.selected(), source.register_environment())?;
            let selected = OwnedSelectedProgram::retain(source.selected());
            Ok(Self {
                program: AllocatedProgram {
                    selected: selected.shared_selected_plan(),
                    homes: source.homes().shared_plan(),
                },
                selected,
                liveness: source.liveness().clone(),
                ranges: source.ranges().clone(),
                legality: source.legality().clone(),
                homes: source.homes().clone(),
                manifest: source.post_allocation_manifest().clone(),
                environment: source.register_environment().clone(),
                effects: effects.shared_plan(),
                evidence: source.evidence().clone(),
                selections: source.selections().clone(),
                budget: source.budget_per_pass(),
                history: source.history().clone(),
                publication,
            })
        }

        pub(crate) fn facts<'a>(
            &'a self,
            preservation: &'a register_homes::AllocatedCalleeSavedRequirements,
        ) -> AllocatedProgramFacts<'a> {
            AllocatedProgramFacts {
                publication: &self.publication,
                preservation,
                environment: &self.environment,
                selected: self.selected.current_program(),
                effects: &self.effects,
                ranges: self.ranges.plan(),
                ranges_identity: self.ranges.receipt().identity(),
                legality: self.legality.plan(),
                legality_identity: self.legality.receipt().identity(),
                homes: self.homes.plan(),
                homes_identity: self.homes.receipt().identity(),
            }
        }

        pub(crate) fn publication(&self) -> &Arc<register_homes::AllocationPublicationFacts> {
            &self.publication
        }

        pub(crate) fn view(&self) -> AllocationOutput<'_> {
            AllocationOutput {
                program: self.program.as_ref(),
                selected: SelectedProgramRef::new(&self.selected),
                liveness: &self.liveness,
                ranges: &self.ranges,
                legality: &self.legality,
                homes: &self.homes,
                manifest: &self.manifest,
                environment: &self.environment,
                evidence: self.evidence.clone(),
                selections: &self.selections,
                budget: self.budget,
                history: &self.history,
            }
        }
    }
}
pub(crate) mod fixed_view {
    use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
    use crate::StagedOptimizedRegisterHomesAfterFixedViewCopies;
    use selected_instruction_analyses::SelectedProgramRef;

    impl ProjectAllocation for StagedOptimizedRegisterHomesAfterFixedViewCopies {
        fn project_allocation(&self) -> AllocationOutput<'_> {
            allocation_output!(
                self,
                &self.selected().plan().transformed,
                SelectedProgramRef::new(self.selected()),
                self.history(),
                AllocationEvidence::FixedViewCopies(self.custody().to_owned())
            )
        }
    }
}
pub(crate) mod rematerialization {
    use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
    use crate::StagedOptimizedActiveResidentRematerialization;
    use selected_instruction_analyses::SelectedProgramRef;

    impl ProjectAllocation for StagedOptimizedActiveResidentRematerialization {
        fn project_allocation(&self) -> AllocationOutput<'_> {
            allocation_output!(
                self,
                self.rematerialization().transformed(),
                SelectedProgramRef::new(self.rematerialization()),
                self.history(),
                AllocationEvidence::ActiveResidentRematerialization(self.custody().to_owned(),)
            )
        }
    }
}
pub(crate) mod retained {
    use std::sync::Arc;

    use super::{AllocationAdmissionError, AllocationOutput, StagedAllocation};
    use optimization_core::Optimization;

    #[derive(Debug, Clone)]
    pub struct RetainedAllocation {
        current: Arc<super::current::CurrentAllocation>,
        requirements: crate::ValidatedAllocatedCalleeSavedRequirements,
        recovery: Option<Optimization>,
    }

    impl RetainedAllocation {
        pub fn program(&self) -> &register_homes::AllocatedProgram {
            &self.current.program
        }

        pub fn current(&self) -> AllocationOutput<'_> {
            self.current.view()
        }

        pub fn facts(&self) -> register_homes::AllocatedProgramFacts<'_> {
            self.current.facts(self.requirements.retained())
        }

        pub fn preservation(&self) -> &crate::ValidatedAllocatedCalleeSavedRequirements {
            &self.requirements
        }

        pub fn publication(&self) -> &Arc<register_homes::AllocationPublicationFacts> {
            self.current.publication()
        }

        /// Check the retained producer evidence only on an explicit replay request.
        pub fn validate_publication(
            &self,
        ) -> Result<(), crate::AllocationPublicationValidationError> {
            crate::publication_validation::validate(&self.current(), self.recovery)
        }

        /// Admit the result of the allocation procedure that ran. `recovery`
        /// is the allocation-recovery rule that procedure carried out, if any.
        pub(crate) fn admit(
            staged: StagedAllocation,
            recovery: Option<Optimization>,
        ) -> Result<Self, AllocationAdmissionError> {
            let current = staged.project();
            let publication = Arc::new(register_homes::AllocationPublicationFacts::retain(
                current.post_allocation_manifest().record().clone(),
                current.selections().clone(),
                current.budget_per_pass(),
            ));
            // Recovery has finished changing instructions and homes. Derive the
            // preservation facts here, once, before physical lowering starts.
            let requirements = crate::stage_allocated_callee_saved_requirements(
                &current,
                crate::AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
                current.budget_per_pass(),
            )
            .map_err(AllocationAdmissionError::Preservation)?;
            Ok(Self {
                requirements,
                recovery,
                current: Arc::new(
                    super::current::CurrentAllocation::capture(&current, publication)
                        .map_err(AllocationAdmissionError::MachineEffects)?,
                ),
            })
        }
    }
}
pub(crate) mod runtime_spill {
    use super::{AllocationEvidence, AllocationOutput};
    use crate::assignment::runtime_spill::RuntimeSpillAllocation;

    impl RuntimeSpillAllocation {
        pub(crate) fn project_allocation(&self) -> AllocationOutput<'_> {
            let rewrite = &self.last.rewrite;
            AllocationOutput {
                program: register_homes::AllocatedProgramRef {
                    selected: rewrite.transformed(),
                    homes: self.homes.plan(),
                },
                selected: rewrite.selected(),
                liveness: &self.facts.liveness,
                ranges: &self.facts.ranges,
                legality: &self.facts.legality,
                homes: &self.homes,
                manifest: &self.manifest,
                environment: self.source.register_environment(),
                selections: self.source.selections(),
                budget: self.source.budget_per_pass(),
                history: self.source.history(),
                evidence: AllocationEvidence::RuntimeSpill(self.manifest.record().identity),
            }
        }
    }
}

use crate::{
    AssignedRegisterHomes, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    ValidatedPostAllocationOptimizationManifest,
};
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
pub use retained::RetainedAllocation;
use selected_instruction_analyses::{
    SelectedOptimizationRecord, SelectedProgramRef, ValidatedAllocationLegality,
    ValidatedLiveRanges, ValidatedLiveness,
};

trait ProjectAllocation {
    fn project_allocation(&self) -> AllocationOutput<'_>;
}

/// The staged result of the one allocation procedure that ran.
#[derive(Debug)]
pub(crate) enum StagedAllocation {
    Homes(StagedOptimizedRegisterHomes),
    FixedViewHomes(StagedOptimizedRegisterHomesAfterFixedViewCopies),
    Rematerialized(StagedOptimizedActiveResidentRematerialization),
    RuntimeSpill(crate::assignment::runtime_spill::RuntimeSpillAllocation),
}

impl StagedAllocation {
    fn project(&self) -> AllocationOutput<'_> {
        match self {
            Self::Homes(staged) => staged.project_allocation(),
            Self::FixedViewHomes(staged) => staged.project_allocation(),
            Self::Rematerialized(staged) => staged.project_allocation(),
            Self::RuntimeSpill(staged) => staged.project_allocation(),
        }
    }
}

#[derive(Clone)]
pub struct AllocationOutput<'program> {
    program: register_homes::AllocatedProgramRef<'program>,
    selected: SelectedProgramRef<'program>,
    liveness: &'program ValidatedLiveness,
    ranges: &'program ValidatedLiveRanges,
    legality: &'program ValidatedAllocationLegality,
    homes: &'program AssignedRegisterHomes,
    manifest: &'program ValidatedPostAllocationOptimizationManifest,
    environment: &'program ValidatedTargetRegisterEnvironment,
    evidence: AllocationEvidence,
    selections: &'program OptimizationSelections,
    budget: OptimizationWorkBudget,
    history: &'program SelectedOptimizationRecord,
}

impl<'program> AllocationOutput<'program> {
    /// The Omega 04 record of the program allocation received.
    pub const fn history(&self) -> &'program SelectedOptimizationRecord {
        self.history
    }

    receipt_accessor_rows!(ref AllocationOutput {
        program: register_homes::AllocatedProgramRef<'program>,
    });

    pub fn selected_plan(&self) -> &'program selected_instructions::SelectedInstructionPlan {
        self.program.selected
    }

    receipt_accessor_rows!(ref AllocationOutput {
        selections: &'program OptimizationSelections,
    });
    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget
    }
    receipt_accessor_rows!(borrow AllocationOutput {
        selected: &SelectedProgramRef<'_>,
    });
    receipt_accessor_rows!(ref AllocationOutput {
        liveness: &'program ValidatedLiveness,
        ranges: &'program ValidatedLiveRanges,
        legality: &'program ValidatedAllocationLegality,
        homes: &'program AssignedRegisterHomes,
    });
    pub const fn post_allocation_manifest(
        &self,
    ) -> &'program ValidatedPostAllocationOptimizationManifest {
        self.manifest
    }
    pub const fn register_environment(&self) -> &'program ValidatedTargetRegisterEnvironment {
        self.environment
    }
    receipt_accessor_rows!(borrow AllocationOutput {
        evidence: &AllocationEvidence,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationEvidence {
    RuntimeSpill(optimization_core::PostAllocationOptimizationManifestIdentity),
    RegisterHomes(StagedOptimizedRegisterHomeCustodyReceipt),
    FixedViewCopies(StagedOptimizedPostCopyRegisterHomeCustodyReceipt),
    ActiveResidentRematerialization(StagedOptimizedActiveResidentRematerializationCustodyReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationAdmissionError {
    MachineEffects(selected_instruction_analyses::MachineEffectStageError),
    Preservation(crate::AllocatedCalleeSavedRequirementError),
}

stage_error_display!(AllocationAdmissionError, "allocation admission failed");
