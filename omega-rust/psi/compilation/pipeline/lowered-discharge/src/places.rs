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
    BlockId, CanonicalStructuralPathSegment, EdgeId, MachineId, OperationId, PlaceId, Proposition,
    PsiSemanticId, ScalarTerm, StructuralCaseId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, PlaceRow, Segment};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, IdTable, MachineTables, ProgramPoint, RefusalReason,
};
use terminal_psi::{
    Block, Operation, OperationKind, RecordFieldValue, StructuralArgument,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
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

fn asked_root(asked: Asked<'_>) -> PlaceId {
    match asked {
        Asked::Contents(place) | Asked::Case(place) => place.root,
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
    /// Asked of a sum: the one case every path establishes it as.
    Case(StructuralCaseId),
    /// Every path supplies contents, but not one same named value or case.
    Unknown,
    /// Some path supplies nothing: no store reaches the use along it and the
    /// place held nothing at entry.
    Unestablished,
}

/// What a place is, by the lowered form's own declarations.
enum Shape<'a> {
    Scalar,
    Typed(StructuralTypeId),
    /// The payload namespace of one case of a sum: a `Case` segment, which
    /// holds no storage of its own and is followed by a payload field.
    Payload(&'a StructuralCaseDeclaration),
}

/// What the walk is asked about a place.
#[derive(Clone, Copy)]
enum Asked<'q> {
    /// What this scalar leaf, or this sum's discriminator, holds.
    Contents(&'q Lineage),
    /// Which case this sum is established as.
    Case(&'q Lineage),
}

impl Asked<'_> {
    /// Whether a logged row ends a path of the walk. For contents, any
    /// write to an overlapping place. For a case, a write to the sum or to
    /// a place above it (structural_access.md: "A write to the sum or any
    /// prefix retires these facts"); a store beneath the sum changes a
    /// payload and not the case.
    fn ended_by(self, row: &Lineage) -> bool {
        match self {
            Self::Contents(leaf) => overlap(row, leaf),
            Self::Case(sum) => row.root == sum.root && sum.segments.starts_with(&row.segments),
        }
    }
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

    fn field_shape(
        &self,
        fields: &'a [StructuralFieldDeclaration],
        field: semantic_vocabulary::StructuralFieldId,
        concerns: Construct,
    ) -> Checked<Shape<'a>> {
        let declared = fields
            .iter()
            .find(|declared| declared.id == field)
            .ok_or_else(|| self.foreign(concerns, "a field is not of the type it projects"))?;
        match declared.field_type {
            StructuralFieldType::Scalar(_) => Ok(Shape::Scalar),
            StructuralFieldType::Structural(nested) => Ok(Shape::Typed(nested)),
            _ => Err(self.unsupported(concerns, "a field type outside plain data")),
        }
    }

    /// What the place at `lineage` is: resolved segment by segment against
    /// the type the previous segment reached. A `Case` segment must name a
    /// declared case of the exact sum it is applied to.
    fn shape(&self, place: &Lineage, concerns: Construct) -> Checked<Shape<'a>> {
        let mut at = Shape::Typed(self.root_type(place.root, concerns)?);
        for segment in &place.segments {
            at = match (at, segment) {
                (Shape::Scalar, _) => {
                    return Err(self.foreign(concerns, "a place lies beneath a scalar field"));
                }
                (Shape::Typed(owner), Segment::Field(field)) => {
                    let StructuralTypeShape::Record { fields, .. } =
                        &self.declaration(owner, concerns)?.shape
                    else {
                        return Err(
                            self.unsupported(concerns, "a field of storage that is no record")
                        );
                    };
                    self.field_shape(fields, *field, concerns)?
                }
                (Shape::Typed(owner), Segment::Case(case)) => {
                    let StructuralTypeShape::Sum { cases } =
                        &self.declaration(owner, concerns)?.shape
                    else {
                        return Err(self.unsupported(concerns, "a case of storage that is no sum"));
                    };
                    Shape::Payload(
                        cases
                            .iter()
                            .find(|declared| declared.id == *case)
                            .ok_or_else(|| {
                                self.foreign(concerns, "a case is not of the sum it is applied to")
                            })?,
                    )
                }
                (Shape::Payload(case), Segment::Field(field)) => {
                    self.field_shape(&case.fields, *field, concerns)?
                }
                (Shape::Payload(_), Segment::Case(_)) => {
                    return Err(self.foreign(concerns, "a case beneath a case"));
                }
                (
                    Shape::Typed(_) | Shape::Payload(_),
                    Segment::FixedIndex(_) | Segment::RuntimeIndex { .. },
                ) => {
                    return Err(self.unsupported(concerns, "a place through an index"));
                }
            };
        }
        Ok(at)
    }

    /// Every leaf at or beneath `place`, as lineages: each scalar, and each
    /// sum itself, whose discriminator is contents like any other.
    fn leaves(&self, place: &Lineage, concerns: Construct) -> Checked<Vec<Lineage>> {
        let mut found = Vec::new();
        let mut at = place.clone();
        match self.shape(place, concerns)? {
            Shape::Scalar => found.push(at),
            Shape::Typed(structural_type) => {
                self.leaves_of(structural_type, &mut at, &mut found, concerns)?;
            }
            Shape::Payload(case) => {
                self.field_leaves(&case.fields, &mut at, &mut found, concerns)?
            }
        }
        Ok(found)
    }

    fn field_leaves(
        &self,
        fields: &[StructuralFieldDeclaration],
        at: &mut Lineage,
        found: &mut Vec<Lineage>,
        concerns: Construct,
    ) -> Checked<()> {
        for field in fields {
            at.segments.push(Segment::Field(field.id));
            match field.field_type {
                StructuralFieldType::Scalar(_) => found.push(at.clone()),
                StructuralFieldType::Structural(nested) => {
                    self.leaves_of(nested, at, found, concerns)?;
                }
                _ => return Err(self.unsupported(concerns, "a field type outside plain data")),
            }
            at.segments.pop();
        }
        Ok(())
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
                self.field_leaves(fields, at, found, concerns)?;
            }
            // A sum is established when something was stored into it whole,
            // whatever its case: the sum's own place stands for its
            // discriminator, and the payload leaves of every case lie under
            // it. A sum with no payload has only that one leaf.
            StructuralTypeShape::Sum { cases } => {
                found.push(at.clone());
                for case in cases {
                    at.segments.push(Segment::Case(case.id));
                    self.field_leaves(&case.fields, at, found, concerns)?;
                    at.segments.pop();
                }
            }
            _ => return Err(self.unsupported(concerns, "storage that is no plain data")),
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
            for (position, segment) in path.iter().enumerate() {
                // A case step carries its identity; a field step is resolved
                // by the lowered form against the prefix before it, which
                // may end in the case whose payload holds the field.
                if let StructuralPathSegment::Case(case) = segment {
                    segments.push(Segment::Case(*case));
                    continue;
                }
                let field = terminal_semantics::runtime_structural_field(
                    self.types.iter(),
                    root_type,
                    &path[..=position],
                )
                .ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "a storage path that is not a path of fields and cases",
                    )
                })?;
                segments.push(Segment::Field(field.id));
            }
        }
        let place = Lineage { root, segments };
        // Resolving the shape checks each case against the sum it is
        // applied to.
        self.shape(&place, concerns)?;
        Ok(place)
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
                        CanonicalStructuralPathSegment::Case(case) => {
                            segments.push(Segment::Case(*case));
                        }
                        CanonicalStructuralPathSegment::FixedIndex(_)
                        | CanonicalStructuralPathSegment::Referent => {
                            return Err(
                                self.unsupported(concerns, "read through an index or a referent")
                            );
                        }
                    }
                }
                segments.push(Segment::Field(*field));
                let leaf = Lineage {
                    root: *source,
                    segments,
                };
                if !matches!(self.shape(&leaf, concerns)?, Shape::Scalar) {
                    return Err(
                        self.foreign(concerns, "a scalar read of storage that is no scalar")
                    );
                }
                Ok(Some(leaf))
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
            OperationKind::EstablishStructuralCase {
                result_case,
                fields,
            } if produced == Some(written.root) => {
                let [Segment::Case(case), Segment::Field(field)] = written.segments.as_slice()
                else {
                    return Ok(None);
                };
                if case != result_case {
                    return Ok(None);
                }
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

    fn block(&self, id: BlockId) -> Checked<&'a Block> {
        self.lowered
            .blocks
            .get(index(id.get()))
            .filter(|block| block.id == id)
            .ok_or_else(|| self.foreign(Construct::Block(id), "a block is not at its position"))
    }

    /// What one reaching row supplies to the place asked about.
    fn supplied(
        &self,
        tables: &MachineTables,
        row: &Logged,
        asked: Asked<'_>,
        budget: &mut usize,
        concerns: Construct,
    ) -> Checked<Reached> {
        if row.on_edge {
            return Err(self.unsupported(concerns, "a write on an edge overlaps a use"));
        }
        let place = match asked {
            Asked::Contents(place) | Asked::Case(place) => place,
        };
        match row.content {
            Content::Value(value)
                if row.lineage == *place && matches!(asked, Asked::Contents(_)) =>
            {
                Ok(Reached::Value(value))
            }
            // A value stored to another overlapping place, or a reference:
            // the place has contents this route cannot name.
            Content::Value(_) | Content::Reference(_) => Ok(Reached::Unknown),
            Content::Vacated => Err(self.unsupported(concerns, "a hole overlaps a use")),
            Content::Overwritten => {
                // Only a row on the place or above it replaces it whole.
                let Some(beneath) = place.segments.strip_prefix(row.lineage.segments.as_slice())
                else {
                    return Ok(Reached::Unknown);
                };
                let Some(operation) = row.operation else {
                    return Ok(Reached::Unknown);
                };
                let operation = self.operation(operation, concerns)?;
                // The row cannot say which case a construction established:
                // the operation at the row does.
                if let (Asked::Case(_), [], Some(case)) =
                    (asked, beneath, self.established_as(operation, &row.lineage))
                {
                    return Ok(Reached::Case(case));
                }
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
                let asked = match asked {
                    Asked::Contents(_) => Asked::Contents(&source),
                    Asked::Case(_) => Asked::Case(&source),
                };
                self.reaching(tables, asked, block, before, budget, concerns)
            }
        }
    }

    /// The case a construction established the root `written` as, when the
    /// operation is one and `written` is its whole result.
    fn established_as(&self, operation: &Operation, written: &Lineage) -> Option<StructuralCaseId> {
        let produced = operation.result.as_structural().map(|result| result.place);
        if produced != Some(written.root) || !written.segments.is_empty() {
            return None;
        }
        match &operation.kind {
            OperationKind::EstablishScalarCase { result_case, .. }
            | OperationKind::EstablishStructuralCase { result_case, .. } => Some(*result_case),
            _ => None,
        }
    }

    /// What taking an edge from `from` into `into` establishes about the
    /// case of `sum`, when it establishes anything.
    /// structural_access.md: "A dominating membership test or dispatch
    /// supplies that fact."
    ///
    /// - An edge of a `StructuralCase` on exactly this place establishes
    ///   its case. When several of its edges enter the block, as into a
    ///   wildcard arm, no one case is established.
    /// - The true edge of a `Conditional` whose condition is the result of
    ///   a `StructuralCaseMembership` on exactly this place establishes the
    ///   tested case, when the test stands in the branching block and no
    ///   write to the sum or a place above it follows the test there.
    ///
    /// The false edge establishes nothing: "Otherwise the case holds when
    /// every other declared case of that sum is refuted" needs proof about
    /// the other cases and is not decided here.
    fn edge_selects(
        &self,
        tables: &MachineTables,
        sum: &Lineage,
        from: BlockId,
        into: BlockId,
        concerns: Construct,
    ) -> Checked<Option<Reached>> {
        match &self.block(from)?.terminator {
            Terminator::StructuralCase {
                source,
                path,
                cases,
            } => {
                if self.stored_at(*source, path, concerns)? != *sum {
                    return Ok(None);
                }
                let mut entering = cases.iter().filter(|edge| edge.target == into);
                Ok(match (entering.next(), entering.next()) {
                    (Some(edge), None) => Some(Reached::Case(edge.case)),
                    (Some(_), Some(_)) => Some(Reached::Unknown),
                    (None, _) => None,
                })
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                if when_true.target != into || when_false.target == into {
                    return Ok(None);
                }
                let origin =
                    terminal_check_tables::row(&tables.values, *condition, self.machine, concerns)?
                        .origin;
                let ValueOrigin::Operation(test) = origin else {
                    return Ok(None);
                };
                let (tested_in, after) = self.graph.locate(ProgramPoint::AfterOperation(test))?;
                if tested_in != from {
                    return Ok(None);
                }
                let OperationKind::StructuralCaseMembership { source, path, case } =
                    &self.operation(test, concerns)?.kind
                else {
                    return Ok(None);
                };
                if self.stored_at(*source, path, concerns)? != *sum {
                    return Ok(None);
                }
                let retired = self.log.iter().any(|row| {
                    row.block == from
                        && row.position > after
                        && Asked::Case(sum).ended_by(&row.lineage)
                });
                Ok((!retired).then_some(Reached::Case(*case)))
            }
            _ => Ok(None),
        }
    }

    /// What the place asked about holds on every path to the point `up_to`
    /// of `block`: the one reaching walk. It answers what a scalar leaf
    /// holds and which case a sum is established as; the two differ only in
    /// which rows end a path and in what an edge can establish.
    fn reaching(
        &self,
        tables: &MachineTables,
        asked: Asked<'_>,
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
                (Some(Reached::Case(known)), Reached::Case(case)) if known == case => {
                    Reached::Case(known)
                }
                (
                    Some(Reached::Value(_) | Reached::Case(_) | Reached::Unknown),
                    Reached::Value(_) | Reached::Case(_) | Reached::Unknown,
                ) => Reached::Unknown,
            });
        };
        // Each block is walked whole at most once, so a cycle ends the walk.
        let mut walked = vec![false; self.graph.block_count()];
        let mut pending = vec![(block, up_to)];
        while let Some((block, up_to)) = pending.pop() {
            // The last row in this block, at or before the point, that ends
            // the path: the latest position, and in one position the later
            // log entry.
            let last = self
                .log
                .iter()
                .filter(|row| {
                    row.block == block && row.position <= up_to && asked.ended_by(&row.lineage)
                })
                .fold(None::<&Logged>, |best, row| match best {
                    Some(best) if best.position > row.position => Some(best),
                    Some(_) | None => Some(row),
                });
            if let Some(row) = last {
                supply(self.supplied(tables, row, asked, budget, concerns)?);
                continue;
            }
            if block == self.graph.entry() {
                supply(
                    match (asked, self.root_kind(asked_root(asked), concerns)?) {
                        // A formal is initialized storage at entry. Its content
                        // has a value identity when elaboration minted one.
                        (Asked::Contents(leaf), StructuralPlaceKind::Parameter { .. }) => self
                            .interned(tables, leaf, concerns)?
                            .and_then(|place| self.entry_contents.get(index(place.get())).copied())
                            .flatten()
                            .map_or(Reached::Unknown, Reached::Value),
                        // Which case a formal holds at entry is not known, and
                        // other storage holds nothing there; that it holds
                        // nothing is the contents question's answer to give.
                        (Asked::Case(_), _) => Reached::Unknown,
                        (
                            Asked::Contents(_),
                            StructuralPlaceKind::BlockParameter { .. }
                            | StructuralPlaceKind::Result
                            | StructuralPlaceKind::OperationResult { .. }
                            | StructuralPlaceKind::ByteSequenceLiteral { .. }
                            | StructuralPlaceKind::ProviderAttachment { .. }
                            | StructuralPlaceKind::TrivialAffineLocal { .. },
                        ) => Reached::Unestablished,
                    },
                );
            }
            let mut asked_of: Vec<BlockId> = Vec::new();
            for predecessor in self.graph.predecessors(block)? {
                // A block that enters by two edges is listed twice.
                if asked_of.contains(predecessor) {
                    continue;
                }
                asked_of.push(*predecessor);
                if let Asked::Case(sum) = asked
                    && let Some(selected) =
                        self.edge_selects(tables, sum, *predecessor, block, concerns)?
                {
                    supply(selected);
                    continue;
                }
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
        let Reached::Value(held) = self.reaching(
            tables,
            Asked::Contents(&leaf),
            block,
            before,
            &mut budget,
            concerns,
        )?
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

    /// What a dispatch binds on one of its edges: each payload parameter of
    /// the target block equal to the one value its payload leaf holds on
    /// every path to the dispatch, when there is one. The same answer a
    /// payload read would get, for the form in which a dispatch delivers a
    /// payload (control_flow.md: "Selected payload fields become scalar
    /// parameters on that case's edge before cleanup.").
    pub(crate) fn payload_equations(
        &self,
        tables: &MachineTables,
        block: &Block,
    ) -> Checked<Vec<(EdgeId, Proposition)>> {
        let Terminator::StructuralCase {
            source,
            path,
            cases,
        } = &block.terminator
        else {
            return Ok(Vec::new());
        };
        let concerns = Construct::Block(block.id);
        let subject = self.stored_at(*source, path, concerns)?;
        let mut equations = Vec::new();
        for edge in cases {
            let target = self.block(edge.target)?;
            for (position, field) in edge.payload_fields.iter().enumerate() {
                let parameter = target.parameters.get(position).ok_or_else(|| {
                    self.foreign(
                        concerns,
                        "a payload field has no parameter in its arm's block",
                    )
                })?;
                let mut leaf = subject.clone();
                leaf.segments.push(Segment::Case(edge.case));
                leaf.segments.push(Segment::Field(*field));
                let mut budget = self.budget();
                let Reached::Value(held) = self.reaching(
                    tables,
                    Asked::Contents(&leaf),
                    block.id,
                    usize::MAX,
                    &mut budget,
                    concerns,
                )?
                else {
                    continue;
                };
                let held_type =
                    terminal_check_tables::row(&tables.values, held, self.machine, concerns)?
                        .scalar_type;
                if held_type != parameter.scalar_type {
                    return Err(self.unsupported(
                        concerns,
                        "a payload holds a value of another type than its parameter",
                    ));
                }
                equations.push((
                    edge.edge,
                    Proposition::Equal(
                        ScalarTerm::value(parameter.id, parameter.scalar_type),
                        ScalarTerm::value(held, held_type),
                    ),
                ));
            }
        }
        Ok(equations)
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
            // structural_access.md: "The source must be readable and
            // established on every incoming path."
            OperationKind::StructuralCaseMembership { source, path, .. } => {
                Ok(vec![self.stored_at(*source, path, concerns)?])
            }
            OperationKind::EstablishRecord { fields }
            | OperationKind::EstablishStructuralCase { fields, .. } => fields
                .iter()
                .filter_map(|operand| operand.value.as_structural())
                .map(|argument| self.argument(argument, concerns))
                .collect(),
            OperationKind::StoreStructuralField { value, .. } => {
                Ok(vec![self.argument(value, concerns)?])
            }
            OperationKind::StructuralCaseLeafCopy { .. } => {
                Err(self.unsupported(concerns, "a case leaf copy"))
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

    /// The place an operation stores into, when it is a store.
    fn operation_destination(&self, operation: &Operation) -> Checked<Option<Lineage>> {
        let concerns = Construct::Operation(operation.id);
        let (destination, path, field) = match &operation.kind {
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                ..
            } => (destination, path, Some(*field)),
            OperationKind::StoreStructuralField {
                destination,
                path,
                field,
                ..
            } => (destination, path, *field),
            OperationKind::WriteOnlyPrimitiveStore {
                destination, path, ..
            } => (destination, path, None),
            _ => return Ok(None),
        };
        let mut stored = self.stored_at(*destination, path, concerns)?;
        if let Some(field) = field {
            stored.segments.push(Segment::Field(field));
            self.shape(&stored, concerns)?;
        }
        Ok(Some(stored))
    }

    /// The two structural judgments of one operation or terminator, in
    /// order: initialization, then payload access.
    ///
    /// Initialization: every leaf at or beneath each place in `used` is
    /// supplied on every path to the point `up_to` of `block`.
    ///
    /// Payload access: for every `Case` step on the path of a place in
    /// `named`, the sum the step is applied to is established as that case
    /// on every path to the point. A payload overlays the sum's storage, so
    /// naming one is sound only there (structural_access.md: "Each `Case`
    /// step requires source admission to establish the selected case for
    /// the same root and prefix at the read's observation epoch."). The
    /// case comes from a dispatch edge, the true edge of a membership test,
    /// or the construction that every reaching write is.
    fn judged(
        &self,
        tables: &MachineTables,
        used: &[Lineage],
        named: &[Lineage],
        block: BlockId,
        up_to: usize,
        concerns: Construct,
    ) -> Checked<Checked<()>> {
        for place in used {
            for leaf in self.leaves(place, concerns)? {
                let mut budget = self.budget();
                if self.reaching(
                    tables,
                    Asked::Contents(&leaf),
                    block,
                    up_to,
                    &mut budget,
                    concerns,
                )? == Reached::Unestablished
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
        for place in named {
            for (length, segment) in place.segments.iter().enumerate() {
                let Segment::Case(case) = segment else {
                    continue;
                };
                let sum = Lineage {
                    root: place.root,
                    segments: place.segments[..length].to_vec(),
                };
                let mut budget = self.budget();
                if self.reaching(
                    tables,
                    Asked::Case(&sum),
                    block,
                    up_to,
                    &mut budget,
                    concerns,
                )? != Reached::Case(*case)
                {
                    return Ok(Err(refusal(
                        RefusalReason::Unestablished,
                        self.machine,
                        concerns,
                        "a case payload is accessed on a path that does not establish its case",
                    )));
                }
            }
        }
        Ok(Ok(()))
    }

    /// Whether a dispatch lists every declared case of the sum it observes
    /// exactly once, each with scalar payload fields of that case matching
    /// its arm block's parameters. A declared case with no edge is the
    /// program's error (patterns.md: "Every dispatch must cover the admitted
    /// subject."); anything else wrong with the roster is a module that does
    /// not describe a dispatch.
    fn roster(
        &self,
        subject: &Lineage,
        edges: &[terminal_psi::StructuralCaseSuccessorEdge],
        concerns: Construct,
    ) -> Checked<Checked<()>> {
        let Shape::Typed(sum) = self.shape(subject, concerns)? else {
            return Err(self.foreign(concerns, "a dispatch on storage that is no sum"));
        };
        let StructuralTypeShape::Sum { cases } = &self.declaration(sum, concerns)?.shape else {
            return Err(self.foreign(concerns, "a dispatch on storage that is no sum"));
        };
        let mut listed = vec![false; cases.len()];
        for edge in edges {
            let position = cases
                .iter()
                .position(|case| case.id == edge.case)
                .ok_or_else(|| self.foreign(concerns, "a dispatch lists a case of another sum"))?;
            if std::mem::replace(&mut listed[position], true) {
                return Err(self.foreign(concerns, "a dispatch lists a case twice"));
            }
            if !edge.structural_arguments.is_empty() || !edge.cleanup_actions.is_empty() {
                return Err(self.unsupported(concerns, "storage passed on a dispatch edge"));
            }
            let parameters = &self.block(edge.target)?.parameters;
            if parameters.len() != edge.payload_fields.len() {
                return Err(self.foreign(
                    concerns,
                    "a dispatch edge and its arm's block disagree on the payload",
                ));
            }
            for (position, (field, parameter)) in
                edge.payload_fields.iter().zip(parameters).enumerate()
            {
                let declared = cases[position_of(cases, edge.case)]
                    .fields
                    .iter()
                    .find(|declared| declared.id == *field);
                let scalar = declared.and_then(|declared| declared.field_type.as_scalar());
                if scalar != Some(parameter.scalar_type)
                    || edge.payload_fields[..position].contains(field)
                {
                    return Err(self.foreign(
                        concerns,
                        "a dispatch edge binds something that is no scalar payload field of its case",
                    ));
                }
            }
        }
        if listed.contains(&false) {
            return Ok(Err(refusal(
                RefusalReason::Unestablished,
                self.machine,
                concerns,
                "a dispatch lists no edge for a declared case",
            )));
        }
        Ok(Ok(()))
    }

    /// One verdict per operation and terminator that reads storage or names
    /// a case payload. A refusal returned outright concerns the module (an
    /// identity that does not resolve, storage outside this route); a use
    /// that is not established is that use's verdict.
    pub(crate) fn uses(&self, tables: &MachineTables) -> Checked<Vec<StorageUse>> {
        let mut uses = Vec::new();
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let used = self.operation_uses(operation)?;
                let mut named = used.clone();
                named.extend(self.operation_destination(operation)?);
                if named.is_empty() {
                    continue;
                }
                // A store reads nothing, so it is a use only when its
                // destination lies in a case payload.
                if used.is_empty()
                    && !named.iter().any(|place| {
                        place
                            .segments
                            .iter()
                            .any(|segment| matches!(segment, Segment::Case(_)))
                    })
                {
                    continue;
                }
                let at = Construct::Operation(operation.id);
                uses.push(StorageUse {
                    at,
                    verdict: self.judged(tables, &used, &named, block.id, position, at)?,
                });
            }
            let (edge, used) = match &block.terminator {
                // A dispatch reads its subject, and its roster is judged
                // with it. It has one edge per case, so it is named by its
                // block.
                Terminator::StructuralCase {
                    source,
                    path,
                    cases,
                } => {
                    let at = Construct::Block(block.id);
                    let subject = [self.stored_at(*source, path, at)?];
                    let verdict = match self.roster(&subject[0], cases, at)? {
                        Ok(()) => {
                            self.judged(tables, &subject, &subject, block.id, usize::MAX, at)?
                        }
                        Err(missing) => Err(missing),
                    };
                    uses.push(StorageUse { at, verdict });
                    continue;
                }
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
                verdict: self.judged(tables, &used, &used, block.id, usize::MAX, at)?,
            });
        }
        Ok(uses)
    }
}

fn position_of(cases: &[StructuralCaseDeclaration], case: StructuralCaseId) -> usize {
    cases
        .iter()
        .position(|declared| declared.id == case)
        .unwrap_or(usize::MAX)
}
