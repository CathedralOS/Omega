/*
We project each staged register-allocation route into a unified
`AllocationOutput` borrow and construct `RetainedAllocation`, verifying that
any exercised recovery matches the build's declared `AllocationRecovery`
phase selection before capturing current allocation facts for `Omega 06`.
*/

macro_rules! allocation_output {
    ($self:expr, $placed:expr, $selected:expr, $evidence:expr) => {
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
                AllocationEvidence::RegisterHomes(self.custody().to_owned())
            )
        }
    }
}
pub(crate) mod current {
    use std::sync::Arc;

    use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};
    use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
    use register_environment::ValidatedTargetRegisterEnvironment;
    use register_homes::{AllocatedProgram, AllocatedProgramFacts};
    use selected_instruction_analyses::SelectedProgramView;
    use selected_instruction_analyses::{
        MachineEffectStageError, OwnedSelectedProgram, SelectedProgramRef,
        ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
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
        homes: ValidatedRegisterHomes,
        manifest: ValidatedPostAllocationOptimizationManifest,
        environment: ValidatedTargetRegisterEnvironment,
        effects: Arc<PreAllocationMachineEffectPlan>,
        evidence: AllocationEvidence,
        selections: OptimizationSelections,
        budget: OptimizationWorkBudget,
    }

    impl CurrentAllocation {
        pub(crate) fn capture(
            source: &AllocationOutput<'_>,
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
            })
        }

        pub(crate) fn facts(&self) -> AllocatedProgramFacts<'_> {
            AllocatedProgramFacts {
                selected: self.selected.current_program(),
                effects: &self.effects,
                ranges: self.ranges.plan(),
                ranges_identity: self.ranges.receipt().identity(),
                legality: self.legality.plan(),
                legality_identity: self.legality.receipt().identity(),
                homes: self.homes.plan(),
                homes_identity: self.homes.receipt().identity(),
                manifest: self.manifest.record(),
            }
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
                AllocationEvidence::FixedViewCopies(self.custody().to_owned())
            )
        }
    }
}
pub(crate) mod literal_folds {
    use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
    use crate::StagedOptimizedRegisterHomesAfterSelectedLowering;

    impl ProjectAllocation for StagedOptimizedRegisterHomesAfterSelectedLowering {
        fn project_allocation(&self) -> AllocationOutput<'_> {
            let program = self.selected();
            allocation_output!(
                self,
                program.plan(),
                program,
                AllocationEvidence::SelectedLowering(self.custody().to_owned())
            )
        }
    }
}
pub(crate) mod pre_allocation {
    use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
    use crate::StagedOptimizedRegisterHomesAfterPreAllocation;

    impl ProjectAllocation for StagedOptimizedRegisterHomesAfterPreAllocation {
        fn project_allocation(&self) -> AllocationOutput<'_> {
            let program = self.selected();
            allocation_output!(
                self,
                program.plan(),
                program,
                AllocationEvidence::PreAllocation(self.custody().to_owned())
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
                AllocationEvidence::ActiveResidentRematerialization(self.custody().to_owned(),)
            )
        }
    }
}
pub(crate) mod retained {
    use super::{AllocationAdmissionError, AllocationOutput, ProjectAllocation};
    use crate::FixedViewCopyPolicy;
    use crate::{
        StagedOptimizedActiveResidentRematerialization, StagedOptimizedRegisterHomes,
        StagedOptimizedRegisterHomesAfterFixedViewCopies,
        StagedOptimizedRegisterHomesAfterPreAllocation,
        StagedOptimizedRegisterHomesAfterSelectedLowering,
    };

    #[derive(Debug)]
    pub struct RetainedAllocation {
        current: super::current::CurrentAllocation,
    }

    impl RetainedAllocation {
        pub fn program(&self) -> &register_homes::AllocatedProgram {
            &self.current.program
        }

        pub fn current(&self) -> AllocationOutput<'_> {
            self.current.view()
        }

