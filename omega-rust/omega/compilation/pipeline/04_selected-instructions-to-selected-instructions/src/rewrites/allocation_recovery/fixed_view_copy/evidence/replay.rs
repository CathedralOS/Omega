//! Independently keyed reconstruction of segment-home boundary authority.

use std::collections::{BTreeMap, BTreeSet};

use optimization_core::OptimizationWorkUsage;
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::MachineId;

use crate::{
    FixedViewCopyError, ValidatedAllocationLegality, ValidatedFixedPrecoloredIntervals,
    ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges,
};
use register_homes::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSegmentHomePolicy,
    FixedPrecoloredSourceSegmentHome, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPolicy,
    FunctionFixedPrecoloredSegmentHomes, FunctionFixedPrecoloredSplitRequirements,
};

use super::{AuthenticatedFixedViewBoundary, FixedViewBoundaryEvidence};
