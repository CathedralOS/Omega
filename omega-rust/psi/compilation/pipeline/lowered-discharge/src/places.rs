/*
What a place holds when an operation uses it, answered once for reads, copies
and the initialization judgment by one walk over three things the tables and
the lowered form already hold: the forward write log, the one overlap
relation over parent links (`overlap`), and the lowered graph.

The walk (`Storage::reaching`). From the point before a use of one scalar
leaf, go back over the graph. The last overlapping write met on a path is the
one that reaches along it. Three things can supply a path:

- a row `Content::Value(v)` on exactly the leaf supplies `v`;
- a row `Content::Overwritten` on the leaf or a place above it supplies new
  contents with no value identity, unless the operation at the row's point is
  one that moves storage whole (a whole copy, a record operand of a literal,
  a record stored into a field). Then the leaf holds what the matching leaf
  of the operation's source held just before it, and the walk continues from
  there. The row cannot say this itself: `Content` has no variant for "what
  that other place held", so the source is read from the operation, whose
  storage path the lowered form resolves (`runtime_structural_field`);
- a path that meets no write before the machine's entry is supplied by the
  place's entry content when its root is a formal ("a valid initialized
  ordinary storage place at entry may supply its exact reaching value
  version", fact_validity.md), and by nothing otherwise.

Two judgments read the answer.

The read equation. fact_validity.md: "A primitive read may introduce an
equality between its copied SSA result and an exact reaching stored SSA
value. Every actual predecessor must supply that same value". A read gets
`read == v`, justified by the read operation's own semantics, exactly when
every path supplies the one value `v`. Different values on different paths,
or contents with no value identity, give no equation: the read is still well
formed, it just says nothing. control_flow.md: "A fact from one arm does not
survive the join."

Initialization (`Storage::uses`). structural_access.md: "An unestablished
local cannot be read: a whole read of the home, a loan of it to an ordinary
or boundary call, and its forwarding on a state edge each require a whole
store into it on every path reaching that use." Every operation and
terminator that reads storage (a field read, a whole copy, a record operand,
a call argument, a return) gets one verdict: each scalar leaf at or beneath
the place it uses must be supplied on every path. A path that supplies
nothing is a refusal with reason `Unestablished`. This is a structural
judgment, not a proposition: it has no obligation row and no certificate.

Knowledge names values only. No row here states a proposition about a place,
and `semantic_vocabulary::Proposition`'s structural variants are not used.
*/

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, MachineId, OperationId, PlaceId, Proposition,
    PsiSemanticId, ScalarTerm, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, PlaceRow, Segment};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, IdTable, MachineTables, ProgramPoint, RefusalReason,
};
use terminal_psi::{
    Operation, OperationKind, RecordFieldValue, StructuralArgument, StructuralFieldType,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, Terminator,
};

use crate::graph::Graph;
use crate::{StorageUse, refusal};

fn index(raw: u64) -> usize {
    usize::try_from(raw - 1).unwrap_or(usize::MAX)
}

/// A place as its root and the segments from the root down to it. A place
/// the table has not interned (a leaf beneath a place used whole) has a
/// lineage too, which is why the walk works on lineages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lineage {
    root: PlaceId,
    segments: Vec<Segment>,
}

