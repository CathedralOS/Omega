//! Independent replay of the inflow relocation: the validator
//! reconstructs the member's join block, the inflow predecessor the
//! destination names, and the landing index inside it from the source on
//! its own audit — sharing no state with the producer's `admission`
//! record — then requires the proposed program to place exactly the
//! member on the landing index and restores the complete source by
//! content, every crossed instruction, every other block and
//! instruction, register, roster row, call, settlement, and edge
//! included.

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_successors, transport_conflict};
use crate::rewrites::sealed;
use crate::rewrites::window_hazards::{
    block_scan_surface, coupled, has_call_contract, register_writes, schedulable, surface,
};
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedFunction, SelectedInstructionId, SelectedInstructionPlan, terminator_instruction,
};
