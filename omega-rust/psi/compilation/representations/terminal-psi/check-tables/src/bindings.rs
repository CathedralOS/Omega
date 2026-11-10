/*
Places and the forward write log. A place is one dense identity whose row
names its parent and one typed segment, so "this place" has exactly one
representation. Elaboration mints at most one row per root and per
(parent, segment) pair, so two occurrences of the same path are the same
identity and equal identities are equal places. The write log records, in
program order, which value each place holds after each write; the binding at
a point is the last reaching write, which the checker derives from the log.
*/

use semantic_vocabulary::{PlaceId, StructuralCaseId, StructuralFieldId, ValueId};

use crate::{ExactPlaceId, ProgramPoint};

/// One exact place. `AGENTS.md` framing rule 3: "Track facts, loans, and
/// restoration obligations at the exact projected place (`root.field` or case
/// variant payload)".
///
/// Invariant: a projection's `parent` was minted before it, so the parent
/// chain of every place ends at a root and contains no cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceRow {
    /// A root storage place of the lowered form: a parameter, named local,
    /// call result or block parameter. `wiki/spec/terminal-psi/ownership.md`:
    /// "Parameter, named-local, and call-result roots retain distinct
    /// establishment identities."
    Root(PlaceId),
    /// The place reached from `parent` by one segment.
    Projection {
        parent: ExactPlaceId,
        segment: Segment,
    },
}

/// One step of a projected place.
///
/// The lowered form's `StructuralPathSegment::Field` holds an interned text
/// index; this crate names the field by `StructuralFieldId` instead, resolved
/// once at elaboration, so no segment is compared by spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Segment {
    /// ownership.md: "Record segments use the exact field identity".
    Field(StructuralFieldId),
    /// ownership.md: "A Case segment guards its payload namespace; it adds
    /// neither physical storage nor an independent claim."
    Case(StructuralCaseId),
    /// ownership.md: "Fixed-array segments use canonical zero-based literal
    /// indexes".
    FixedIndex(u64),
    /// An element selected at run time, named by the selector value captured
    /// when the place was formed. `wiki/spec/terminal-psi/loans.md`: "Loan
    /// formation captures a place, not a live selector expression." Because a
    /// value never changes, a later write to the selector's source names a
    /// different value and therefore a different place.
    RuntimeIndex { selector: ValueId },
}

/// One write, in program order: within a block, `WriteId`s increase in
/// operation order.
///
/// `wiki/spec/language/dependent_values.md`: "Invalidate flow refinements atom
/// by atom on overlapping written places". Knowledge about the old value stays
/// true; the write changes which value `place` and every overlapping place
/// holds from `at` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteRow {
    pub place: ExactPlaceId,
    pub at: ProgramPoint,
    pub content: Content,
}

/// What a place holds after a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    /// Exactly this value.
    Value(ValueId),
    /// New contents with no known value identity, for the place and
    /// everything beneath it: an unknown or conservative write frame.
    /// dependent_values.md: "An unrepresentable origin or incomplete
    /// recursive summary uses the conservative ownership ceiling, never an
    /// empty write set."
    Overwritten,
    /// No valid contents: an owned move out of borrowed storage left a hole.
    /// `wiki/spec/language/ownership.md`: "Zero-filled or stale bytes do not
    /// fill the hole." The matching restoration debt is an obligation
    /// (`obligations::ObligationKind::RestorationWindow`).
    Vacated,
}