        pub fn facts(&self) -> register_homes::AllocatedProgramFacts<'_> {
            self.current.facts()
        }

        fn capture(
            current: &AllocationOutput<'_>,
            runtime_spill_prefix_selection: Option<optimization_core::Optimization>,
        ) -> Result<Self, AllocationAdmissionError> {
            validate_recovery_selection(current, runtime_spill_prefix_selection)?;
            Ok(Self {
                current: super::current::CurrentAllocation::capture(current)
                    .map_err(AllocationAdmissionError::MachineEffects)?,
            })
        }
    }

    macro_rules! retained_allocation_from_staged {
        ($($source:ty),+ $(,)?) => {
            $(
                impl TryFrom<$source> for RetainedAllocation {
                    type Error = AllocationAdmissionError;

                    fn try_from(source: $source) -> Result<Self, Self::Error> {
                        Self::capture(&source.project_allocation(), None)
                    }
                }
            )+
        };
    }

    retained_allocation_from_staged!(
        StagedOptimizedRegisterHomes,
        StagedOptimizedRegisterHomesAfterFixedViewCopies,
        StagedOptimizedRegisterHomesAfterSelectedLowering,
        StagedOptimizedRegisterHomesAfterPreAllocation,
        StagedOptimizedActiveResidentRematerialization,
    );

    impl TryFrom<crate::assignment::runtime_spill::RuntimeSpillAllocation> for RetainedAllocation {
        type Error = AllocationAdmissionError;

        fn try_from(
            source: crate::assignment::runtime_spill::RuntimeSpillAllocation,
        ) -> Result<Self, Self::Error> {
            Self::capture(
                &source.project_allocation(),
                source.recovery_prefix_selection(),
            )
        }
    }

    fn validate_recovery_selection(
        current: &AllocationOutput<'_>,
        runtime_spill_prefix_selection: Option<optimization_core::Optimization>,
    ) -> Result<(), AllocationAdmissionError> {
        use super::AllocationEvidence;
        use optimization_core::{Optimization, OptimizationExecutionPhase};
        let expected: &[Optimization] = match current.evidence() {
            AllocationEvidence::FixedViewCopies(receipt) => {
                match receipt.source().source().policy() {
                    FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => {
                        &[Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1]
                    }
                    FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1
                    | FixedViewCopyPolicy::ImmediateBeforeFixedUseV1
                    | FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1 => &[],
                }
            }
            AllocationEvidence::RuntimeSpill(_) => match runtime_spill_prefix_selection {
                Some(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1) => {
                    &[Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1]
                }
                Some(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1) => {
                    &[Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1]
                }
                _ => &[],
            },
            AllocationEvidence::ActiveResidentRematerialization(_) => {
                &[Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1]
            }
            AllocationEvidence::RegisterHomes(_)
            | AllocationEvidence::SelectedLowering(_)
            | AllocationEvidence::PreAllocation(_) => &[],
        };
        if current
            .selections()
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .as_slice()
            != expected
        {
            return Err(AllocationAdmissionError::SelectionMismatch);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) mod tests {
        #[test]
        fn every_retained_constructor_checks_the_recovery_selection_before_capture() {
            let source = include_str!("output.rs")
                .split("mod retained {")
                .nth(1)
                .expect("the retained module")
                .split("#[cfg(test)]")
                .next()
                .expect("the retained production body");
            let rows = source
                .split("retained_allocation_from_staged!(")
                .nth(1)
                .expect("the staged-result row list")
                .split(");")
                .next()
                .expect("the row list body");
            assert_eq!(rows.matches("StagedOptimized").count(), 5);
            let constructors = source.split("impl TryFrom<").skip(1).collect::<Vec<_>>();
            assert_eq!(constructors.len(), 2);
            for constructor in &constructors {
                assert!(constructor.contains("Self::capture("));
            }
            let capture = source
                .split("fn capture(")
                .nth(1)
                .expect("the shared capture");
            let policy = capture
                .find("validate_recovery_selection(current")
                .expect("the recovery-selection check");
            let owned = capture
                .find("CurrentAllocation::capture(current)")
                .expect("the owned current facts");
            assert!(policy < owned);
            assert_eq!(
                source
                    .matches("Self::capture(&source.project_allocation(), None)")
                    .count(),
                1
            );
            assert!(source.contains("source.recovery_prefix_selection(),"));
            assert!(!source.contains("replay_allocation"));
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
                evidence: AllocationEvidence::RuntimeSpill(self.manifest.record().identity),
            }
        }
    }
}

use crate::{
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
    StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes,
};
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
pub use retained::RetainedAllocation;
use selected_instruction_analyses::{
    SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
};

trait ProjectAllocation {
    fn project_allocation(&self) -> AllocationOutput<'_>;
}

#[derive(Clone)]
pub struct AllocationOutput<'program> {
    program: register_homes::AllocatedProgramRef<'program>,
    selected: SelectedProgramRef<'program>,
    liveness: &'program ValidatedLiveness,
    ranges: &'program ValidatedLiveRanges,
    legality: &'program ValidatedAllocationLegality,
    homes: &'program ValidatedRegisterHomes,
    manifest: &'program ValidatedPostAllocationOptimizationManifest,
    environment: &'program ValidatedTargetRegisterEnvironment,
    evidence: AllocationEvidence,
    selections: &'program OptimizationSelections,
    budget: OptimizationWorkBudget,
}

impl<'program> AllocationOutput<'program> {
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
        homes: &'program ValidatedRegisterHomes,
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
    SelectedLowering(StagedOptimizedPostSelectedLoweringHomeCustodyReceipt),
    PreAllocation(StagedOptimizedPostPreAllocationHomeCustodyReceipt),
    ActiveResidentRematerialization(StagedOptimizedActiveResidentRematerializationCustodyReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationAdmissionError {
    SelectionMismatch,
    MachineEffects(selected_instruction_analyses::MachineEffectStageError),
}

stage_error_display!(AllocationAdmissionError, "allocation admission failed");
