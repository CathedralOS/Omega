use register_homes::PostAllocationSelectedTransformation;

use crate::{
    PreAllocationLedger, PreAllocationResult, SelectedLoweringLedger, SelectedLoweringResult,
};
use selected_instruction_analyses::{ValidatedAllocationLegality, ValidatedLiveRanges};

pub(super) fn selected_lowering_final_analysis(
    run: &SelectedLoweringResult,
) -> (&ValidatedLiveRanges, &ValidatedAllocationLegality) {
    (run.ranges(), run.legality())
}

/// The pre-allocation run's final ranges and legality: the last committed
/// step's rebuilt facts, or the run's admitted analyses when the clean pass
/// found no admissible copy.
pub(super) fn pre_allocation_final_analysis(
    run: &PreAllocationResult,
) -> (&ValidatedLiveRanges, &ValidatedAllocationLegality) {
    (run.ranges(), run.legality())
}

pub(super) fn pre_allocation_transformations(
    source: &PreAllocationLedger,
) -> Vec<PostAllocationSelectedTransformation> {
    source.transformations().to_vec()
}

pub(super) fn selected_lowering_transformations(
    source: &SelectedLoweringLedger,
) -> Vec<PostAllocationSelectedTransformation> {
    source.transformations().to_vec()
}
