//! Independent register partition replay.

use std::collections::{BTreeMap, BTreeSet};

use register_model::RegisterViewId;

use crate::FixedPrecoloredSplitRequirementError;
use register_homes::{
    FixedPrecoloredInterval, FixedPrecoloredRegisterSplitRequirements,
    FixedPrecoloredSourceFragmentRequirements, FixedPrecoloredSourceSegment,
    FixedPrecoloredSourceSegmentId, FixedPrecoloredSourceSegmentOpening, VirtualPointLegality,
    VirtualRegisterAllocationLegality,
};
use selected_instructions::{LiveRangePoint, VirtualLiveRange};
