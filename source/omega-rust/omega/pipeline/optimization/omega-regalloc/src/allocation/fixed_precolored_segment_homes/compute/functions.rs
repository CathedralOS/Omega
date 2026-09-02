use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    FixedPrecoloredSegmentHomeError, FunctionFixedPrecoloredSegmentHomes,
    FunctionFixedPrecoloredSplitRequirements, FunctionLiveRanges,
};

use super::{conflicts, domains, placement, work::Work};

pub(super) fn derive(
    requirements: &[FunctionFixedPrecoloredSplitRequirements],
    ranges: &[FunctionLiveRanges],
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
            placement::assign(function, ranges.machine, &domains, &conflicts, work)
        })
        .collect()
}
