/*
What a place holds when an operation reads it. A field read produces a fresh
value; this module says when that value is provably the value a write stored,
so the prover sees one more equation between values and never a place.

fact_validity.md: "A primitive read may introduce an equality between its
copied SSA result and an exact reaching stored SSA value. Every actual
predecessor must supply that same value". So the equation needs the writes
that reach the read on every path, which are found from three things the
tables already hold: the forward write log, the one overlap relation over
parent links (`overlap`), and the lowered graph. Walking back from the read,
the first overlapping write met on a path is the one that reaches along it; a
path that meets none before the machine's entry is supplied by the place's
entry content ("a valid initialized ordinary storage place at entry may
supply its exact reaching value version").

The read gets `read == v`, justified by the read operation's own semantics,
exactly when every arrival supplies the one value `v`. Different values on
different arrivals, or contents with no value identity (a call's result, an
overlapping write to another place), give no equation: the read is still
well formed, it just says nothing. control_flow.md: "A fact from one arm
does not survive the join."

Knowledge names values only. No row here states a proposition about a place,
and `semantic_vocabulary::Proposition`'s structural variants are not used.
*/

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, MachineId, PlaceId, Proposition, PsiSemanticId,
    ScalarTerm, StructuralFieldId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, PlaceRow, Segment};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, IdTable, MachineTables, ProgramPoint, RefusalReason,
};
use terminal_psi::{Operation, OperationKind};

use crate::graph::Graph;
use crate::refusal;

fn index(raw: u64) -> usize {
    usize::try_from(raw - 1).unwrap_or(usize::MAX)
}

/// The places from a root down to `place`, root first.
fn lineage(
    machine: MachineId,
    places: &IdTable<ExactPlaceId, PlaceRow>,
    place: ExactPlaceId,
) -> Checked<(PlaceId, Vec<Segment>)> {
    let mut segments = Vec::new();
    let mut at = place;
    loop {
        let row = terminal_check_tables::row(places, at, machine, Construct::Place(at))?;
        match row.origin {
            PlaceOrigin::Root(root) => {
                segments.reverse();
                return Ok((root, segments));
            }
            PlaceOrigin::Projection { parent, segment } => {
                segments.push(segment);
                at = parent;
            }
        }
    }
}

/// Whether two places may share storage: the one overlap relation.
///
/// A place overlaps itself, every ancestor and every descendant. Two places
/// that part ways below a common ancestor are disjoint only when the two
/// segments they part by are known to select different storage, and the one
/// such pair is two different record fields. Every other pair of sibling
/// segments overlaps: two cases of one sum overlay ("Variant payloads
/// overlay, so different case identities do not prove disjoint storage",
/// structural_access.md), and indexed siblings stay overlapping until the
/// lane that reasons about selector values refines them here. Places under
/// different roots are different storage. Later lanes extend this function
/// (runtime indices, referents); they do not add a second relation.
pub(crate) fn overlap(
    machine: MachineId,
    places: &IdTable<ExactPlaceId, PlaceRow>,
    left: ExactPlaceId,
    right: ExactPlaceId,
) -> Checked<bool> {
    if left == right {
        return Ok(true);
    }
    let (left_root, left_path) = lineage(machine, places, left)?;
    let (right_root, right_path) = lineage(machine, places, right)?;
    if left_root != right_root {
        return Ok(false);
    }
    let parting = left_path
        .iter()
        .zip(&right_path)
        .find(|(left, right)| left != right);
    Ok(match parting {
        // One path is a prefix of the other: ancestor and descendant.
        None => true,
        Some((Segment::Field(_), Segment::Field(_))) => false,
        Some((
            Segment::Field(_)
            | Segment::Case(_)
            | Segment::FixedIndex(_)
            | Segment::RuntimeIndex { .. },
            _,
        )) => true,
    })
}

/// The entry content value of each place that has one, indexed by
/// `ExactPlaceId - 1`. A derived column over the value table: elaboration
/// mints at most one such value per place, and a second one is refused
/// because it would be a second identity for one content.
pub(crate) fn entry_contents(
    machine: MachineId,
    tables: &MachineTables,
) -> Checked<Vec<Option<ValueId>>> {
    let mut contents = vec![None; tables.places.len()];
    for (value, row) in tables.values.iter() {
        let ValueOrigin::EntryContent { place } = row.origin else {
            continue;
        };
        let slot = contents.get_mut(index(place.get())).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Value(value),
                "entry content of a place past the end of this machine",
            )
        })?;
        if slot.replace(value).is_some() {
            return Err(refusal(
                RefusalReason::Unsupported,
                machine,
                Construct::Value(value),
                "a place has two entry content values",
            ));
        }
    }
    Ok(contents)
}

/// The child of `parent` by one record field, by binary search of the
/// parent's sorted children. Elaboration interns the place of every read it
/// emits, so an absent child is a table that does not describe the module.
fn child(
    machine: MachineId,
    places: &IdTable<ExactPlaceId, PlaceRow>,
    parent: ExactPlaceId,
    field: StructuralFieldId,
) -> Checked<ExactPlaceId> {
    let row = terminal_check_tables::row(places, parent, machine, Construct::Place(parent))?;
    let segment = Segment::Field(field);
    row.children
        .binary_search_by_key(&segment, |(segment, _)| *segment)
        .map(|position| row.children[position].1)
        .map_err(|_| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Place(parent),
                "a read names a projection the place table does not hold",
            )
        })
}

