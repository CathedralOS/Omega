//! Independent per-boundary fixed-site copy reconstruction for the leaf-local
//! and immediate-site policies.
use std::collections::BTreeSet;

use register_model::{RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{SelectedInstructionId, VirtualRegisterId, VirtualRegisterOrigin};

use crate::{FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError};
use selected_instructions::VirtualFixedConstraintSite;
