//! Places one evaluation holds together: the operands of a call, or the
//! reference leaves of one aggregate value formed at once. Every pair must be
//! non-interfering under `captured_place_compatibility`: an exclusive or
//! write-only hold conflicts with any overlapping hold, and two shared holds
//! conflict only when the checker cannot establish non-interference. Calls and
//! aggregate formation share this judgment and its wording; each diagnostic
//! supplies its own subject and verb.

use checked_trees::{BorrowAccessKind, CapturedPlace};

use super::overlap::{StatedOrderingPremise, captured_place_compatibility};

/// One of the two holds a conflict compares.
#[derive(Clone, Copy)]
pub(super) enum Hold {
    First,
    Second,
}

/// Why two holds formed together interfere. Each variant names the hold whose
/// place the diagnostic reports.
pub(super) enum SimultaneousHoldConflict {
    RepeatedMutable,
    MutableAndRead { mutable: Hold },
    WriteOnly { write_only: Hold },
    DependentReads,
}

pub(super) fn simultaneous_hold_conflict<'p>(
    program: &'p typed_trees::TypedTrees,
    first: &CapturedPlace,
    first_kind: &BorrowAccessKind,
    second: &CapturedPlace,
    second_kind: &BorrowAccessKind,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> Option<SimultaneousHoldConflict> {
    if captured_place_compatibility(
        program,
        first,
        first_kind,
        second,
        second_kind,
        premises,
        bound_lookup,
    )
    .non_interfering
    {
        return None;
    }
    Some(match (first_kind, second_kind) {
        (BorrowAccessKind::Mutable, BorrowAccessKind::Mutable) => {
            SimultaneousHoldConflict::RepeatedMutable
        }
        (BorrowAccessKind::Mutable, BorrowAccessKind::Read) => {
            SimultaneousHoldConflict::MutableAndRead {
                mutable: Hold::First,
            }
        }
        (BorrowAccessKind::Read, BorrowAccessKind::Mutable) => {
            SimultaneousHoldConflict::MutableAndRead {
                mutable: Hold::Second,
            }
        }
        (BorrowAccessKind::WriteOnly, _) => SimultaneousHoldConflict::WriteOnly {
            write_only: Hold::First,
        },
        (_, BorrowAccessKind::WriteOnly) => SimultaneousHoldConflict::WriteOnly {
            write_only: Hold::Second,
        },
        (BorrowAccessKind::Read, BorrowAccessKind::Read) => {
            SimultaneousHoldConflict::DependentReads
        }
    })
}

impl SimultaneousHoldConflict {
    /// The held-place phrase that follows a subject and verb such as
    /// "state `touch` receives" or "`messages` holds". `label` renders a
    /// hold's place; `sibling` names the other hold of a write-only overlap.
    pub(super) fn phrase(&self, label: impl Fn(Hold) -> String, sibling: &str) -> String {
        match self {
            Self::RepeatedMutable => {
                format!("`{}` as mutable more than once", label(Hold::First))
            }
            Self::MutableAndRead { mutable } => {
                format!("`{}` as both mutable and read-only", label(*mutable))
            }
            Self::WriteOnly { write_only } => {
                format!("write-only `{}` overlapping {sibling}", label(*write_only))
            }
            Self::DependentReads => {
                "dependent read-only places that do not establish non-interference".to_owned()
            }
        }
    }
}
