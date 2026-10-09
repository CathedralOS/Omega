//! Optimizer module role: stage group. Independently keyed split replay.

mod cuts;
mod function;
mod indexes;
mod partition;
mod topology;
mod work;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FixedPrecoloredSplitRequirementError, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedLiveRanges,
};
use register_homes::{
    FixedPrecoloredSplitRequirementPolicy, FunctionAllocationLegality,
    FunctionFixedPrecoloredIntervals, FunctionFixedPrecoloredSplitRequirements,
};
use selected_instructions::FunctionLiveRanges;
