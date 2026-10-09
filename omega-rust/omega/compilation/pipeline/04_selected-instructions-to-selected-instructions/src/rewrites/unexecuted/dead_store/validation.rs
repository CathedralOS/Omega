//! Independent validation of dead-store elimination.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! store shape against the bound constraint row, the write row's place and
//! byte extent (or the fixed bytes a resolved `index`/`length` collapses
//! the dynamic extent to), the forward walk that must find one covering
//! write on every reachable path or confine the path to clear blocks
//! forever, the crossed-edge and boundary-settlement audits, and the
//! dropped-row contract — then rebuilds the function the contract demands
//! and requires the proposal to equal it. Restoring the store and its
//! roster rows must reproduce the complete source by content. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedStructuralTransport, SelectedSuccessor,
    terminator_instruction,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPlaceDeclaration;

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{member_position, terminator_successors};
use crate::rewrites::sealed;
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, constant_index, extent_intersects, extent_reached_by,
    local_slot_is_place_storage, local_store_shape, slot_is_subject_storage, staging_slot,
    structural_place_declarations, write_role_covers_subject,
};
use crate::rewrites::window_hazards::{UnrosteredReach, unrostered_reach};
