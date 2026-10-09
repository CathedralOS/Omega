//! Member-run relocation through one shared admission on the selected CFG.
//!
//! The per-shape scheduling families — `edge`, `predecessor`, `diamond`,
//! `join`, `fork`, `arm`, `bypass`, `triangle`, `confluence`, `inflow` and
//! their run variants — each enumerate the window their own shape crosses
//! and spell the independence audit by hand. This module is the general
//! mechanism the execution board's scheduling flag names: one admission
//! over a contiguous member run and a destination point. Given
//! `first_member`..`last_member` in one block's body and a `destination`
//! instruction anywhere in the function, admission derives the window the
//! move crosses itself — every acyclic edge path between the run's block
//! and the destination block contributes its terminator-carried
//! instruction and successor row, every intermediate block contributes
//! its whole body, the run's own block contributes the tail behind the
//! run, and the destination block contributes the positions before the
//! landing index — then proves the move changes no traversal: every
//! predecessor edge into the destination lies on a crossed path (no
//! traversal gains the run) and every exit of every block any path
//! leaves reaches the destination (no traversal loses the run). The
//! shared audit then runs once: every member schedulable, no member
//! coupled with a crossed position or a crossed edge's terminator
//! instruction, every crossed edge a plain semantic successor free of
//! register-transport interference, roster-carrying runs crossing only
//! unaccounted positions, and no boundary settlement whose observed
//! executed prefix changes — including settlements inside fully crossed
//! intermediate bodies.
//!
//! A run named inside the same block it lands in is the in-block move the
//! local families own: the window is the span between the run and the
//! landing index and the traversal proof is vacuous. Moves whose gained
//! or lost traversal set is nonempty but still sound — the fork-head
//! hoists and join sinks — are the `dead_path` audit's families and stay
//! with them; this admission proves the preservation case.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: every member keeps its own id, kind,
//! operands, provenance, and implicit surface while only the run's
//! position in the program moves. Proposal and independent replay share
//! only the admission predicates. Validation consumes the proposed program,
//! requires the run to sit at the landing index in its original order,
//! and restores the complete source by content — every other block,
//! instruction, register, roster row, call, and settlement is retained
//! bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, instruction_position,
    terminator_instruction,
};

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{edge_surface, member_position, terminator_successors};

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;
