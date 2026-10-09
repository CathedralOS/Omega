//! Independent validation of boundary-decided predicate materialization.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the named instruction's
//! ordering `MaterializeBoolean*` kind and emitted `[def result]` shape,
//! the flag units it reads resolving through the shared condition-state
//! walk to one compare's published definitions, the compare's resolved
//! operand poles, and — for a decided predicate — the target's own
//! materialize row the rewritten instruction is built from — then
//! rebuilds the instruction the contract demands and requires the
//! proposal to carry it at the materialization's position. Reinserting
//! the source boolean must restore the complete source by content: every
//! other instruction, register, call, and function included. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::ConditionStateError;
use crate::rewrites::sealed;
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use semantic_vocabulary::IntegerValue;

/// What the pole operand forces on the reader's predicate over
/// `left - right`, in the validator's own record: the same two outcomes
/// the contract defines, re-decided here so a wrong boundary decision
/// inside the producer's `outcome` cannot pass validation.

/// The validator's own reconstruction of the fold the contract permits:
/// the admitted materialization's coordinates and emitted provenance, the
/// boolean result register and decided outcome, the target's materialize
/// row a constant fold is built from, the resolved-index successor lists
/// the shared walk ran over, and the flag-use count for the measured-step
/// contract. It shares no state with the producer's `admission` record.
use crate::rewrites::block_edges::member_position;
