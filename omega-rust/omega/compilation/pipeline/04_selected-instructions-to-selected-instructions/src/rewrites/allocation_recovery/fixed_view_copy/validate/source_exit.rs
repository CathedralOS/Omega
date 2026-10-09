//! Independent shared-source-exit copy reconstruction: replay re-derives the
//! same boundary partition and rebuilds each emitted copy from current facts
//! alone — the shared dominating point from connector evidence, member site
//! blocks from the source function, and the admission gate from the policy.
use std::collections::BTreeSet;

use register_model::{RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{SelectedInstructionId, VirtualRegisterId, VirtualRegisterOrigin};

use crate::{FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPolicy};
use selected_instructions::VirtualFixedConstraintSite;
