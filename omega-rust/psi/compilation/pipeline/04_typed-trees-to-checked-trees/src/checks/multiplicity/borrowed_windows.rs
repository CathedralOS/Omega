//! Borrowed-storage invariant windows through exclusive borrows (ch11).
//!
//! A consuming move out of `&mut`-reachable storage opens a repair obligation
//! on the exact absent place: while the window is open the place is absent
//! from the owner, so every read, borrow, move, or store that touches it or an
//! enclosing owner rejects, and every exit edge (transition or return)
//! requires the obligation discharged. The replacement is an ordinary store
//! of a value carrying the hole's exact type; disjoint sibling work between
//! the extraction and the repair is unrestricted.
//!
//! Windows are keyed on the resolved *storage* place, not the access route:
//! a hole opened through a local `&mut` reborrow (`let r = &mut self.f; let x
//! = r.g`) is the same hole the owner path (`self.f.g = ..`) repairs, because
//! both name one referent subtree. Repair route liveness stays with the
//! ordinary exclusive-loan checks; the window only records that the place is
//! absent until a store of the required type reseats it.
//!
//! Extraction requires a readable exclusive chain: every reference crossed by
//! the moved place's prefixes (the `self` receiver for machine-rooted places,
//! a `&mut` parameter root, or a `&mut` field/local inside the path) must be
//! `&mut`. Shared and write-only links keep the existing plain rejection.
//!
//! Replay observes each evaluated place before applying its exact discovered
//! move, then checks invocations after their operands. Grouping all moves at
//! statement entry rejects earlier reads and calls; exempting all mentions of
//! a moved place hides later stale uses. The occurrence retained by discovery
//! distinguishes those cases without reclassifying ownership from syntax.
//! A repair store runs only after its value and destination selectors finish.

use crate::checks::borrows::StorageSelectorFacts;
use validation::CanonicalPlace;

use language_core::ReferenceAccess;
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) use arm_windows::{ArmWindowPlan, ArmWindowVerdict};
pub(super) use boundary_checks::{
    authored_route_exclusive, borrowed_transfer_diagnostic, check_call_occurrences,
    check_recorded_invocations, state_has_boundary_operator,
};
pub(super) use windows::BorrowedStorageWindows;

mod arm_windows;
mod boundary_checks;
mod windows;

/// The join-agreement comparison: two arms carry the same resolved debts,
/// regardless of the order their moves recorded them in.
fn same_place_set(
    program: &dyn ::validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    left: &[(facts::PlaceRoot, Vec<facts::PlaceSegment>)],
    right: &[(facts::PlaceRoot, Vec<facts::PlaceSegment>)],
) -> bool {
    left.len() == right.len()
        && left.iter().all(|place| {
            right.iter().any(|candidate| {
                same_storage_debt(
                    program,
                    captures,
                    (place.0, &place.1),
                    (candidate.0, &candidate.1),
                )
            })
        })
}

fn same_storage_debt(
    program: &dyn ::validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    left: (facts::PlaceRoot, &[facts::PlaceSegment]),
    right: (facts::PlaceRoot, &[facts::PlaceSegment]),
) -> bool {
    left.0 == right.0
        && crate::checks::borrows::storage_path_compatibility(program, captures, left.1, right.1).1
            == checked_trees::CapturedPlaceContainment::Same
}

/// Rebase a place written through a `&mut`-typed local onto the referent
/// storage it borrows. `let r = &mut self.f` makes `r.g` and `self.f.g` name
/// the same absent subtree; the loop also follows plain reference copies
/// (`let s = r`), recast chains, and later reassignments of the reference
/// local to the source the route currently captures.
fn resolve_storage_place(
    program: &dyn ::validation::CheckingInput,
    _machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    root_symbol: SymbolHandle,
    path: &[facts::PlaceSegment],
) -> (facts::PlaceRoot, Vec<facts::PlaceSegment>) {
    let mut root = facts::PlaceRoot::Symbol(root_symbol);
    let mut segments = path.to_vec();
    for _ in 0..16 {
        let Some(symbol) = root.as_symbol() else {
            break;
        };
        let Some(source) = crate::flow::reference_route_source(
            program,
            state,
            statements,
            statement_index,
            symbol,
        ) else {
            break;
        };
        let Some(source) =
            crate::flow::reference_source_place(program, state.symbol, statement_index, source)
        else {
            break;
        };
        let facts::PlaceRoot::Symbol(..) = source.root else {
            break;
        };
        let mut rebased = source.segments;
        rebased.extend_from_slice(&segments);
        root = source.root;
        let rebased_place = CanonicalPlace {
            root,
            segments: rebased,
        };
        // A `&mut self.f` capture may record the attached field itself as the
        // root; join it with its receiver storage so it keys like `self.f`.

        root = rebased_place.root;
        segments = rebased_place.segments;
    }
    (root, segments)
}

/// The access of a possibly-constrained reference type.
fn reference_access(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<ReferenceAccess> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { access, .. } => Some(*access),
        TypeReferenceNode::Constrained { base_type, .. } => reference_access(program, *base_type),
        _ => None,
    }
}

/// Every reference crossed by the extracted place must be `&mut`: the `self`
/// receiver for machine-rooted storage, then each proper prefix of the path
/// (a `&mut` parameter root, a `&mut` field inside owned storage, a `&mut`
/// local the window resolved through). Shared or write-only links cannot
/// carry a restoration window.
fn exclusive_storage_chain(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
) -> bool {
    if root == facts::PlaceRoot::Symbol(machine.symbol) {
        return false;
    }
    (0..=path.len()).all(|length| {
        let prefix = CanonicalPlace {
            root,
            segments: path[..length].to_vec(),
        };
        validation::canonical_place_type_reference(program, state.symbol, statement_index, &prefix)
            .and_then(|type_reference| reference_access(program, type_reference))
            .is_none_or(|access| access == ReferenceAccess::Mutable)
    })
}

/// An authored-facing name for a resolved storage place: `self`-rooted
/// storage spells through the receiver, other roots spell their own symbol,
/// and segments spell as field/index/case projections.
pub(super) fn place_spelling(
    program: &dyn ::validation::CheckingInput,
    _machine: &typed_trees::machine::Machine,
    _state: &typed_trees::state::State,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
) -> String {
    let mut spelling = match root {
        facts::PlaceRoot::Symbol(symbol) => program.symbols.name(symbol).to_string(),
        _ => "?".to_string(),
    };
    for segment in path {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                spelling.push('.');
                spelling.push_str(program.symbols.name(*symbol));
            }
            facts::PlaceSegment::FixedIndex { index } => {
                spelling.push_str(&format!("[{index}]"));
            }
            facts::PlaceSegment::Index { .. } => spelling.push_str("[index]"),
            facts::PlaceSegment::OpaqueRange => spelling.push_str("[?..?]"),
            facts::PlaceSegment::FixedRange { start, end } => {
                spelling.push_str(&format!("[{start}..{end}]"));
            }
            facts::PlaceSegment::Case { variant } => {
                spelling.push_str("::");
                spelling.push_str(program.symbols.name(*variant));
            }
        }
    }
    spelling
}