/// The exact place a field read observes, or `None` for any other
/// operation.
fn read_place(
    machine: MachineId,
    tables: &MachineTables,
    operation: &Operation,
) -> Checked<Option<ExactPlaceId>> {
    let (source, path, field) = match &operation.kind {
        OperationKind::ScalarStructuralField {
            source,
            path,
            field,
        }
        | OperationKind::BooleanStructuralField {
            source,
            path,
            field,
        } => (*source, path, *field),
        _ => return Ok(None),
    };
    let concerns = Construct::Operation(operation.id);
    let mut place = *terminal_check_tables::row(&tables.roots, source, machine, concerns)?;
    for segment in path {
        place = match segment {
            CanonicalStructuralPathSegment::Field(field) => {
                child(machine, &tables.places, place, *field)?
            }
            CanonicalStructuralPathSegment::FixedIndex(_)
            | CanonicalStructuralPathSegment::Case(_)
            | CanonicalStructuralPathSegment::Referent => {
                return Err(refusal(
                    RefusalReason::Unsupported,
                    machine,
                    concerns,
                    "read through an index, a case or a referent",
                ));
            }
        };
    }
    child(machine, &tables.places, place, field).map(Some)
}

/// One write that overlaps the read place, located in the graph.
struct Candidate {
    block: BlockId,
    /// Its position in the block, as `Graph::locate` orders points.
    position: usize,
    /// The value it stores into exactly the read place, if that is what it
    /// does. Any other overlapping write leaves the place with contents the
    /// read cannot name.
    value: Option<ValueId>,
}

/// The value `place` holds on every path to the point before `read`, when
/// it is one and the same value on all of them.
fn reaching_value(
    machine: MachineId,
    tables: &MachineTables,
    graph: &Graph,
    entry_contents: &[Option<ValueId>],
    place: ExactPlaceId,
    read: &Operation,
) -> Checked<Option<ValueId>> {
    let concerns = Construct::Operation(read.id);
    let mut candidates = Vec::new();
    for (_, write) in tables.writes.iter() {
        if !overlap(machine, &tables.places, place, write.place)? {
            continue;
        }
        let (block, position) = match write.at {
            ProgramPoint::BlockEntry(_) | ProgramPoint::AfterOperation(_) => {
                graph.locate(write.at)?
            }
            // A write on one edge reaches only that edge's destination; this
            // walk moves between blocks, not edges, and elaboration logs no
            // edge write yet. Refused rather than given to every successor.
            ProgramPoint::Edge(_) => {
                return Err(refusal(
                    RefusalReason::Unsupported,
                    machine,
                    concerns,
                    "a write on an edge overlaps a read",
                ));
            }
        };
        let value = match write.content {
            Content::Value(value) if write.place == place => Some(value),
            Content::Value(_) | Content::Reference(_) | Content::Overwritten | Content::Vacated => {
                None
            }
        };
        candidates.push(Candidate {
            block,
            position,
            value,
        });
    }
    // The point before the read: everything at a position up to the read's
    // own index has happened, the read's own result has not.
    let (start, after) = graph.locate(ProgramPoint::AfterOperation(read.id))?;
    let before = after.checked_sub(1).ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine,
            concerns,
            "a read has no point before it",
        )
    })?;
    let mut single: Option<ValueId> = None;
    let mut differs = false;
    let mut supply = |value: Option<ValueId>| match (value, single) {
        (Some(value), None) => single = Some(value),
        (Some(value), Some(known)) if value == known => {}
        (Some(_), Some(_)) | (None, _) => differs = true,
    };
    // Each block is walked whole at most once, so a cycle ends the walk.
    let mut walked = vec![false; graph.block_count()];
    let mut pending = vec![(start, before)];
    while let Some((block, up_to)) = pending.pop() {
        // The last overlapping write in this block at or before the point:
        // the latest position, and in one position the later log entry.
        let last = candidates
            .iter()
            .filter(|candidate| candidate.block == block && candidate.position <= up_to)
            .fold(None::<&Candidate>, |best, candidate| match best {
                Some(best) if best.position > candidate.position => Some(best),
                Some(_) | None => Some(candidate),
            });
        if let Some(write) = last {
            supply(write.value);
            continue;
        }
        if block == graph.entry() {
            let content = entry_contents
                .get(index(place.get()))
                .copied()
                .flatten()
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::Unsupported,
                        machine,
                        concerns,
                        "a read reaches the entry with no write and no entry content",
                    )
                })?;
            supply(Some(content));
        }
        for predecessor in graph.predecessors(block)? {
            let slot = walked.get_mut(index(predecessor.get())).ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    machine,
                    Construct::Block(*predecessor),
                    "an edge leaves a block past the end",
                )
            })?;
            if !std::mem::replace(slot, true) {
                pending.push((*predecessor, usize::MAX));
            }
        }
    }
    Ok(if differs { None } else { single })
}

/// The equation a field read establishes on its normal successor:
/// `read == v` when `v` is the one value its place holds on every path to
/// it, else none.
pub(crate) fn read_equation(
    machine: MachineId,
    tables: &MachineTables,
    graph: &Graph,
    entry_contents: &[Option<ValueId>],
    operation: &Operation,
) -> Checked<Option<Proposition>> {
    let Some(place) = read_place(machine, tables, operation)? else {
        return Ok(None);
    };
    let concerns = Construct::Operation(operation.id);
    let result = operation.result.scalar_ref().ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine,
            concerns,
            "a field read has no scalar result",
        )
    })?;
    let Some(held) = reaching_value(machine, tables, graph, entry_contents, place, operation)?
    else {
        return Ok(None);
    };
    let held_type =
        terminal_check_tables::row(&tables.values, held, machine, concerns)?.scalar_type;
    if held_type != result.scalar_type {
        return Err(refusal(
            RefusalReason::Unsupported,
            machine,
            concerns,
            "a place holds a value of another type than its read",
        ));
    }
    Ok(Some(Proposition::Equal(
        ScalarTerm::value(result.id, result.scalar_type),
        ScalarTerm::value(held, held_type),
    )))
}