pub(crate) fn lineage(
    machine: MachineId,
    places: &IdTable<ExactPlaceId, PlaceRow>,
    place: ExactPlaceId,
) -> Checked<Lineage> {
    let mut segments = Vec::new();
    let mut at = place;
    loop {
        let row = terminal_check_tables::row(places, at, machine, Construct::Place(at))?;
        match row.origin {
            PlaceOrigin::Root(root) => {
                segments.reverse();
                return Ok(Lineage { root, segments });
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
///
/// It is stated over lineages, not table rows, because the walk also asks it
/// about leaves the place table has not interned.
pub(crate) fn overlap(left: &Lineage, right: &Lineage) -> bool {
    if left.root != right.root {
        return false;
    }
    let parting = left
        .segments
        .iter()
        .zip(&right.segments)
        .find(|(left, right)| left != right);
    match parting {
        // One path is a prefix of the other: the same place, or ancestor and
        // descendant.
        None => true,
        Some((Segment::Field(_), Segment::Field(_))) => false,
        Some((
            Segment::Field(_)
            | Segment::Case(_)
            | Segment::FixedIndex(_)
            | Segment::RuntimeIndex { .. },
            _,
        )) => true,
    }
}

/// The entry content value of each place that has one, indexed by
/// `ExactPlaceId - 1`. A derived column over the value table: elaboration
/// mints at most one such value per place, and a second one is refused
/// because it would be a second identity for one content.
fn entry_contents(machine: MachineId, tables: &MachineTables) -> Checked<Vec<Option<ValueId>>> {
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

/// One row of the write log, located in the graph.
struct Logged {
    lineage: Lineage,
    block: BlockId,
    /// Its position in the block, as `Graph::locate` orders points.
    position: usize,
    content: Content,
    /// The operation whose normal continuation the row is stated at.
    operation: Option<OperationId>,
    /// A row on an edge reaches only that edge's destination; this walk
    /// moves between blocks, not edges, and elaboration logs no edge write
    /// yet. One that overlaps a use is refused rather than given to every
    /// successor.
    on_edge: bool,
}

/// What one scalar leaf holds on every path to a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reached {
    /// The one value every path supplies.
    Value(ValueId),
    /// Every path supplies contents, but not one same named value.
    Unknown,
    /// Some path supplies nothing: no store reaches the use along it and the
    /// place held nothing at entry.
    Unestablished,
}

/// What a place is, by the lowered form's own declarations.
enum Shape {
    Scalar,
    Typed(StructuralTypeId),
}

/// One machine's storage as the checker reads it: the write log located in
/// the graph, the entry contents, and the lowered declarations that say
/// what lies beneath a place. Derived once per machine; the tables do not
/// change beneath it, because discharge only appends knowledge.
pub(crate) struct Storage<'a> {
    machine: MachineId,
    lowered: &'a TerminalMachine,
    types: &'a [StructuralTypeDeclaration],
    graph: &'a Graph,
    entry_contents: Vec<Option<ValueId>>,
    log: Vec<Logged>,
}

impl<'a> Storage<'a> {
    pub(crate) fn new(
        machine: MachineId,
        lowered: &'a TerminalMachine,
        types: &'a [StructuralTypeDeclaration],
        graph: &'a Graph,
        tables: &MachineTables,
    ) -> Checked<Self> {
        let mut log = Vec::with_capacity(tables.writes.len());
        for (_, write) in tables.writes.iter() {
            let (block, position) = graph.locate(write.at)?;
            let (operation, on_edge) = match write.at {
                ProgramPoint::AfterOperation(operation) => (Some(operation), false),
                ProgramPoint::BlockEntry(_) => (None, false),
                ProgramPoint::Edge(_) => (None, true),
            };
            log.push(Logged {
                lineage: lineage(machine, &tables.places, write.place)?,
                block,
                position,
                content: write.content,
                operation,
                on_edge,
            });
        }
        Ok(Self {
            machine,
            lowered,
            types,
            graph,
            entry_contents: entry_contents(machine, tables)?,
            log,
        })
    }

    fn unsupported(&self, concerns: Construct, detail: &str) -> terminal_check_tables::Refusal {
        refusal(RefusalReason::Unsupported, self.machine, concerns, detail)
    }

    fn foreign(&self, concerns: Construct, detail: &str) -> terminal_check_tables::Refusal {
        refusal(
            RefusalReason::ForeignIdentity,
            self.machine,
            concerns,
            detail,
        )
    }

    fn declaration(
        &self,
        id: StructuralTypeId,
        concerns: Construct,
    ) -> Checked<&'a StructuralTypeDeclaration> {
        self.types
            .get(index(id.get()))
            .filter(|declaration| declaration.id == id)
            .ok_or_else(|| self.foreign(concerns, "a structural type is not at its position"))
    }

    fn root_kind(&self, root: PlaceId, concerns: Construct) -> Checked<StructuralPlaceKind> {
        self.lowered
            .structural_places
            .get(index(root.get()))
            .filter(|declaration| declaration.id == root)
            .map(|declaration| declaration.kind)
            .ok_or_else(|| self.foreign(concerns, "a root place is not declared at its position"))
    }

    fn root_type(&self, root: PlaceId, concerns: Construct) -> Checked<StructuralTypeId> {
        match self.root_kind(root, concerns)? {
            StructuralPlaceKind::OperationResult {
                structural_type, ..
            } => Ok(structural_type),
            StructuralPlaceKind::Parameter { position } => self
                .lowered
                .structural_parameters
                .iter()
                .find(|parameter| parameter.position == position && parameter.place == root)
                .map(|parameter| parameter.structural_type)
                .ok_or_else(|| self.foreign(concerns, "a formal's place has no declaration")),
            StructuralPlaceKind::Result => match &self.lowered.result {
                TerminalMachineResult::Structural(result) if result.place == root => {
                    Ok(result.structural_type)
                }
                _ => Err(self.foreign(concerns, "a result place on a machine with none")),
            },
            StructuralPlaceKind::BlockParameter { .. }
            | StructuralPlaceKind::ByteSequenceLiteral { .. }
            | StructuralPlaceKind::ProviderAttachment { .. }
            | StructuralPlaceKind::TrivialAffineLocal { .. } => {
                Err(self.unsupported(concerns, "a root place kind outside this route"))
            }
        }
    }

    /// What the place at `lineage` is: resolved segment by segment against
    /// the type the previous segment reached.
    fn shape(&self, place: &Lineage, concerns: Construct) -> Checked<Shape> {
        let mut at = Shape::Typed(self.root_type(place.root, concerns)?);
        for segment in &place.segments {
            let Shape::Typed(owner) = at else {
                return Err(self.foreign(concerns, "a place lies beneath a scalar field"));
            };
            let StructuralTypeShape::Record { fields, .. } =
                &self.declaration(owner, concerns)?.shape
            else {
                return Err(self.unsupported(concerns, "a projection of storage that is no record"));
            };
            let Segment::Field(field) = segment else {
                return Err(self.unsupported(concerns, "a place through an index or a case"));
            };
            let declared = fields
                .iter()
                .find(|declared| declared.id == *field)
                .ok_or_else(|| {
                    self.foreign(concerns, "a field is not of the record it projects")
                })?;
            at = match declared.field_type {
                StructuralFieldType::Scalar(_) => Shape::Scalar,
                StructuralFieldType::Structural(nested) => Shape::Typed(nested),
                _ => return Err(self.unsupported(concerns, "a field type outside plain records")),
            };
        }
        Ok(at)
    }

    /// Every scalar leaf at or beneath `place`, as lineages.
    fn leaves(&self, place: &Lineage, concerns: Construct) -> Checked<Vec<Lineage>> {
        let mut found = Vec::new();
        match self.shape(place, concerns)? {
            Shape::Scalar => found.push(place.clone()),
            Shape::Typed(structural_type) => {
                let mut at = place.clone();
                self.leaves_of(structural_type, &mut at, &mut found, concerns)?;
            }
        }
        Ok(found)
    }

    fn leaves_of(
        &self,
        structural_type: StructuralTypeId,
        at: &mut Lineage,
        found: &mut Vec<Lineage>,
        concerns: Construct,
    ) -> Checked<()> {
        match &self.declaration(structural_type, concerns)?.shape {
            StructuralTypeShape::PrimitiveScalar(_) => found.push(at.clone()),
            StructuralTypeShape::Record { fields, .. } => {
                for field in fields {
                    at.segments.push(Segment::Field(field.id));
                    match field.field_type {
                        StructuralFieldType::Scalar(_) => found.push(at.clone()),
                        StructuralFieldType::Structural(nested) => {
                            self.leaves_of(nested, at, found, concerns)?;
                        }
                        _ => {
                            return Err(
                                self.unsupported(concerns, "a field type outside plain records")
                            );
                        }
                    }
                    at.segments.pop();
                }
            }
            _ => return Err(self.unsupported(concerns, "storage that is no plain record")),
        }
        Ok(())
    }

    /// The place a Terminal Psi storage path names beneath a root. Each
    /// field segment is resolved by the lowered form itself against the type
    /// the previous segment reached (`runtime_structural_field`); this
    /// module compares no identity of its own.
    fn stored_at(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        concerns: Construct,
    ) -> Checked<Lineage> {
        let mut segments = Vec::with_capacity(path.len());
        if !path.is_empty() {
            let root_type = self.root_type(root, concerns)?;
            for length in 1..=path.len() {
                let field = terminal_semantics::runtime_structural_field(
                    self.types.iter(),
                    root_type,
                    &path[..length],
                )
                .ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "a storage path that is not a path of record fields",
                    )
                })?;
                segments.push(Segment::Field(field.id));
            }
        }
        Ok(Lineage { root, segments })
    }

    fn argument(&self, argument: &StructuralArgument, concerns: Construct) -> Checked<Lineage> {
        self.stored_at(argument.place, &argument.path, concerns)
    }

    /// The scalar leaf a read operation observes, or `None` for any other
    /// operation.
    fn read_leaf(&self, operation: &Operation) -> Checked<Option<Lineage>> {
        let concerns = Construct::Operation(operation.id);
        match &operation.kind {
            OperationKind::ScalarStructuralField {
                source,
                path,
                field,
            }
            | OperationKind::BooleanStructuralField {
                source,
                path,
                field,
            } => {
                let mut segments = Vec::with_capacity(path.len() + 1);
                for segment in path {
                    match segment {
                        CanonicalStructuralPathSegment::Field(field) => {
                            segments.push(Segment::Field(*field));
                        }
                        CanonicalStructuralPathSegment::FixedIndex(_)
                        | CanonicalStructuralPathSegment::Case(_)
                        | CanonicalStructuralPathSegment::Referent => {
                            return Err(self.unsupported(
                                concerns,
                                "read through an index, a case or a referent",
                            ));
                        }
                    }
                }
                segments.push(Segment::Field(*field));
                Ok(Some(Lineage {
                    root: *source,
                    segments,
                }))
            }
            OperationKind::PrimitiveScalarRead { source, path } => {
                self.stored_at(*source, path, concerns).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// The interned row of a lineage, when the place table holds one.
    fn interned(
        &self,
        tables: &MachineTables,
        place: &Lineage,
        concerns: Construct,
    ) -> Checked<Option<ExactPlaceId>> {
        let mut at =
            *terminal_check_tables::row(&tables.roots, place.root, self.machine, concerns)?;
        for segment in &place.segments {
            let row = terminal_check_tables::row(&tables.places, at, self.machine, concerns)?;
            match row
                .children
                .binary_search_by_key(segment, |(segment, _)| *segment)
            {
                Ok(position) => at = row.children[position].1,
                Err(_) => return Ok(None),
            }
        }
        Ok(Some(at))
    }

    fn operation(&self, id: OperationId, concerns: Construct) -> Checked<&'a Operation> {
        let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(id))?;
        self.lowered
            .blocks
            .get(index(block.get()))
            .zip(after.checked_sub(1))
            .and_then(|(block, position)| block.operations.get(position))
            .filter(|operation| operation.id == id)
            .ok_or_else(|| self.foreign(concerns, "an operation is not where the graph places it"))
    }

    /// The place whose contents an operation moved, whole, into the place
    /// `written` that a row at its normal continuation names; `None` when
    /// the operation stores nothing this route can trace to a place, which
    /// leaves the row's own meaning: new contents with no value identity.
    fn moved_from(
        &self,
        operation: &Operation,
        written: &Lineage,
        concerns: Construct,
    ) -> Checked<Option<Lineage>> {
        let produced = operation.result.as_structural().map(|result| result.place);
        match &operation.kind {
            OperationKind::StructuralLeafCopy { source, path }
                if produced == Some(written.root) && written.segments.is_empty() =>
            {
                self.stored_at(*source, path, concerns).map(Some)
            }
            OperationKind::EstablishRecord { fields } if produced == Some(written.root) => {
                let [Segment::Field(field)] = written.segments.as_slice() else {
                    return Ok(None);
                };
                match fields.iter().find(|operand| operand.field == *field) {
                    Some(operand) => match &operand.value {
                        RecordFieldValue::Structural(argument) => {
                            self.argument(argument, concerns).map(Some)
                        }
                        RecordFieldValue::Scalar { .. } => Ok(None),
                    },
                    None => Ok(None),
                }
            }
            OperationKind::StoreStructuralField {
                destination,
                path,
                field: Some(field),
                value,
                ..
            } => {
                let mut stored = self.stored_at(*destination, path, concerns)?;
                stored.segments.push(Segment::Field(*field));
                if stored == *written {
                    self.argument(value, concerns).map(Some)
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// What one reaching row supplies to `leaf`.
    fn supplied(
        &self,
        tables: &MachineTables,
        row: &Logged,
        leaf: &Lineage,
        budget: &mut usize,
        concerns: Construct,
    ) -> Checked<Reached> {
        if row.on_edge {
            return Err(self.unsupported(concerns, "a write on an edge overlaps a use"));
        }
        match row.content {
            Content::Value(value) if row.lineage == *leaf => Ok(Reached::Value(value)),
            // A value stored to another overlapping place, or a reference:
            // the leaf has contents this route cannot name.
            Content::Value(_) | Content::Reference(_) => Ok(Reached::Unknown),
            Content::Vacated => Err(self.unsupported(concerns, "a hole overlaps a use")),
            Content::Overwritten => {
                // Only a row on the leaf or above it replaces the leaf whole.
                let Some(beneath) = leaf.segments.strip_prefix(row.lineage.segments.as_slice())
                else {
                    return Ok(Reached::Unknown);
                };
                let Some(operation) = row.operation else {
                    return Ok(Reached::Unknown);
                };
                let operation = self.operation(operation, concerns)?;
                let Some(mut source) = self.moved_from(operation, &row.lineage, concerns)? else {
                    return Ok(Reached::Unknown);
                };
                source.segments.extend_from_slice(beneath);
                let (block, after) = self
                    .graph
                    .locate(ProgramPoint::AfterOperation(operation.id))?;
                let before = after
                    .checked_sub(1)
                    .ok_or_else(|| self.foreign(concerns, "an operation has no point before it"))?;
                self.reaching(tables, &source, block, before, budget, concerns)
            }
        }
    }

    /// What the scalar leaf `leaf` holds on every path to the point
    /// `up_to` of `block`: the one reaching-content walk.
    fn reaching(
        &self,
        tables: &MachineTables,
        leaf: &Lineage,
        block: BlockId,
        up_to: usize,
        budget: &mut usize,
        concerns: Construct,
    ) -> Checked<Reached> {
        // Each traced move continues strictly before its operation, so an
        // acyclic machine ends within one step per operation; a cycle of
        // moves is refused rather than followed for ever.
        *budget = budget
            .checked_sub(1)
            .ok_or_else(|| self.unsupported(concerns, "a chain of copies does not end"))?;
        let mut merged: Option<Reached> = None;
        let mut supply = |reached: Reached| {
            merged = Some(match (merged, reached) {
                (None, reached) => reached,
                (Some(Reached::Unestablished), _) | (Some(_), Reached::Unestablished) => {
                    Reached::Unestablished
                }
                (Some(Reached::Value(known)), Reached::Value(value)) if known == value => {
                    Reached::Value(known)
                }
                (
                    Some(Reached::Value(_) | Reached::Unknown),
                    Reached::Value(_) | Reached::Unknown,
                ) => Reached::Unknown,
            });
        };
        // Each block is walked whole at most once, so a cycle ends the walk.
        let mut walked = vec![false; self.graph.block_count()];
        let mut pending = vec![(block, up_to)];
        while let Some((block, up_to)) = pending.pop() {
            // The last overlapping write in this block at or before the
            // point: the latest position, and in one position the later log
            // entry.
            let last = self
                .log
                .iter()
                .filter(|row| {
                    row.block == block && row.position <= up_to && overlap(&row.lineage, leaf)
                })
                .fold(None::<&Logged>, |best, row| match best {
                    Some(best) if best.position > row.position => Some(best),
                    Some(_) | None => Some(row),
                });
            if let Some(row) = last {
                supply(self.supplied(tables, row, leaf, budget, concerns)?);
                continue;
            }
            if block == self.graph.entry() {
                supply(match self.root_kind(leaf.root, concerns)? {
                    // A formal is initialized storage at entry. Its content
                    // has a value identity when elaboration minted one.
                    StructuralPlaceKind::Parameter { .. } => self
                        .interned(tables, leaf, concerns)?
                        .and_then(|place| self.entry_contents.get(index(place.get())).copied())
                        .flatten()
                        .map_or(Reached::Unknown, Reached::Value),
                    StructuralPlaceKind::BlockParameter { .. }
                    | StructuralPlaceKind::Result
                    | StructuralPlaceKind::OperationResult { .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
                    | StructuralPlaceKind::TrivialAffineLocal { .. } => Reached::Unestablished,
                });
            }
            for predecessor in self.graph.predecessors(block)? {
                let slot = walked.get_mut(index(predecessor.get())).ok_or_else(|| {
                    self.foreign(
                        Construct::Block(*predecessor),
                        "an edge leaves a block past the end",
                    )
                })?;
                if !std::mem::replace(slot, true) {
                    pending.push((*predecessor, usize::MAX));
                }
            }
        }
        // No path reaches the point at all: nothing is claimed about it.
        Ok(merged.unwrap_or(Reached::Unknown))
    }

    fn budget(&self) -> usize {
        self.lowered
            .blocks
            .iter()
            .map(|block| block.operations.len())
            .sum::<usize>()
            + 1
    }

    /// The equation a read establishes on its normal successor:
    /// `read == v` when `v` is the one value its place holds on every path
    /// to it, else none.
    pub(crate) fn read_equation(
        &self,
        tables: &MachineTables,
        operation: &Operation,
    ) -> Checked<Option<Proposition>> {
        let Some(leaf) = self.read_leaf(operation)? else {
            return Ok(None);
        };
        let concerns = Construct::Operation(operation.id);
        let result = operation
            .result
            .scalar_ref()
            .ok_or_else(|| self.foreign(concerns, "a read has no scalar result"))?;
        // Elaboration interns the place of every read it emits, so an absent
        // row is a table that does not describe the module.
        if self.interned(tables, &leaf, concerns)?.is_none() {
            return Err(self.foreign(
                concerns,
                "a read names a projection the place table does not hold",
            ));
        }
        let (block, after) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let before = after
            .checked_sub(1)
            .ok_or_else(|| self.foreign(concerns, "a read has no point before it"))?;
        let mut budget = self.budget();
        let Reached::Value(held) =
            self.reaching(tables, &leaf, block, before, &mut budget, concerns)?
        else {
            return Ok(None);
        };
        let held_type =
            terminal_check_tables::row(&tables.values, held, self.machine, concerns)?.scalar_type;
        if held_type != result.scalar_type {
            return Err(self.unsupported(
                concerns,
                "a place holds a value of another type than its read",
            ));
        }
        Ok(Some(Proposition::Equal(
            ScalarTerm::value(result.id, result.scalar_type),
            ScalarTerm::value(held, held_type),
        )))
    }

    /// The places an operation reads whole or by one leaf.
    fn operation_uses(&self, operation: &Operation) -> Checked<Vec<Lineage>> {
        let concerns = Construct::Operation(operation.id);
        if let Some(leaf) = self.read_leaf(operation)? {
            return Ok(vec![leaf]);
        }
        match &operation.kind {
            OperationKind::StructuralLeafCopy { source, path } => {
                Ok(vec![self.stored_at(*source, path, concerns)?])
            }
            OperationKind::EstablishRecord { fields } => fields
                .iter()
                .filter_map(|operand| operand.value.as_structural())
                .map(|argument| self.argument(argument, concerns))
                .collect(),
            OperationKind::StoreStructuralField { value, .. } => {
                Ok(vec![self.argument(value, concerns)?])
            }
            // A call's record arguments; every other operation reads no
            // storage, by the lowered form's own accessor.
            kind => kind
                .structural_arguments()
                .iter()
                .map(|argument| self.argument(argument, concerns))
                .collect(),
        }
    }

    /// Whether every scalar leaf at or beneath each used place is supplied
    /// on every path to the point `up_to` of `block`.
    fn established(
        &self,
        tables: &MachineTables,
        used: &[Lineage],
        block: BlockId,
        up_to: usize,
        concerns: Construct,
    ) -> Checked<Checked<()>> {
        for place in used {
            for leaf in self.leaves(place, concerns)? {
                let mut budget = self.budget();
                if self.reaching(tables, &leaf, block, up_to, &mut budget, concerns)?
                    == Reached::Unestablished
                {
                    return Ok(Err(refusal(
                        RefusalReason::Unestablished,
                        self.machine,
                        concerns,
                        "storage is used on a path on which nothing was stored into it",
                    )));
                }
            }
        }
        Ok(Ok(()))
    }

    /// One verdict per operation and terminator that reads storage. A
    /// refusal returned outright concerns the module (an identity that does
    /// not resolve, storage outside this route); a use that is not
    /// established is that use's verdict.
    pub(crate) fn uses(&self, tables: &MachineTables) -> Checked<Vec<StorageUse>> {
        let mut uses = Vec::new();
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let used = self.operation_uses(operation)?;
                if used.is_empty() {
                    continue;
                }
                let at = Construct::Operation(operation.id);
                uses.push(StorageUse {
                    at,
                    verdict: self.established(tables, &used, block.id, position, at)?,
                });
            }
            let (edge, used) = match &block.terminator {
                Terminator::ReturnStructural { edge, source, .. } => (
                    *edge,
                    vec![Lineage {
                        root: *source,
                        segments: Vec::new(),
                    }],
                ),
                Terminator::InvokeNoNormal {
                    edge,
                    structural_arguments,
                    ..
                } => (
                    *edge,
                    structural_arguments
                        .iter()
                        .map(|argument| self.argument(argument, Construct::Edge(*edge)))
                        .collect::<Checked<Vec<_>>>()?,
                ),
                Terminator::Jump {
                    edge,
                    structural_arguments,
                    ..
                } => (
                    *edge,
                    structural_arguments
                        .iter()
                        .map(|argument| self.argument(argument, Construct::Edge(*edge)))
                        .collect::<Checked<Vec<_>>>()?,
                ),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    if !when_true.structural_arguments.is_empty()
                        || !when_false.structural_arguments.is_empty()
                    {
                        return Err(self.unsupported(
                            Construct::Block(block.id),
                            "storage passed on a conditional edge",
                        ));
                    }
                    continue;
                }
                Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::Crash { .. } => continue,
                _ => {
                    return Err(self.unsupported(
                        Construct::Block(block.id),
                        "a terminator outside this route",
                    ));
                }
            };
            if used.is_empty() {
                continue;
            }
            let at = Construct::Edge(edge);
            uses.push(StorageUse {
                at,
                verdict: self.established(tables, &used, block.id, usize::MAX, at)?,
            });
        }
        Ok(uses)
    }
}
