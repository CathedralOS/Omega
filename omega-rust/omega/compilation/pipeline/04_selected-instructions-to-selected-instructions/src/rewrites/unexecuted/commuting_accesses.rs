//! Memory-roster commutation for the scheduling rewrites under proven
//! access independence: which two recorded accesses may trade order
//! without either observing the other's effect, and the row permutation
//! that keeps the roster in execution order when they do. The commuting
//! interchange families — `commuting_interchange` at pair granularity,
//! `commuting_member_run_interchange` at member-against-run granularity,
//! and `commuting_run_interchange` at run granularity — share this one
//! audit; the families that keep recorded accesses in place never consult
//! it.
//!
//! The overlap reading is the roster's own, owned by `place_storage`: each
//! row's storage route comes from `row_storage`, fixed extents compare
//! through `ranges_disjoint`, and a local-slot row reaches a place's bytes
//! exactly when the slot is that place's own storage. This audit applies
//! the same reading symmetrically between two rows.
use selected_instructions::{
    SelectedFunction, SelectedInstructionId, SelectedMemoryAccess, SelectedMemoryAccessRole,
};
use terminal_psi::StructuralPlaceDeclaration;

use crate::rewrites::unexecuted::place_storage::local_slot_is_place_storage;
