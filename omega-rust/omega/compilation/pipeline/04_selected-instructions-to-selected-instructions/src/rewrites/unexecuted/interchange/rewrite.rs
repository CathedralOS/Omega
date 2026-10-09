//! Proposal of the in-block commuting run interchange: admit the window
//! the two named runs bound, then reorder the window to the later run, the
//! interior, and the earlier run — the run interchange's own splice — and
//! rewrite the window's roster rows into the new execution order.
//! Validation independently re-derives the transform and restores the
//! source by content.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::admission;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::commuting_accesses as accesses;
