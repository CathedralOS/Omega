/*
We route `stage_register_allocation` across identity inputs, completed
`SelectedLowering` and `PreAllocation` optimization runs, declared allocation
recovery rules (`SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` and
`ActiveResidentImmediateU64MultiUseRematerializationV1`), direct baseline home
assignment, and runtime-spill pressure recovery.
*/

use crate::assignment::recovery::{
    stage_active_resident_register_allocation, stage_pre_allocation_register_allocation,
    stage_selected_lowering_register_allocation, stage_shared_entry_fixed_view_register_allocation,
};
use optimization_core::{Optimization, OptimizationExecutionPhase};

use crate::{
    AllocationAdmissionError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedRegisterHomeCustodyError, RetainedAllocation,
};
use crate::{AllocationRecoveryRuleCatalogError, selected_allocation_recovery_rule};
use crate::{
    OptimizedFixedPrecoloredSegmentHomeCustodyError, OptimizedFixedViewCopyCustodyError,
    OptimizedSelectedReanalysisError,
};
use selected_instruction_analyses::SelectedOptimizationOutput;
use selected_instruction_analyses::{
    OptimizedAllocationLegalityCustodyError, stage_optimized_allocation_legality,
};

pub fn stage_register_allocation(
    selected: SelectedOptimizationOutput,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    let ranges = match selected {
        SelectedOptimizationOutput::Identity(ranges) => ranges,
        SelectedOptimizationOutput::SelectedLowering(run) => {
            return stage_selected_lowering_register_allocation(run);
        }
        SelectedOptimizationOutput::PreAllocation(run) => {
            return stage_pre_allocation_register_allocation(run);
        }
    };
    let selections = ranges.selections().clone();
    let recovery = selected_allocation_recovery_rule(
        &selections.project_phase(OptimizationExecutionPhase::AllocationRecovery),
    )
    .map_err(RegisterAllocationError::RecoveryCatalog)?;
    if let Some(rule) = recovery {
        return match rule {
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 => {
                let legality = stage_optimized_allocation_legality(ranges)
                    .map_err(RegisterAllocationError::Legality)?;
                stage_shared_entry_fixed_view_register_allocation(legality)
            }
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
                stage_active_resident_register_allocation(ranges)
            }
            _ => Err(RegisterAllocationError::UnsupportedComposition),
        };
    }
    let legality =
        stage_optimized_allocation_legality(ranges).map_err(RegisterAllocationError::Legality)?;
    let mut home_reuse = crate::assignment::home_assignment::compute::FunctionHomeReuse::default();
    let assignment =
        match crate::assignment::runtime_spill::assign_source_reusing(&legality, &mut home_reuse) {
            Ok(homes) => homes,
            Err(error) if error.admits_runtime_recovery() => {
                let recovered =
                    crate::assignment::runtime_spill::recover_with_reuse(legality, home_reuse)
                        .map_err(RegisterAllocationError::RuntimeSpill)?;
                return RetainedAllocation::try_from(recovered)
                    .map_err(RegisterAllocationError::Admission);
            }
            Err(error) => {
                return Err(RegisterAllocationError::Homes(
                    OptimizedRegisterHomeCustodyError::Assignment(error),
                ));
            }
        };
    let homes =
        crate::assignment::baseline::stage_register_homes_with_assignment(legality, assignment)
            .map_err(RegisterAllocationError::Homes)?;
    RetainedAllocation::try_from(homes).map_err(RegisterAllocationError::Admission)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterAllocationError {
    UnsupportedComposition,
    RecoveryCatalog(AllocationRecoveryRuleCatalogError),
    FixedSegments(OptimizedFixedPrecoloredSegmentHomeCustodyError),
    FixedViewCopies(OptimizedFixedViewCopyCustodyError),
    FixedViewHomes(crate::OptimizedPostCopyRegisterHomeCustodyError),
    Reanalysis(OptimizedSelectedReanalysisError),
    Rematerialization(crate::OptimizedActiveResidentRematerializationError),
    RuntimeSpill(crate::RuntimeSpillAllocationError),
    Legality(OptimizedAllocationLegalityCustodyError),
    Homes(OptimizedRegisterHomeCustodyError),
    TransformedHomes(OptimizedPostSelectedLoweringHomeCustodyError),
    PreAllocationHomes(crate::OptimizedPostPreAllocationHomeCustodyError),
    Admission(AllocationAdmissionError),
}

stage_error_display!(RegisterAllocationError, "register allocation failed");

#[cfg(test)]
pub(crate) mod route_tests {
    #[test]
    fn shared_entry_route_probes_segment_homes_before_committing_the_sequence() {
        let source = include_str!("assignment.rs");
        let entrance = source
            .split("fn fixed_view_allocation")
            .nth(1)
            .expect("the composing sequence body");
        let gate = entrance
            .find("SharedEntryAfterCompareBeforeBranchV1")
            .expect("the shared-entry policy gate");
        let probe = entrance
            .find("probe_optimized_fixed_precolored_segment_homes")
            .expect("the borrow-only segment-home probe");
        let decline = entrance
            .find("recover_after_declined_fixed_view_probe")
            .expect("the declined-sequence spill hand-off");
        let sequence = entrance
            .find("fixed_view_reanalysis(legality, policy)")
            .expect("the custody-consuming sequence");
        let post_copy = entrance
            .find("recover_after_fixed_view_copies")
            .expect("the post-copy spill arm");
        assert!(gate < probe && probe < decline && decline < sequence);
        assert!(sequence < post_copy);
    }

