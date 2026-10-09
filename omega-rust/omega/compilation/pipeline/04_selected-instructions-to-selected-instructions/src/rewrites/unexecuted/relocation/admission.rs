//! The one relocation admission: locate the contiguous member run the two
//! named members bound inside one block's body, resolve the destination
//! instruction to a landing position in its block, take the direction whose
//! acyclic paths exist — downstream to a reachable destination, upstream
//! from one that reaches the run's block — derive the window the move
//! crosses over every such path, prove the move changes no traversal —
//! every predecessor edge into the arrival block lies on a crossed path and
//! every exit of every crossed block stays on one — then apply the shared
//! hazard, transport, memory-roster and settlement audit once.
use crate::ValidatedSelectedAnalysis;
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;
