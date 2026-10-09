//! Keyed function reconstruction for independent replay.

use super::super::early_clobber_uses;
use std::collections::{BTreeMap, BTreeSet};

use selected_instructions::VirtualRegisterId;

use crate::FixedPrecoloredSplitRequirementError;
use register_homes::{
    FixedPrecoloredInterval, FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionFixedPrecoloredSplitRequirements, VirtualRegisterAllocationLegality,
};
use selected_instructions::FunctionLiveRanges;

use super::{partition, work};
// Mirrors the compute admission: early-clobber participants partition
// identically because no `early_clobber_points` row enters a source domain;
// the hazard is enforced on the reanalyzed plan. Only the use count feeds
// work accounting.
