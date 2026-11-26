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

Elaboration indexes roots by `PlaceId` (`MachineTables::roots`) and children
by their projection segment (`PlaceRow::children`). The forward log owns each
write in program order.
*/

use arena::IdTable;
use semantic_vocabulary::{MachineId, PlaceId, ValueId};

use crate::{ExactPlaceId, ProgramPoint, WriteId};

/// One exact place. `AGENTS.md` framing rule 3: "Track facts, loans, and
/// restoration obligations at the exact projected place (`root.field` or case
/// variant payload)".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlaceRow {
    pub origin: PlaceOrigin,
    /// Child identities, ordered by the segment in each child's origin.
    pub children: Vec<ExactPlaceId>,
}

impl PlaceRow {
    fn child_position(
        &self,
        places: &IdTable<ExactPlaceId, PlaceRow>,
        segment: Segment,
    ) -> Result<usize, usize> {
        self.children.binary_search_by_key(&Some(segment), |child| {
            match places.get(*child)?.origin {
                PlaceOrigin::Projection { segment, .. } => Some(segment),
                PlaceOrigin::Root(_) => None,
            }
        })
    }

    pub fn child(
        &self,
        places: &IdTable<ExactPlaceId, PlaceRow>,
        segment: Segment,
    ) -> Option<ExactPlaceId> {
        self.child_position(places, segment)
            .ok()
            .map(|position| self.children[position])
    }
}

/// Invariant: a projection's `parent` was minted before it, so every parent
/// chain ends at a root and contains no cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

// Storage analysis and checking must retain the same resolved field IDs,
// captured selectors and representation views. The representation owns them.
pub use terminal_psi::ResolvedStructuralPathSegment as Segment;

/// A dynamic projection can name entry contents only when its selector is
/// already bound at the same entry as its structural root.
pub fn entry_snapshot_segment(
    segment: Segment,
    owner: semantic_vocabulary::StructuralPlaceKind,
    values: &IdTable<ValueId, crate::values::ValueRow>,
) -> bool {
    use crate::values::ValueOrigin;
    use semantic_vocabulary::StructuralPlaceKind;
    if let Segment::RuntimeIndex { selector } = segment {
        return values.get(selector).is_some_and(|value| {
            value.scalar_type.as_integer().is_some()
                && match (value.origin, owner) {
                    (ValueOrigin::MachineParameter { .. }, _) => true,
                    (
                        ValueOrigin::BlockParameter { block, .. },
                        StructuralPlaceKind::BlockParameter {
                            block: root_block, ..
                        },
                    ) => block == root_block,
                    _ => false,
                }
        });
    }
    matches!(owner, StructuralPlaceKind::Parameter { .. })
        || matches!(
            segment,
            Segment::Field(_)
                | Segment::Case(_)
                | Segment::FixedIndex(_)
                | Segment::Referent
                | Segment::Extent
        )
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
/// that projection cannot give one place two identities. Lookups preserve the
/// shared geometry until an entry needs to insert a new projection.
pub fn intern_projection(
    machine: MachineId,
    places: &mut super::CheckingRows<ExactPlaceId, PlaceRow>,
    parent: ExactPlaceId,
    segment: Segment,
) -> crate::Checked<ExactPlaceId> {
    let missing = || crate::Refusal {
        reason: crate::RefusalReason::ForeignIdentity,
        site: crate::RefusalSite::Machine(machine, crate::Construct::Place(parent)),
        source_span: None,
        detail: "place is past the end of this machine".to_owned(),
    };
    let row = places.get(parent).ok_or_else(missing)?;
    match row.child_position(places, segment) {
        Ok(position) => Ok(row.children[position]),
        Err(position) => {
            let child = places.push(PlaceRow {
                origin: PlaceOrigin::Projection { parent, segment },
                children: Vec::new(),
            });
            places
                .get_mut(parent)
                .ok_or_else(missing)?
                .children
                .insert(position, child);
            Ok(child)
        }
    }
}

pub fn record_write(
    machine: MachineId,
    places: &IdTable<ExactPlaceId, PlaceRow>,
    writes: &mut IdTable<WriteId, WriteRow>,
    row: WriteRow,
) -> crate::Checked<()> {
    places.get(row.place).ok_or_else(|| crate::Refusal {
        reason: crate::RefusalReason::ForeignIdentity,
        site: crate::RefusalSite::Machine(machine, crate::Construct::Place(row.place)),
        source_span: None,
        detail: "place is past the end of this machine".to_owned(),
    })?;
    writes.push(row);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::StructuralFieldId;

    #[test]
    fn children_are_interned_once_per_segment_by_binary_search() {
        let field = |raw| Segment::Field(StructuralFieldId::new(raw).unwrap());
        let mut places = crate::CheckingRows::default();
        let root = places.push(PlaceRow {
            origin: PlaceOrigin::Root(PlaceId::new(1).unwrap()),
            children: Vec::new(),
        });
        let machine = MachineId::new(1).unwrap();
        let children =
            [3, 1, 2].map(|raw| intern_projection(machine, &mut places, root, field(raw)).unwrap());
        let row = places.get(root).unwrap();
        assert_eq!(row.children, [children[1], children[2], children[0]]);
        assert_eq!(row.child(&places, field(2)), Some(children[2]));
        assert_eq!(row.child(&places, field(4)), None);
        assert_eq!(
            intern_projection(machine, &mut places, root, field(2)).unwrap(),
            children[2]
        );
        assert_eq!(places.len(), 4);
    }
}
