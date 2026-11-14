/*
Places and the forward write log. A place is one dense identity whose row
names its origin (a root, or a parent plus one typed segment), so "this place"
has exactly one representation. Elaboration mints at most one row per root and
per (parent, segment) pair, so two occurrences of the same path are the same
identity: equal identities are equal places.

The converse does not hold. Different identities are not disjoint places: a
root overlaps every projection beneath it, and `FixedIndex(3)` and
`RuntimeIndex { selector }` with a selector known to equal 3 are two rows for
one storage cell. Overlap is a question the checker answers from parent links
and knowledge about selector values. Nothing may treat unequal identities as
disjoint.

The write log records, in program order, what each place holds after each
write. The binding at a point is the last reaching write to the place or to
any place that overlaps it, which the checker derives from the log.

Three lookups are derived columns, filled by elaboration as it pushes rows,
so no lane needs a map: a root by `PlaceId` (`MachineTables::roots`), a
child by segment (`PlaceRow::children`), and the writes to a place
(`PlaceRow::writes`).
*/

use semantic_vocabulary::{
    IdTable, MachineId, PlaceId, StructuralCaseId, StructuralFieldId, ValueId,
};

use crate::{ExactPlaceId, ProgramPoint, WriteId};

/// One exact place. `AGENTS.md` framing rule 3: "Track facts, loans, and
/// restoration obligations at the exact projected place (`root.field` or case
/// variant payload)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceRow {
    pub origin: PlaceOrigin,
    /// The projections of this place, sorted by `Segment`, at most one per
    /// segment: interning a child is a binary search here.
    pub children: Vec<(Segment, ExactPlaceId)>,
    /// The writes whose `place` is exactly this place, in program order.
    /// Writes to ancestors and descendants are on their own rows; reach them
    /// through `PlaceOrigin::Projection::parent` and `children`.
    pub writes: Vec<WriteId>,
}

/// Invariant: a projection's `parent` was minted before it, so every parent
/// chain ends at a root and contains no cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceOrigin {
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
///
/// Segments identify rows, not storage: two different segments under one
/// parent may still select the same cell (see the module comment), so
/// disjointness is never read off unequal segments.
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
    /// The descriptor's current extent, a scalar observation of its
    /// contents. This is metadata, not a physical field or another owner.
    /// byte_views.md: "a validated length observation of a machine or block
    /// parameter equals that exact parameter's `ViewExtent`".
    /// Replacing any ancestor descriptor replaces this observation's
    /// reaching content, just as a store replaces a scalar field's.
    Extent,
    /// Cross a live permission descriptor into its separately owned backing.
    /// It is never an inline field, a claim transfer or a disjointness fact.
    Referent,
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
    /// Exactly this scalar value.
    Value(ValueId),
    /// New contents with no known value identity, for the place and
    /// everything beneath it: an unknown or conservative write frame.
    /// dependent_values.md: "An unrepresentable origin or incomplete
    /// recursive summary uses the conservative ownership ceiling, never an
    /// empty write set."
    Overwritten,
    /// No valid contents: an owned move transferred this region's value.
    /// `wiki/spec/language/ownership.md`: "Zero-filled or stale bytes do not
    /// fill the hole." A borrowed-storage extraction additionally carries
    /// restoration debt (`obligations::ObligationKind::RestorationWindow`);
    /// an ordinary owned transfer ends its source value.
    Vacated,
}

/// Calls can reveal a more precise written field after every body is lowered.
/// Use the same interning owner as the original storage producer, so adding
/// that projection cannot give one place two identities.
pub fn intern_projection(
    machine: MachineId,
    places: &mut IdTable<ExactPlaceId, PlaceRow>,
    parent: ExactPlaceId,
    segment: Segment,
) -> crate::Checked<ExactPlaceId> {
    let missing = || crate::Refusal {
        reason: crate::RefusalReason::ForeignIdentity,
        machine,
        concerns: crate::Construct::Place(parent),
        detail: "place is past the end of this machine".to_owned(),
    };
    let row = places.get(parent).ok_or_else(missing)?;
    match row
        .children
        .binary_search_by_key(&segment, |(segment, _)| *segment)
    {
        Ok(position) => Ok(row.children[position].1),
        Err(position) => {
            let child = places.push(PlaceRow {
                origin: PlaceOrigin::Projection { parent, segment },
                children: Vec::new(),
                writes: Vec::new(),
            });
            places
                .get_mut(parent)
                .ok_or_else(missing)?
                .children
                .insert(position, (segment, child));
            Ok(child)
        }
    }
}

pub fn record_write(
    machine: MachineId,
    places: &mut IdTable<ExactPlaceId, PlaceRow>,
    writes: &mut IdTable<WriteId, WriteRow>,
    row: WriteRow,
) -> crate::Checked<()> {
    let place = places.get_mut(row.place).ok_or_else(|| crate::Refusal {
        reason: crate::RefusalReason::ForeignIdentity,
        machine,
        concerns: crate::Construct::Place(row.place),
        detail: "place is past the end of this machine".to_owned(),
    })?;
    place.writes.push(writes.push(row));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IdTable, PsiSemanticId};

    #[test]
    fn children_are_interned_once_per_segment_by_binary_search() {
        let field = |raw| Segment::Field(StructuralFieldId::new(raw).unwrap());
        let mut places: IdTable<ExactPlaceId, PlaceRow> = IdTable::default();
        let root = places.push(PlaceRow {
            origin: PlaceOrigin::Root(PlaceId::new(1).unwrap()),
            children: Vec::new(),
            writes: Vec::new(),
        });
        for raw in [3, 1, 2] {
            let child = places.push(PlaceRow {
                origin: PlaceOrigin::Projection {
                    parent: root,
                    segment: field(raw),
                },
                children: Vec::new(),
                writes: Vec::new(),
            });
            let row = places.get_mut(root).unwrap();
            let at = row
                .children
                .binary_search_by_key(&field(raw), |(segment, _)| *segment)
                .unwrap_err();
            row.children.insert(at, (field(raw), child));
        }
        let children = &places.get(root).unwrap().children;
        let found = children.binary_search_by_key(&field(2), |(segment, _)| *segment);
        assert_eq!(children[found.unwrap()].1.get(), 4);
        assert!(children.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
}
