//! Independently keyed reconstruction of fixed/precolored point intervals.

use std::collections::BTreeMap;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::RegisterOperandAccess;

use crate::{FixedPrecoloredIntervalError, ValidatedAllocationLegality, ValidatedLiveRanges};
use register_homes::{
    FixedPrecoloredInterval, FixedPrecoloredIntervalPolicy, FunctionAllocationLegality,
    FunctionFixedPrecoloredIntervals,
};
use selected_instructions::{FunctionLiveRanges, LiveRangePoint, VirtualFixedConstraintSite};
