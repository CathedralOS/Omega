//! Sorted fixed-use and transition rows used only by canonical replay.

use super::super::{SiteKey, site_key, site_point};
use std::collections::BTreeSet;

use register_model::{RegisterOperandAccess, RegisterViewId};

use crate::FixedPrecoloredSplitRequirementError;
use register_homes::{EntryFixedViewTransition, FixedPrecoloredInterval, VirtualPointLegality};
use selected_instructions::VirtualFixedConstraintSite;