    #[test]
    fn completed_run_evidence_routes_through_the_composing_sequences() {
        let source = include_str!("register_allocation.rs");
        let entrance = source
            .split("fn stage_register_allocation")
            .nth(1)
            .expect("the stage entrance body");
        let folds_arm = entrance
            .find("SelectedOptimizationOutput::SelectedLowering(run)")
            .expect("the selected-lowering output arm");
        let lowering_composition = entrance
            .find("stage_selected_lowering_register_allocation(run)")
            .expect("the selected-lowering composing route");
        let pre_allocation_arm = entrance
            .find("SelectedOptimizationOutput::PreAllocation(run)")
            .expect("the pre-allocation output arm");
        let pre_allocation_composition = entrance
            .find("stage_pre_allocation_register_allocation(run)")
            .expect("the pre-allocation composing route");
        assert!(folds_arm < lowering_composition && lowering_composition < pre_allocation_arm);
        assert!(pre_allocation_arm < pre_allocation_composition);
    }

    #[test]
    fn selected_lowering_route_composes_residual_pressure_into_runtime_spill() {
        let source = include_str!("assignment.rs");
        let entrance = source
            .split("fn stage_selected_lowering_register_allocation")
            .nth(1)
            .expect("the selected-lowering route body");
        let probe = entrance
            .find("assign_optimized_register_homes_after_selected_lowering")
            .expect("the borrow-only assignment probe");
        let homes = entrance
            .find("stage_optimized_register_homes_after_selected_lowering_with_assignment")
            .expect("the post-probe homes arm");
        let spill = entrance
            .find("recover_after_selected_lowering")
            .expect("the residual-pressure spill arm");
        let typed = entrance
            .rfind("OptimizedPostSelectedLoweringHomeCustodyError::Assignment(error)")
            .expect("the unrecovered assignment error surface");
        assert!(probe < homes && homes < spill && spill < typed);
    }

    #[test]
    fn pre_allocation_route_composes_residual_pressure_into_runtime_spill() {
        let source = include_str!("assignment.rs");
        let entrance = source
            .split("fn stage_pre_allocation_register_allocation")
            .nth(1)
            .expect("the pre-allocation route body");
        let probe = entrance
            .find("assign_optimized_register_homes_after_pre_allocation")
            .expect("the borrow-only assignment probe");
        let homes = entrance
            .find("stage_optimized_register_homes_after_pre_allocation_with_assignment")
            .expect("the post-probe homes arm");
        let spill = entrance
            .find("recover_after_pre_allocation")
            .expect("the residual-pressure spill arm");
        let typed = entrance
            .rfind("OptimizedPostPreAllocationHomeCustodyError::Assignment(error)")
            .expect("the unrecovered assignment error surface");
        assert!(probe < homes && homes < spill && spill < typed);
    }

    #[test]
    fn completed_run_recovery_records_the_run_custody_as_its_prefix() {
        let source = include_str!("assignment.rs");
        let projection = source
            .split("fn resolve")
            .nth(1)
            .expect("the resolved source family");
        let lowering_completion = projection
            .find("selected_lowering_completion: Some(upstream.identity())")
            .expect("the bound suite completion");
        let pre_allocation_completion = projection
            .find("pre_allocation_completion: Some(upstream.identity())")
            .expect("the bound suite completion");
        assert!(lowering_completion < pre_allocation_completion);
        assert_eq!(
            projection
                .matches("prefix: upstream.transformations().to_vec()")
                .count(),
            2
        );
    }

    #[test]
    fn active_resident_route_composes_residual_pressure_into_runtime_spill() {
        let source = include_str!("assignment.rs");
        let entrance = source
            .split("fn stage_active_resident_register_allocation")
            .nth(1)
            .expect("the active-resident route body");
        let prefix = entrance
            .find("stage_optimized_active_resident_rematerialization_pressure")
            .expect("the proven rematerialization prefix");
        let assignment = entrance
            .find("crate::assign_register_homes(")
            .expect("the post-prefix assignment probe");
        let completion = entrance
            .find("complete_optimized_active_resident_rematerialization")
            .expect("the terminal completion");
        let spill = entrance
            .find("recover_after_active_resident_rematerialization")
            .expect("the residual-pressure spill arm");
        let typed = entrance
            .rfind("OptimizedActiveResidentRematerializationError::Homes(error)")
            .expect("the unrecovered assignment error surface");
        assert!(prefix < assignment && assignment < completion && completion < spill);
        assert!(spill < typed);
    }

    #[test]
    fn retained_replay_binds_leaf_local_evidence_to_no_declared_recovery_selection() {
        let source = include_str!("output.rs");
        let check = source
            .split("fn validate_recovery_selection")
            .nth(1)
            .expect("the recovery-selection check");
        let shared_entry = check
            .find("SharedEntryAfterCompareBeforeBranchV1")
            .expect("the shared-entry policy arm");
        let leaf_local = check
            .find("LeafLocalBeforeFixedUseV1")
            .expect("the leaf-local policy arm");
        assert!(shared_entry < leaf_local);
        assert!(check[..leaf_local].contains("source().source().policy()"));
    }
}
