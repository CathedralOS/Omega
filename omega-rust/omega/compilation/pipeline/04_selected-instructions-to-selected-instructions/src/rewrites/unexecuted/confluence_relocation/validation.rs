//! Independent replay of the confluence run relocation: the validator
//! reconstructs the run's block, the join the named destination sits in,
//! and the landing index from the source on its own audit — sharing no
//! state with the producer's `admission` record — then requires the
//! proposed program to place exactly the run's members on the landing
//! index in the join block in their original order, and restores the
//! complete source by content, every crossed instruction, every other
//! block and instruction, register, roster row, call, settlement, and
//! edge included. A producer admission error therefore fails validation
//! even when the proposal is exactly what that producer emitted.

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::transport_conflict;
use crate::rewrites::sealed;
use crate::rewrites::window_hazards::{coupled, has_call_contract, schedulable, surface};
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
