//! Independently reconstructed disjoint union of source-rooted fragment
//! trees, keyed by block identity rather than positional assumptions.

use std::collections::{HashMap, HashSet};

use crate::FixedPrecoloredSplitRequirementError;
use register_homes::{VirtualPointLegality, VirtualRegisterAllocationLegality};
use selected_instructions::{LiveRangeEdgeConnector, LiveRangeFragment, VirtualLiveRange};
