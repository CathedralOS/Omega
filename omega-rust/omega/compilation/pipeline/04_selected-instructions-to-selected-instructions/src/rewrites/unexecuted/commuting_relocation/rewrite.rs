//! Proposal of the in-block commuting run relocation: admit the window the
//! named run and destination bound, then rotate the run onto the
//! destination's position — the crossed positions shifting one run-width
//! toward the run's vacated span — and rewrite the window's roster rows
//! into the new execution order. Validation independently re-derives the
//! transform and restores the source by content.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::admission;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::commuting_accesses as accesses;
