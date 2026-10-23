use register_model::ValidatedPhysicalRegisterModel;

use crate::FixedPrecoloredSegmentHomeError;
use register_homes::{
    FunctionFixedPrecoloredSegmentHomes, FunctionFixedPrecoloredSplitRequirements,
};
use selected_instructions::FunctionLiveRanges;

use super::super::domains;
use super::{conflicts, placement};
use crate::work_accounting::Work;

pub(super) fn derive(
    requirements: &[FunctionFixedPrecoloredSplitRequirements],
    ranges: &[std::sync::Arc<FunctionLiveRanges>],
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut Work,
) -> Result<Vec<FunctionFixedPrecoloredSegmentHomes>, FixedPrecoloredSegmentHomeError> {
    if requirements.len() != ranges.len() {
        return Err(FixedPrecoloredSegmentHomeError::RootMismatch);
    }
    requirements
        .iter()
        .zip(ranges)
        .enumerate()
        .map(|(function, (requirements, ranges))| {
            if requirements.machine != ranges.machine {
                return Err(FixedPrecoloredSegmentHomeError::FunctionMismatch { function });
            }
            let domains = domains::build(function, requirements, work)?;
            let conflicts = conflicts::build(function, &domains, ranges, physical, work)?;
            placement::assign(function, ranges.machine, &domains, &conflicts, ranges, work)
        })
        .collect()
}
